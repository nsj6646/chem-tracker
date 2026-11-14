use chem_tracker::db::{open_connection, seed_sample_data};
use chem_tracker::masters::{
    delete_master_item, list_master_items, update_master_item, MasterEntityKind,
    UpdateMasterItemInput,
};
use chem_tracker::products::get_product_detail;
use chem_tracker::reports::{query_annual_substance_totals, AnnualReportFilter};
use chem_tracker::shipments::{query_shipments, ShipmentFilter};
use std::path::PathBuf;

fn temp_db_path(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "chem_tracker_t04_{name}_{}_{}.db",
        std::process::id(),
        nanos
    ))
}

struct TempDb {
    path: PathBuf,
}

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(self.path.with_extension("db-wal"));
        let _ = std::fs::remove_file(self.path.with_extension("db-shm"));
    }
}

#[test]
fn slice1_list_master_items_returns_records_with_exact_reference_counts() {
    let db_path = temp_db_path("slice1");
    let _guard = TempDb {
        path: db_path.clone(),
    };
    let mut conn = open_connection(&db_path).expect("open_connection");
    seed_sample_data(&mut conn, &db_path).expect("seed_sample_data");

    // 1. Domestic Importers:
    // (주)대한케미칼 -> 5 shipments, 한성정밀화학(주) -> 1 shipment, (주)미래소재 -> 0 shipments
    let importers = list_master_items(&conn, MasterEntityKind::DomesticImporter)
        .expect("list domestic importers");
    assert_eq!(importers.len(), 3);

    let daehan = importers
        .iter()
        .find(|i| i.name.as_deref() == Some("(주)대한케미칼"))
        .expect("daehan importer");
    assert_eq!(daehan.shipment_count, 5);
    assert_eq!(daehan.reference_count, 5);
    assert_eq!(daehan.product_count, 0);
    assert_eq!(daehan.component_count, 0);

    let sejin = importers
        .iter()
        .find(|i| i.name.as_deref() == Some("세진소재산업(주)"))
        .expect("sejin importer");
    assert_eq!(sejin.shipment_count, 0);
    assert_eq!(sejin.reference_count, 0);

    // 2. Overseas Branches:
    // 유럽지사 (독일) -> 2 products + 5 shipments = 7 refs
    // 북미지사 (미국) -> 1 product + 1 shipment = 2 refs
    // 일본지사 (도쿄) -> 0 products + 0 shipments = 0 refs
    let branches =
        list_master_items(&conn, MasterEntityKind::OverseasBranch).expect("list overseas branches");
    assert_eq!(branches.len(), 3);

    let eu = branches
        .iter()
        .find(|b| b.name.as_deref() == Some("유럽지사 (독일)"))
        .expect("eu branch");
    assert_eq!(eu.product_count, 2);
    assert_eq!(eu.shipment_count, 5);
    assert_eq!(eu.reference_count, 7);

    let jp = branches
        .iter()
        .find(|b| b.name.as_deref() == Some("일본지사 (도쿄)"))
        .expect("jp branch");
    assert_eq!(jp.product_count, 0);
    assert_eq!(jp.shipment_count, 0);
    assert_eq!(jp.reference_count, 0);

    // 3. Chemical Substances:
    // 67-64-1 (아세톤) -> 2 BOM components, 108-88-3 (톨루엔) -> 2 BOM components,
    // plus an unreferenced substance -> 0 components
    conn.execute(
        "INSERT INTO chemical_substances (cas_number, substance_name) VALUES ('50-00-0', '포름알데히드');",
        [],
    )
    .expect("insert unreferenced substance");

    let substances = list_master_items(&conn, MasterEntityKind::ChemicalSubstance)
        .expect("list chemical substances");
    assert_eq!(substances.len(), 8);

    let acetone = substances
        .iter()
        .find(|s| s.cas_number.as_deref() == Some("67-64-1"))
        .expect("acetone");
    assert_eq!(acetone.substance_name.as_deref(), Some("아세톤 (Acetone)"));
    assert_eq!(acetone.component_count, 2);
    assert_eq!(acetone.reference_count, 2);

    let formaldehyde = substances
        .iter()
        .find(|s| s.cas_number.as_deref() == Some("50-00-0"))
        .expect("formaldehyde");
    assert_eq!(formaldehyde.component_count, 0);
    assert_eq!(formaldehyde.reference_count, 0);
}

#[test]
fn slice2_update_master_item_validates_uniqueness_and_updates_referencing_views() {
    let db_path = temp_db_path("slice2");
    let _guard = TempDb {
        path: db_path.clone(),
    };
    let mut conn = open_connection(&db_path).expect("open_connection");
    seed_sample_data(&mut conn, &db_path).expect("seed_sample_data");

    // Insert an English-named branch & importer so case-insensitive uniqueness is exercised cleanly
    conn.execute(
        "INSERT INTO overseas_branches (name) VALUES ('UK Branch');",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO domestic_importers (name) VALUES ('Alpha Chem');",
        [],
    )
    .unwrap();

    // 1. Reject empty trimmed inputs
    let importers = list_master_items(&conn, MasterEntityKind::DomesticImporter).unwrap();
    let daehan = importers
        .iter()
        .find(|i| i.name.as_deref() == Some("(주)대한케미칼"))
        .unwrap();

    let empty_imp = update_master_item(
        &mut conn,
        &UpdateMasterItemInput {
            id: daehan.id,
            kind: MasterEntityKind::DomesticImporter,
            name: Some("   ".to_string()),
            cas_number: None,
            substance_name: None,
            note: None,
        },
    );
    assert!(empty_imp.is_err(), "empty importer name must fail");

    // 2. Reject case-insensitive duplicate importer name
    let dup_imp = update_master_item(
        &mut conn,
        &UpdateMasterItemInput {
            id: daehan.id,
            kind: MasterEntityKind::DomesticImporter,
            name: Some("  alpha chem ".to_string()),
            cas_number: None,
            substance_name: None,
            note: None,
        },
    );
    let err_msg = dup_imp.unwrap_err().to_string();
    assert!(
        err_msg.contains("이미 존재"),
        "expected duplicate Korean error, got: {err_msg}"
    );

    // 3. Rename DomesticImporter in place -> shipment query reflects new name via FK ID
    let renamed_imp = update_master_item(
        &mut conn,
        &UpdateMasterItemInput {
            id: daehan.id,
            kind: MasterEntityKind::DomesticImporter,
            name: Some(" (주)대한케미칼 코리아 ".to_string()),
            cas_number: None,
            substance_name: None,
            note: Some("  본사 이전  ".to_string()),
        },
    )
    .expect("valid importer rename");
    assert_eq!(renamed_imp.name.as_deref(), Some("(주)대한케미칼 코리아"));
    assert_eq!(renamed_imp.note.as_deref(), Some("본사 이전"));
    assert_eq!(renamed_imp.reference_count, 5);

    let shipments_after = query_shipments(
        &conn,
        ShipmentFilter {
            domestic_importer_id: Some(daehan.id),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(shipments_after.rows.len(), 5);
    assert!(shipments_after
        .rows
        .iter()
        .all(|r| r.domestic_importer_name == "(주)대한케미칼 코리아"));

    // 4. Reject case-insensitive duplicate OverseasBranch & allow valid rename in place
    let branches = list_master_items(&conn, MasterEntityKind::OverseasBranch).unwrap();
    let eu = branches
        .iter()
        .find(|b| b.name.as_deref() == Some("유럽지사 (독일)"))
        .unwrap();

    let dup_branch = update_master_item(
        &mut conn,
        &UpdateMasterItemInput {
            id: eu.id,
            kind: MasterEntityKind::OverseasBranch,
            name: Some("uk branch".to_string()),
            cas_number: None,
            substance_name: None,
            note: None,
        },
    );
    assert!(dup_branch.is_err(), "case-insensitive duplicate branch must fail");

    let renamed_eu = update_master_item(
        &mut conn,
        &UpdateMasterItemInput {
            id: eu.id,
            kind: MasterEntityKind::OverseasBranch,
            name: Some(" 유럽지사 (프랑크푸르트) ".to_string()),
            cas_number: None,
            substance_name: None,
            note: Some("EU HQ".to_string()),
        },
    )
    .expect("valid branch rename");
    assert_eq!(
        renamed_eu.name.as_deref(),
        Some("유럽지사 (프랑크푸르트)")
    );
    assert_eq!(renamed_eu.reference_count, 7);

    // Verify products and shipments reflect updated branch name via FK ID
    let prod_detail = get_product_detail(&conn, 1).unwrap();
    assert_eq!(
        prod_detail.overseas_branch_name,
        "유럽지사 (프랑크푸르트)"
    );
    let eu_shipments = query_shipments(
        &conn,
        ShipmentFilter {
            overseas_branch_id: Some(eu.id),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(eu_shipments.rows.len(), 5);
    assert!(eu_shipments
        .rows
        .iter()
        .all(|r| r.overseas_branch_name == "유럽지사 (프랑크푸르트)"));

    // 5. ChemicalSubstance: reject empty CAS, reject duplicate normalized CAS, allow update of CAS & substance_name
    let substances = list_master_items(&conn, MasterEntityKind::ChemicalSubstance).unwrap();
    let poly = substances
        .iter()
        .find(|s| s.cas_number.as_deref() == Some("9002-88-4"))
        .unwrap();

    let empty_cas = update_master_item(
        &mut conn,
        &UpdateMasterItemInput {
            id: poly.id,
            kind: MasterEntityKind::ChemicalSubstance,
            name: None,
            cas_number: Some("   ".to_string()),
            substance_name: Some("폴리에틸렌".to_string()),
            note: None,
        },
    );
    assert!(empty_cas.is_err(), "empty CAS must fail");

    let dup_cas = update_master_item(
        &mut conn,
        &UpdateMasterItemInput {
            id: poly.id,
            kind: MasterEntityKind::ChemicalSubstance,
            name: None,
            cas_number: Some(" 67 - 64 - 1 ".to_string()),
            substance_name: Some("폴리에틸렌".to_string()),
            note: None,
        },
    );
    let cas_err = dup_cas.unwrap_err().to_string();
    assert!(
        cas_err.contains("이미 존재"),
        "expected duplicate CAS Korean error, got: {cas_err}"
    );

    // Valid update: trim/normalize CAS + fill substance_name (also works when keeping same CAS)
    let updated_sub = update_master_item(
        &mut conn,
        &UpdateMasterItemInput {
            id: poly.id,
            kind: MasterEntityKind::ChemicalSubstance,
            name: None,
            cas_number: Some(" 9002 - 88 - 4 ".to_string()),
            substance_name: Some(" 폴리에틸렌 (Polyethylene) ".to_string()),
            note: None,
        },
    )
    .expect("valid substance update");
    assert_eq!(updated_sub.cas_number.as_deref(), Some("9002-88-4"));
    assert_eq!(
        updated_sub.substance_name.as_deref(),
        Some("폴리에틸렌 (Polyethylene)")
    );

    // Also update CAS typo on an existing substance and verify product BOM + annual report reflect it
    let fixed_cas = update_master_item(
        &mut conn,
        &UpdateMasterItemInput {
            id: poly.id,
            kind: MasterEntityKind::ChemicalSubstance,
            name: None,
            cas_number: Some("9002-88-5".to_string()),
            substance_name: Some("폴리에틸렌 (수정)".to_string()),
            note: None,
        },
    )
    .expect("CAS typo fix should succeed");
    assert_eq!(fixed_cas.cas_number.as_deref(), Some("9002-88-5"));

    let prod3_detail = get_product_detail(&conn, 3).unwrap();
    assert!(prod3_detail
        .components
        .iter()
        .any(|c| c.cas_number == "9002-88-5"
            && c.substance_name.as_deref() == Some("폴리에틸렌 (수정)")));

    let report = query_annual_substance_totals(
        &conn,
        &AnnualReportFilter {
            target_year: Some(2026),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(report.rows.iter().any(|r| {
        r.cas_number == "9002-88-5"
            && r.substance_name.as_deref() == Some("폴리에틸렌 (수정)")
    }));
    assert!(!report.rows.iter().any(|r| r.cas_number == "9002-88-4"));
}

#[test]
fn slice3_delete_master_item_enforces_restrict_guards_and_deletes_unreferenced() {
    let db_path = temp_db_path("slice3");
    let _guard = TempDb {
        path: db_path.clone(),
    };
    let mut conn = open_connection(&db_path).expect("open_connection");
    seed_sample_data(&mut conn, &db_path).expect("seed_sample_data");

    // 1. Referenced DomesticImporter (5 shipments) is blocked with exact count & zero data loss
    let importers = list_master_items(&conn, MasterEntityKind::DomesticImporter).unwrap();
    let daehan = importers
        .iter()
        .find(|i| i.name.as_deref() == Some("(주)대한케미칼"))
        .unwrap();
    let sejin = importers
        .iter()
        .find(|i| i.name.as_deref() == Some("세진소재산업(주)"))
        .unwrap();

    let blocked_imp = delete_master_item(&mut conn, MasterEntityKind::DomesticImporter, daehan.id)
        .expect_err("referenced domestic importer must be blocked");
    let msg_imp = blocked_imp.to_string();
    assert!(
        msg_imp.contains("5건") && msg_imp.contains("삭제할 수 없습니다"),
        "expected 5건 Korean guard error, got: {msg_imp}"
    );
    // Verify ON DELETE RESTRICT also blocks raw SQL DELETE
    assert!(conn
        .execute("DELETE FROM domestic_importers WHERE id = ?1;", [daehan.id])
        .is_err());

    // Unreferenced DomesticImporter (세진소재산업(주), 0 refs) deletes cleanly
    let deleted_imp = delete_master_item(&mut conn, MasterEntityKind::DomesticImporter, sejin.id)
        .expect("unreferenced domestic importer should delete");
    assert_eq!(deleted_imp.deleted_id, sejin.id);
    assert_eq!(deleted_imp.deleted_label, "세진소재산업(주)");
    assert_eq!(
        list_master_items(&conn, MasterEntityKind::DomesticImporter)
            .unwrap()
            .len(),
        2
    );

    // 2. Referenced OverseasBranch (유럽지사: 2 products + 5 shipments = 7 refs) is blocked with exact count
    let branches = list_master_items(&conn, MasterEntityKind::OverseasBranch).unwrap();
    let eu = branches
        .iter()
        .find(|b| b.name.as_deref() == Some("유럽지사 (독일)"))
        .unwrap();
    let jp = branches
        .iter()
        .find(|b| b.name.as_deref() == Some("일본지사 (도쿄)"))
        .unwrap();

    let blocked_branch = delete_master_item(&mut conn, MasterEntityKind::OverseasBranch, eu.id)
        .expect_err("referenced overseas branch must be blocked");
    let msg_branch = blocked_branch.to_string();
    assert!(
        msg_branch.contains("7건") && msg_branch.contains("삭제할 수 없습니다"),
        "expected 7건 Korean guard error, got: {msg_branch}"
    );
    assert!(conn
        .execute("DELETE FROM overseas_branches WHERE id = ?1;", [eu.id])
        .is_err());

    // Unreferenced OverseasBranch (일본지사 (도쿄), 0 refs) deletes cleanly
    let deleted_branch = delete_master_item(&mut conn, MasterEntityKind::OverseasBranch, jp.id)
        .expect("unreferenced overseas branch should delete");
    assert_eq!(deleted_branch.deleted_id, jp.id);
    assert_eq!(deleted_branch.deleted_label, "일본지사 (도쿄)");
    assert_eq!(
        list_master_items(&conn, MasterEntityKind::OverseasBranch)
            .unwrap()
            .len(),
        2
    );

    // 3. Referenced ChemicalSubstance (67-64-1: 2 BOM components) is blocked with exact count
    conn.execute(
        "INSERT INTO chemical_substances (cas_number, substance_name) VALUES ('50-00-0', '포름알데히드');",
        [],
    )
    .unwrap();
    let substances = list_master_items(&conn, MasterEntityKind::ChemicalSubstance).unwrap();
    let acetone = substances
        .iter()
        .find(|s| s.cas_number.as_deref() == Some("67-64-1"))
        .unwrap();
    let formaldehyde = substances
        .iter()
        .find(|s| s.cas_number.as_deref() == Some("50-00-0"))
        .unwrap();

    let blocked_sub =
        delete_master_item(&mut conn, MasterEntityKind::ChemicalSubstance, acetone.id)
            .expect_err("referenced chemical substance must be blocked");
    let msg_sub = blocked_sub.to_string();
    assert!(
        msg_sub.contains("2건") && msg_sub.contains("삭제할 수 없습니다"),
        "expected 2건 Korean guard error, got: {msg_sub}"
    );
    assert!(conn
        .execute("DELETE FROM chemical_substances WHERE id = ?1;", [acetone.id])
        .is_err());

    // Unreferenced ChemicalSubstance (50-00-0, 0 refs) deletes cleanly
    let deleted_sub = delete_master_item(
        &mut conn,
        MasterEntityKind::ChemicalSubstance,
        formaldehyde.id,
    )
    .expect("unreferenced chemical substance should delete");
    assert_eq!(deleted_sub.deleted_id, formaldehyde.id);
    assert_eq!(deleted_sub.deleted_label, "50-00-0");
    assert_eq!(
        list_master_items(&conn, MasterEntityKind::ChemicalSubstance)
            .unwrap()
            .len(),
        7
    );

    // 4. Deleting non-existent ID fails clearly without panicking
    assert!(delete_master_item(&mut conn, MasterEntityKind::ChemicalSubstance, 999_999).is_err());
}
