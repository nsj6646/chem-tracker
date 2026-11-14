use chem_tracker::backup::default_backup_filename;
use chem_tracker::db::{open_connection, seed_sample_data};
use chem_tracker::domain::{normalize_cas_number, validate_calendar_date};
use chem_tracker::excel_import::{
    commit_excel_import, preview_excel_import, ImportKind, ImportOptions,
};
use chem_tracker::masters::{
    delete_master_item, list_master_items, update_master_item, MasterEntityKind,
    UpdateMasterItemInput,
};
use chem_tracker::products::{save_product_with_bom, SaveBomComponentInput, SaveProductInput};
use chem_tracker::reports::{
    export_annual_report_xlsx, query_annual_substance_totals, AnnualReportFilter, ReportMode,
    ReportPeriodMode,
};
use chem_tracker::shipments::validate_shipment_date;
use rust_xlsxwriter::Workbook;
use std::path::PathBuf;

fn temp_file(name: &str, ext: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "chem_tracker_crf_{name}_{}_{}.{ext}",
        std::process::id(),
        nanos
    ))
}

struct TempFile(PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        if self.0.extension().and_then(|e| e.to_str()) == Some("db") {
            let wal = PathBuf::from(format!("{}-wal", self.0.display()));
            let shm = PathBuf::from(format!("{}-shm", self.0.display()));
            let _ = std::fs::remove_file(wal);
            let _ = std::fs::remove_file(shm);
        }
    }
}

#[test]
fn domain_cas_normalization_and_calendar_date_shared_helpers() {
    assert_eq!(normalize_cas_number(" 67 \u{2013} 64 - 1 "), "67-64-1");
    assert_eq!(normalize_cas_number("7732\u{2212}18\u{2015}5"), "7732-18-5");
    assert_eq!(validate_calendar_date("2024-02-29").unwrap(), (2024, 2, 29));
    assert!(validate_calendar_date("2026-02-29").is_err());
    assert_eq!(
        validate_shipment_date(" 2026-03-15 ").unwrap(),
        "2026-03-15"
    );
    assert!(validate_shipment_date("1899-12-31").is_err());
    assert_eq!(
        default_backup_filename("20261114").unwrap(),
        "chem-tracker-backup-20261114.db"
    );
}

#[test]
fn products_and_excel_import_normalize_unicode_cas_and_preserve_existing_names() {
    let db = TempFile(temp_file("cas_norm", "db"));
    let mut conn = open_connection(&db.0).expect("open db");
    seed_sample_data(&mut conn, &db.0).expect("seed");

    // 1. Saving a product with duplicate CAS differing only by Unicode dash & spaces must fail
    let dup_err = save_product_with_bom(
        &mut conn,
        &SaveProductInput {
            id: None,
            product_code: "PRD-DUP-CAS".to_string(),
            product_name: Some("중복 CAS 시험".to_string()),
            overseas_branch_name: "유럽지사 (독일)".to_string(),
            components: vec![
                SaveBomComponentInput {
                    cas_number: "67-64-1".to_string(),
                    substance_name: None,
                    weight_pct_text: "50".to_string(),
                },
                SaveBomComponentInput {
                    cas_number: " 67 \u{2013} 64 - 1 ".to_string(),
                    substance_name: None,
                    weight_pct_text: "50".to_string(),
                },
            ],
        },
    );
    assert!(dup_err.is_err(), "Unicode-dash CAS duplicate must be rejected");

    // 2. Saving a product with Unicode-dash CAS & empty substance_name reuses seeded 67-64-1
    // and preserves its existing non-empty name "아세톤 (Acetone)"
    let saved = save_product_with_bom(
        &mut conn,
        &SaveProductInput {
            id: None,
            product_code: "PRD-NORM-01".to_string(),
            product_name: Some("정규화 테스트".to_string()),
            overseas_branch_name: "유럽지사 (독일)".to_string(),
            components: vec![
                SaveBomComponentInput {
                    cas_number: "67\u{2013}64\u{2212}1".to_string(),
                    substance_name: Some("   ".to_string()),
                    weight_pct_text: "40".to_string(),
                },
                SaveBomComponentInput {
                    cas_number: "7732 - 18 - 5".to_string(),
                    substance_name: None,
                    weight_pct_text: "60".to_string(),
                },
            ],
        },
    )
    .expect("save normalized product");

    assert_eq!(saved.product.components[0].cas_number, "67-64-1");
    assert_eq!(
        saved.product.components[0].substance_name.as_deref(),
        Some("아세톤 (Acetone)")
    );
    assert_eq!(saved.product.components[1].cas_number, "7732-18-5");
    assert_eq!(
        saved.product.components[1].substance_name.as_deref(),
        Some("정제수 (Water)")
    );

    let cas67_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM chemical_substances WHERE cas_number LIKE '67%';",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(cas67_rows, 1, "Unicode dash CAS must not create duplicate rows");

    // 3. BOM Excel import normalizes CAS numbers in preview & commit and preserves existing names
    let xlsx_path = TempFile(temp_file("bom_cas_norm", "xlsx"));
    {
        let mut wb = Workbook::new();
        let sheet = wb.add_worksheet();
        for (c, h) in [
            "해외지사",
            "수입제품코드",
            "제품명",
            "CAS번호",
            "화학물질명",
            "함유율(%)",
        ]
        .iter()
        .enumerate()
        {
            sheet.write_string(0, c as u16, *h).unwrap();
        }
        sheet.write_string(1, 0, "유럽지사 (독일)").unwrap();
        sheet.write_string(1, 1, "PRD-XLSX-CAS").unwrap();
        sheet.write_string(1, 2, "엑셀 정규화 제품").unwrap();
        sheet.write_string(1, 3, "67 \u{2014} 64 - 1").unwrap();
        sheet.write_string(1, 4, "").unwrap();
        sheet.write_string(1, 5, "30").unwrap();

        sheet.write_string(2, 0, "유럽지사 (독일)").unwrap();
        sheet.write_string(2, 1, "PRD-XLSX-CAS").unwrap();
        sheet.write_string(2, 2, "엑셀 정규화 제품").unwrap();
        sheet.write_string(2, 3, "108\u{2013}88-3").unwrap();
        sheet.write_string(2, 4, "").unwrap();
        sheet.write_string(2, 5, "70").unwrap();
        wb.save(&xlsx_path.0).unwrap();
    }

    let preview = preview_excel_import(&conn, ImportKind::Bom, &xlsx_path.0).expect("preview bom");
    assert!(preview.can_commit);
    assert_eq!(preview.rows[0].cas_number.as_deref(), Some("67-64-1"));
    assert_eq!(preview.rows[1].cas_number.as_deref(), Some("108-88-3"));

    commit_excel_import(
        &mut conn,
        ImportKind::Bom,
        &xlsx_path.0,
        ImportOptions {
            overwrite_existing: true,
            skip_duplicates: true,
        },
    )
    .expect("commit normalized bom");

    let acetone_name: Option<String> = conn
        .query_row(
            "SELECT substance_name FROM chemical_substances WHERE cas_number = '67-64-1';",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(acetone_name.as_deref(), Some("아세톤 (Acetone)"));

    let cas_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM chemical_substances WHERE cas_number IN ('67-64-1', '108-88-3');",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(cas_count, 2);
}

#[test]
fn masters_single_row_fetch_updates_and_deletes_targeted_item_only() {
    let db = TempFile(temp_file("masters_single", "db"));
    let mut conn = open_connection(&db.0).expect("open db");
    seed_sample_data(&mut conn, &db.0).expect("seed");

    conn.execute(
        "INSERT INTO domestic_importers (name, note) VALUES ('신규 미사용 수입사', '메모1');",
        [],
    )
    .unwrap();
    let new_id = conn.last_insert_rowid();

    let updated = update_master_item(
        &mut conn,
        &UpdateMasterItemInput {
            kind: MasterEntityKind::DomesticImporter,
            id: new_id,
            name: Some("수정된 미사용 수입사".to_string()),
            cas_number: None,
            substance_name: None,
            note: Some("메모2".to_string()),
        },
    )
    .expect("update single master item");
    assert_eq!(updated.id, new_id);
    assert_eq!(updated.name.as_deref(), Some("수정된 미사용 수입사"));
    assert_eq!(updated.reference_count, 0);

    let del = delete_master_item(&mut conn, MasterEntityKind::DomesticImporter, new_id)
        .expect("delete single master item");
    assert_eq!(del.deleted_id, new_id);

    let remaining = list_master_items(&conn, MasterEntityKind::DomesticImporter).unwrap();
    assert!(!remaining.iter().any(|item| item.id == new_id));
}

#[test]
fn annual_report_period_mode_and_export_result_contract() {
    let db = TempFile(temp_file("reports_contract", "db"));
    let mut conn = open_connection(&db.0).expect("open db");
    seed_sample_data(&mut conn, &db.0).expect("seed");

    // 1. CustomRange with empty start_date & end_date MUST NOT silently fall back to target_year!
    let custom_empty = query_annual_substance_totals(
        &conn,
        &AnnualReportFilter {
            period_mode: ReportPeriodMode::CustomRange,
            target_year: Some(2025),
            start_date: None,
            end_date: None,
            domestic_importer_id: None,
            overseas_branch_id: None,
            product_id: None,
        },
    )
    .expect("custom range empty");
    assert_eq!(custom_empty.period_label, "전체 기간");
    assert_eq!(custom_empty.matching_shipment_count, 6);

    // 2. Year mode ignores stray custom start/end dates cleanly
    let year_only = query_annual_substance_totals(
        &conn,
        &AnnualReportFilter {
            period_mode: ReportPeriodMode::Year,
            target_year: Some(2025),
            start_date: Some("2026-01-01".to_string()),
            end_date: Some("2026-12-31".to_string()),
            domestic_importer_id: None,
            overseas_branch_id: None,
            product_id: None,
        },
    )
    .expect("year mode ignores custom dates");
    assert_eq!(year_only.period_label, "2025년 (01.01 ~ 12.31)");
    assert_eq!(year_only.matching_shipment_count, 1);

    // 3. export_annual_report_xlsx validates empty output path & returns ReportExportResult
    assert!(export_annual_report_xlsx(
        &conn,
        &AnnualReportFilter::default(),
        ReportMode::ExternalSummary,
        "",
    )
    .is_err());

    let out_xlsx = TempFile(temp_file("report_result", "xlsx"));
    let export_res = export_annual_report_xlsx(
        &conn,
        &AnnualReportFilter {
            period_mode: ReportPeriodMode::Year,
            target_year: Some(2026),
            ..Default::default()
        },
        ReportMode::InternalDetail,
        &out_xlsx.0,
    )
    .expect("export internal detail");
    assert_eq!(export_res.sheet_count, 2);
    assert_eq!(export_res.saved_path, out_xlsx.0.display().to_string());
    assert!(export_res.message.contains("2개 시트"));
    assert_eq!(export_res.report.matching_shipment_count, 5);
}
