use chem_tracker::db::{open_connection, seed_sample_data};
use chem_tracker::shipments::{
    delete_shipment, list_domestic_importers, list_product_options, query_shipments, save_shipment,
    SaveShipmentInput, ShipmentFilter,
};

fn temp_db_path(name: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "chem_tracker_t03_{name}_{}_{}.db",
        std::process::id(),
        nanos
    ))
}

#[test]
fn slice1_list_domestic_importers_and_product_options() {
    let db_path = temp_db_path("slice1");
    let mut conn = open_connection(&db_path).expect("open_connection");

    // Empty database returns empty option lists
    assert!(list_domestic_importers(&conn).unwrap().is_empty());
    assert!(list_product_options(&conn).unwrap().is_empty());

    // Seed sample data
    seed_sample_data(&mut conn, &db_path).expect("seed_sample_data");

    let importers = list_domestic_importers(&conn).expect("list_domestic_importers");
    assert_eq!(importers.len(), 3);
    assert!(importers.iter().any(|i| i.name == "(주)대한케미칼"));
    assert!(importers.iter().any(|i| i.name == "한성정밀화학(주)"));
    assert!(importers.iter().any(|i| i.name == "세진소재산업(주)"));

    let products = list_product_options(&conn).expect("list_product_options");
    assert_eq!(products.len(), 3);
    let eu101 = products
        .iter()
        .find(|p| p.product_code == "PRD-EU-101")
        .expect("PRD-EU-101 present");
    assert_eq!(eu101.product_name.as_deref(), Some("코팅솔벤트 A-100"));
    assert_eq!(eu101.overseas_branch_name, "유럽지사 (독일)");
    assert!(eu101.overseas_branch_id > 0);

    let us305 = products
        .iter()
        .find(|p| p.product_code == "PRD-US-305")
        .expect("PRD-US-305 present");
    assert_eq!(us305.overseas_branch_name, "북미지사 (미국)");

    drop(conn);
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
    let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
}

#[test]
fn slice2_save_shipment_validation_inline_importer_branch_lock_update_and_delete() {
    let db_path = temp_db_path("slice2");
    let mut conn = open_connection(&db_path).expect("open_connection");
    seed_sample_data(&mut conn, &db_path).expect("seed_sample_data");

    let products = list_product_options(&conn).unwrap();
    let eu101 = products.iter().find(|p| p.product_code == "PRD-EU-101").unwrap();
    let us305 = products.iter().find(|p| p.product_code == "PRD-US-305").unwrap();

    // 1. Validation failures
    // Invalid calendar dates
    for bad_date in [
        "",
        "2026/05/01",
        "2026-13-01",
        "2026-02-29", // 2026 is not a leap year
        "2026-04-31",
    ] {
        let res = save_shipment(
            &conn,
            SaveShipmentInput {
                id: None,
                shipment_date: bad_date.to_string(),
                domestic_importer_name: "(주)대한케미칼".to_string(),
                product_id: eu101.product_id,
                import_weight_kg: "100".to_string(),
                note: None,
            },
        );
        assert!(res.is_err(), "expected invalid date rejection for '{bad_date}'");
    }

    // Blank importer
    assert!(save_shipment(
        &conn,
        SaveShipmentInput {
            id: None,
            shipment_date: "2024-02-29".to_string(), // valid leap day
            domestic_importer_name: "   ".to_string(),
            product_id: eu101.product_id,
            import_weight_kg: "100".to_string(),
            note: None,
        }
    )
    .is_err());

    // Non-existent product
    assert!(save_shipment(
        &conn,
        SaveShipmentInput {
            id: None,
            shipment_date: "2026-06-15".to_string(),
            domestic_importer_name: "(주)대한케미칼".to_string(),
            product_id: 999_999,
            import_weight_kg: "100".to_string(),
            note: None,
        }
    )
    .is_err());

    // Zero, negative, or > 4dp weight
    for bad_weight in ["0", "-10.5", "12.34567", "abc", ""] {
        assert!(
            save_shipment(
                &conn,
                SaveShipmentInput {
                    id: None,
                    shipment_date: "2026-06-15".to_string(),
                    domestic_importer_name: "(주)대한케미칼".to_string(),
                    product_id: eu101.product_id,
                    import_weight_kg: bad_weight.to_string(),
                    note: None,
                }
            )
            .is_err(),
            "expected weight rejection for '{bad_weight}'"
        );
    }

    // 2. Create shipment with inline creation of a brand-new domestic importer + leap day
    let created = save_shipment(
        &conn,
        SaveShipmentInput {
            id: None,
            shipment_date: "2024-02-29".to_string(),
            domestic_importer_name: "  (주)신규수입상사  ".to_string(),
            product_id: eu101.product_id,
            import_weight_kg: "1,250.3750".to_string(),
            note: Some("  초도 입고  ".to_string()),
        },
    )
    .expect("create shipment with inline importer");

    assert_eq!(created.shipment_date, "2024-02-29");
    assert_eq!(created.shipment_year, 2024);
    assert_eq!(created.domestic_importer_name, "(주)신규수입상사");
    assert_eq!(created.overseas_branch_id, eu101.overseas_branch_id);
    assert_eq!(created.overseas_branch_name, "유럽지사 (독일)");
    assert_eq!(created.product_code, "PRD-EU-101");
    assert_eq!(created.import_weight_kg_text, "1250.375");
    assert_eq!(created.import_weight_kg_scaled, 12_503_750);
    assert_eq!(created.import_weight_kg_display, "1,250.375");
    assert_eq!(created.import_weight_ton_display, "1.2504");
    assert_eq!(created.note.as_deref(), Some("초도 입고"));

    // Inline importer was persisted cleanly and is reused on subsequent save
    let importers = list_domestic_importers(&conn).unwrap();
    assert_eq!(importers.len(), 4);

    // 3. Update shipment: switch product -> automatically re-locks overseas_branch_id to North America
    let updated = save_shipment(
        &conn,
        SaveShipmentInput {
            id: Some(created.id),
            shipment_date: "2026-07-01".to_string(),
            domestic_importer_name: "(주)신규수입상사".to_string(),
            product_id: us305.product_id,
            import_weight_kg: "0.04".to_string(),
            note: Some("   ".to_string()),
        },
    )
    .expect("update shipment");

    assert_eq!(updated.id, created.id);
    assert_eq!(updated.shipment_year, 2026);
    assert_eq!(updated.overseas_branch_id, us305.overseas_branch_id);
    assert_eq!(updated.overseas_branch_name, "북미지사 (미국)");
    assert_eq!(updated.import_weight_kg_text, "0.04");
    assert_eq!(updated.import_weight_kg_scaled, 400);
    assert_eq!(updated.import_weight_ton_display, "0.00004");
    assert_eq!(updated.note, None);
    assert_eq!(list_domestic_importers(&conn).unwrap().len(), 4);

    // Updating non-existent shipment returns error
    assert!(save_shipment(
        &conn,
        SaveShipmentInput {
            id: Some(999_999),
            shipment_date: "2026-07-01".to_string(),
            domestic_importer_name: "(주)신규수입상사".to_string(),
            product_id: us305.product_id,
            import_weight_kg: "10".to_string(),
            note: None,
        }
    )
    .is_err());

    // 4. Delete shipment and verify missing delete fails
    delete_shipment(&conn, created.id).expect("delete_shipment");
    assert!(delete_shipment(&conn, created.id).is_err());

    drop(conn);
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
    let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
}

#[test]
fn slice3_query_shipments_filters_year_index_and_exact_summary_totals() {
    let db_path = temp_db_path("slice3");
    let mut conn = open_connection(&db_path).expect("open_connection");
    seed_sample_data(&mut conn, &db_path).expect("seed_sample_data");

    // Sample data has 6 shipments:
    // 2026: 2400 + 1500.5 + 3200 + 1100.25 + 4000 = 12200.75 kg
    // 2025: 1800 kg
    // Total: 14000.75 kg -> 14.00075 ton -> formatted ton 14.0008
    let all = query_shipments(&conn, ShipmentFilter::default()).expect("query all");
    assert_eq!(all.rows.len(), 6);
    assert_eq!(all.summary.total_count, 6);
    assert_eq!(all.summary.total_weight_kg_text, "14,000.75");
    assert_eq!(all.summary.total_weight_ton_text, "14.0008");
    assert_eq!(all.available_years, vec![2026, 2025]);
    // Ordered newest date first
    assert_eq!(all.rows[0].shipment_date, "2026-08-04");
    assert_eq!(all.rows.last().unwrap().shipment_date, "2025-11-12");

    // Filter by year = 2026
    let y2026 = query_shipments(
        &conn,
        ShipmentFilter {
            year: Some(2026),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(y2026.summary.total_count, 5);
    assert_eq!(y2026.summary.total_weight_kg_text, "12,200.75");
    assert_eq!(y2026.summary.total_weight_ton_text, "12.2008");

    // Filter by year = 2026 + domestic_importer = (주)대한케미칼
    let importers = list_domestic_importers(&conn).unwrap();
    let daehan = importers
        .iter()
        .find(|i| i.name == "(주)대한케미칼")
        .unwrap();
    let daehan_2026 = query_shipments(
        &conn,
        ShipmentFilter {
            year: Some(2026),
            domestic_importer_id: Some(daehan.id),
            ..Default::default()
        },
    )
    .unwrap();
    // 2400 + 1500.5 + 3200 + 1100.25 = 8200.75 kg
    assert_eq!(daehan_2026.summary.total_count, 4);
    assert_eq!(daehan_2026.summary.total_weight_kg_text, "8,200.75");
    assert_eq!(daehan_2026.summary.total_weight_ton_text, "8.2008");

    // Filter by overseas_branch (북미지사)
    let products = list_product_options(&conn).unwrap();
    let us305 = products.iter().find(|p| p.product_code == "PRD-US-305").unwrap();
    let us_only = query_shipments(
        &conn,
        ShipmentFilter {
            overseas_branch_id: Some(us305.overseas_branch_id),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(us_only.summary.total_count, 1);
    assert_eq!(us_only.summary.total_weight_kg_text, "3,200.00");
    assert_eq!(us_only.summary.total_weight_ton_text, "3.2000");

    // Filter by search_text matching product_code or product_name (case-insensitive)
    let by_code = query_shipments(
        &conn,
        ShipmentFilter {
            search_text: Some("eu-101".to_string()),
            ..Default::default()
        },
    )
    .unwrap();
    // 2026-01-15 (2400) + 2026-08-04 (1100.25) + 2025-11-12 (1800) = 5300.25
    assert_eq!(by_code.summary.total_count, 3);
    assert_eq!(by_code.summary.total_weight_kg_text, "5,300.25");

    let by_name = query_shipments(
        &conn,
        ShipmentFilter {
            search_text: Some("세정제".to_string()),
            ..Default::default()
        },
    )
    .unwrap();
    // PRD-EU-204 ("고기능 세정제 B-200"): 1500.5 + 4000 = 5500.50
    assert_eq!(by_name.summary.total_count, 2);
    assert_eq!(by_name.summary.total_weight_kg_text, "5,500.50");

    // No match returns empty rows and zero formatted totals
    let none = query_shipments(
        &conn,
        ShipmentFilter {
            year: Some(1999),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(none.summary.total_count, 0);
    assert_eq!(none.summary.total_weight_kg_text, "0.00");
    assert_eq!(none.summary.total_weight_ton_text, "0.0000");

    // Exact fixed-point sum check without float drift: 0.1 + 0.2 + 0.0004 = 0.3004 kg
    let eu101 = products.iter().find(|p| p.product_code == "PRD-EU-101").unwrap();
    for w in ["0.1", "0.2", "0.0004"] {
        save_shipment(
            &conn,
            SaveShipmentInput {
                id: None,
                shipment_date: "2027-01-10".to_string(),
                domestic_importer_name: "정밀테스트(주)".to_string(),
                product_id: eu101.product_id,
                import_weight_kg: w.to_string(),
                note: None,
            },
        )
        .unwrap();
    }
    let y2027 = query_shipments(
        &conn,
        ShipmentFilter {
            year: Some(2027),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(y2027.summary.total_count, 3);
    assert_eq!(y2027.summary.total_weight_kg_text, "0.3004");
    assert_eq!(y2027.summary.total_weight_ton_text, "0.0003");

    // Verify shipment_year generated column uses idx_shipments_year_importer
    let plan: String = conn
        .query_row(
            "EXPLAIN QUERY PLAN
             SELECT id FROM import_shipments
             WHERE shipment_year = 2026 AND domestic_importer_id = 1;",
            [],
            |r| r.get(3),
        )
        .unwrap();
    assert!(
        plan.contains("idx_shipments_year_importer"),
        "expected query plan to use idx_shipments_year_importer, got: {plan}"
    );

    drop(conn);
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
    let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
}
