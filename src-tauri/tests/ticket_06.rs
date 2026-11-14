use calamine::{open_workbook, Data, Reader, Xlsx};
use chem_tracker::db::{open_connection, seed_sample_data};
use chem_tracker::decimal::parse_decimal_4dp;
use chem_tracker::reports::{
    export_annual_report_xlsx, get_report_filter_options, query_annual_substance_totals,
    AnnualReportFilter, ReportMode, ReportPeriodMode,
};
use rust_decimal::Decimal;
use std::path::PathBuf;
use std::str::FromStr;

fn temp_db_path(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "chem_tracker_t06_{name}_{}_{}.db",
        std::process::id(),
        nanos
    ))
}

fn cleanup_db(path: &PathBuf) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(path.with_extension("db-wal"));
    let _ = std::fs::remove_file(path.with_extension("db-shm"));
}

#[test]
fn slice1_annual_substance_totals_filters_and_exact_decimal_math() {
    let db_path = temp_db_path("slice1");
    let mut conn = open_connection(&db_path).expect("open_connection");
    seed_sample_data(&mut conn, &db_path).expect("seed_sample_data");

    // Verify filter options are populated from DB
    let options = get_report_filter_options(&conn).expect("get_report_filter_options");
    assert_eq!(options.years, vec![2026, 2025]);
    assert_eq!(options.domestic_importers.len(), 3);
    assert_eq!(options.overseas_branches.len(), 3);
    assert_eq!(options.imported_products.len(), 3);

    // Find IDs for (주)대한케미칼, 유럽지사 (독일), PRD-EU-101
    let daehan_id = options
        .domestic_importers
        .iter()
        .find(|i| i.label == "(주)대한케미칼")
        .unwrap()
        .id;
    let europe_id = options
        .overseas_branches
        .iter()
        .find(|b| b.label == "유럽지사 (독일)")
        .unwrap()
        .id;
    let prd_eu_101_id = options
        .imported_products
        .iter()
        .find(|p| p.product_code == "PRD-EU-101")
        .unwrap()
        .id;

    // 1. Filter by target_year = 2026 AND domestic_importer = (주)대한케미칼
    // Seeded 2026 shipments for (주)대한케미칼:
    //   - 2026-01-15 PRD-EU-101: 2400 kg (67-64-1: 45.25%, 108-88-3: 34.75%, 141-78-6: 20%)
    //   - 2026-03-10 PRD-EU-204: 1500.5 kg (67-64-1: 60%, 71-36-3: 25.5%, 7732-18-5: 14.5%)
    //   - 2026-05-22 PRD-US-305: 3200 kg (1330-20-7: 72.125%, 108-88-3: 17.875%, 9002-88-4: 10%)
    //   - 2026-08-04 PRD-EU-101: 1100.25 kg (67-64-1: 45.25%, 108-88-3: 34.75%, 141-78-6: 20%)
    // Total product weight = 2400 + 1500.5 + 3200 + 1100.25 = 8200.75 kg
    let report = query_annual_substance_totals(
        &conn,
        &AnnualReportFilter {
            period_mode: ReportPeriodMode::Year,
            target_year: Some(2026),
            start_date: None,
            end_date: None,
            domestic_importer_id: Some(daehan_id),
            overseas_branch_id: None,
            product_id: None,
        },
    )
    .expect("query_annual_substance_totals");

    assert_eq!(report.matching_shipment_count, 4);
    assert_eq!(report.unique_substance_count, 7);
    assert_eq!(
        report.total_product_weight_kg,
        Decimal::from_str("8200.75").unwrap()
    );
    assert_eq!(report.total_product_weight_kg_formatted, "8,200.75");
    assert_eq!(
        report.total_substance_kg,
        Decimal::from_str("8200.75").unwrap()
    );
    assert_eq!(report.total_substance_kg_formatted, "8,200.75");
    assert_eq!(report.total_substance_ton_formatted, "8.2008");

    // Check 67-64-1 (아세톤):
    //   2400 * 0.4525 = 1086
    //   1500.5 * 0.60 = 900.3
    //   1100.25 * 0.4525 = 497.863125
    //   Exact sum = 2484.163125 kg -> rounded 4dp = 2,484.1631 kg, ton = 2.484163125 -> 2.4842 ton
    let acetone = report
        .rows
        .iter()
        .find(|r| r.cas_number == "67-64-1")
        .expect("67-64-1 row");
    assert_eq!(
        acetone.total_substance_kg,
        Decimal::from_str("2484.163125").unwrap()
    );
    assert_eq!(acetone.total_substance_kg_formatted, "2,484.1631");
    assert_eq!(acetone.total_substance_ton_formatted, "2.4842");
    assert_eq!(acetone.breakdown.len(), 3);
    assert_eq!(acetone.breakdown[0].shipment_date, "2026-01-15");
    assert_eq!(acetone.breakdown[2].shipment_date, "2026-08-04");
    assert_eq!(
        acetone.breakdown[2].substance_weight_kg,
        Decimal::from_str("497.863125").unwrap()
    );

    // 2. Filter with custom date range + overseas branch + product
    let custom_report = query_annual_substance_totals(
        &conn,
        &AnnualReportFilter {
            period_mode: ReportPeriodMode::CustomRange,
            target_year: None,
            start_date: Some("2026-01-01".to_string()),
            end_date: Some("2026-03-31".to_string()),
            domestic_importer_id: Some(daehan_id),
            overseas_branch_id: Some(europe_id),
            product_id: Some(prd_eu_101_id),
        },
    )
    .expect("custom date range query");

    assert_eq!(custom_report.matching_shipment_count, 1);
    assert_eq!(custom_report.unique_substance_count, 3);
    assert_eq!(
        custom_report.total_product_weight_kg,
        Decimal::from_str("2400").unwrap()
    );

    // 3. Verify unrounded intermediate accumulation + small ton 5-decimal formatting:
    // Add a substance with CAS '50-00-0' at 0.04% and a product with 100 kg -> 0.04 kg = 0.00004 ton
    conn.execute(
        "INSERT INTO chemical_substances (cas_number, substance_name) VALUES ('50-00-0', '포름알데히드');",
        [],
    )
    .unwrap();
    let f_id: i64 = conn
        .query_row(
            "SELECT id FROM chemical_substances WHERE cas_number = '50-00-0';",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let w_id: i64 = conn
        .query_row(
            "SELECT id FROM chemical_substances WHERE cas_number = '7732-18-5';",
            [],
            |r| r.get(0),
        )
        .unwrap();
    conn.execute(
        "INSERT INTO imported_products (product_code, product_name, overseas_branch_id) VALUES ('PRD-TINY-1', '미량 시험제', ?1);",
        [europe_id],
    )
    .unwrap();
    let tiny_prod_id = conn.last_insert_rowid();
    let p_small = parse_decimal_4dp("0.04").unwrap();
    let p_rest = parse_decimal_4dp("99.96").unwrap();
    conn.execute(
        "INSERT INTO product_components (product_id, substance_id, weight_pct_text, weight_pct_scaled) VALUES (?1, ?2, ?3, ?4);",
        rusqlite::params![tiny_prod_id, f_id, p_small.canonical_text, p_small.scaled_i64],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO product_components (product_id, substance_id, weight_pct_text, weight_pct_scaled) VALUES (?1, ?2, ?3, ?4);",
        rusqlite::params![tiny_prod_id, w_id, p_rest.canonical_text, p_rest.scaled_i64],
    )
    .unwrap();
    let w_100 = parse_decimal_4dp("100").unwrap();
    conn.execute(
        "INSERT INTO import_shipments (shipment_date, domestic_importer_id, overseas_branch_id, product_id, import_weight_kg_text, import_weight_kg_scaled)
         VALUES ('2026-09-01', ?1, ?2, ?3, ?4, ?5);",
        rusqlite::params![daehan_id, europe_id, tiny_prod_id, w_100.canonical_text, w_100.scaled_i64],
    )
    .unwrap();

    let tiny_report = query_annual_substance_totals(
        &conn,
        &AnnualReportFilter {
            period_mode: ReportPeriodMode::Year,
            target_year: Some(2026),
            start_date: None,
            end_date: None,
            domestic_importer_id: Some(daehan_id),
            overseas_branch_id: None,
            product_id: Some(tiny_prod_id),
        },
    )
    .expect("tiny query");
    let form_row = tiny_report
        .rows
        .iter()
        .find(|r| r.cas_number == "50-00-0")
        .expect("50-00-0 row");
    assert_eq!(form_row.total_substance_kg, Decimal::from_str("0.04").unwrap());
    assert_eq!(form_row.total_substance_kg_formatted, "0.04");
    assert_eq!(form_row.total_substance_ton_formatted, "0.00004");

    // 4. Invalid date range should return Validation error
    let bad_range = query_annual_substance_totals(
        &conn,
        &AnnualReportFilter {
            period_mode: ReportPeriodMode::CustomRange,
            target_year: None,
            start_date: Some("2026-12-31".to_string()),
            end_date: Some("2026-01-01".to_string()),
            domestic_importer_id: None,
            overseas_branch_id: None,
            product_id: None,
        },
    );
    assert!(bad_range.is_err());

    drop(conn);
    cleanup_db(&db_path);
}

fn temp_xlsx_path(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "chem_tracker_t06_{name}_{}_{}.xlsx",
        std::process::id(),
        nanos
    ))
}

#[test]
fn slice2_xlsx_export_external_summary_and_internal_detail_modes() {
    let db_path = temp_db_path("slice2");
    let mut conn = open_connection(&db_path).expect("open_connection");
    seed_sample_data(&mut conn, &db_path).expect("seed_sample_data");

    let options = get_report_filter_options(&conn).unwrap();
    let daehan_id = options
        .domestic_importers
        .iter()
        .find(|i| i.label == "(주)대한케미칼")
        .unwrap()
        .id;

    // Also insert a date-like CAS number '50-00-0' to ensure '@' text format preserves it
    conn.execute(
        "INSERT INTO chemical_substances (cas_number, substance_name) VALUES ('50-00-0', '포름알데히드');",
        [],
    )
    .unwrap();
    let cas50_id: i64 = conn
        .query_row(
            "SELECT id FROM chemical_substances WHERE cas_number = '50-00-0';",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let branch_id = options.overseas_branches[0].id;
    conn.execute(
        "INSERT INTO imported_products (product_code, product_name, overseas_branch_id) VALUES ('2026-01-99', '테스트제품', ?1);",
        [branch_id],
    )
    .unwrap();
    let prod_id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO product_components (product_id, substance_id, weight_pct_text, weight_pct_scaled) VALUES (?1, ?2, '100', 1000000);",
        rusqlite::params![prod_id, cas50_id],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO import_shipments (shipment_date, domestic_importer_id, overseas_branch_id, product_id, import_weight_kg_text, import_weight_kg_scaled)
         VALUES ('2026-06-01', ?1, ?2, ?3, '125.5', 1255000);",
        rusqlite::params![daehan_id, branch_id, prod_id],
    )
    .unwrap();

    let filter = AnnualReportFilter {
        period_mode: ReportPeriodMode::Year,
        target_year: Some(2026),
        start_date: None,
        end_date: None,
        domestic_importer_id: Some(daehan_id),
        overseas_branch_id: None,
        product_id: None,
    };

    // 1. External Summary Mode (default): 1 sheet, zero confidential percentages or product codes
    let ext_path = temp_xlsx_path("external");
    export_annual_report_xlsx(&conn, &filter, ReportMode::ExternalSummary, &ext_path)
        .expect("export external xlsx");

    let mut ext_wb: Xlsx<_> = open_workbook(&ext_path).expect("open external xlsx");
    assert_eq!(ext_wb.sheet_names(), vec!["연간물질집계_요약".to_string()]);
    let ext_range = ext_wb
        .worksheet_range("연간물질집계_요약")
        .expect("external range");

    assert_eq!(ext_range.get((0, 0)), Some(&Data::String("NO".to_string())));
    assert_eq!(ext_range.get((0, 1)), Some(&Data::String("CAS번호".to_string())));
    assert_eq!(ext_range.get((0, 2)), Some(&Data::String("화학물질명".to_string())));
    assert_eq!(
        ext_range.get((0, 3)),
        Some(&Data::String("연간 총 수입량 (kg)".to_string()))
    );
    assert_eq!(
        ext_range.get((0, 4)),
        Some(&Data::String("연간 총 수입량 (ton)".to_string()))
    );

    // Find '50-00-0' row and verify it is stored as Data::String (never DateTime/Float)
    let mut found_cas50 = false;
    for r in 1..ext_range.height() {
        for c in 0..ext_range.width() {
            if let Some(Data::String(s)) = ext_range.get((r, c)) {
                assert!(!s.contains("함유율"), "confidential header leaked: {s}");
                assert!(!s.contains("PRD-"), "product code leaked: {s}");
                assert!(!s.contains("2026-01-99"), "product code leaked: {s}");
            }
        }
        if ext_range.get((r, 1)) == Some(&Data::String("50-00-0".to_string())) {
            found_cas50 = true;
            assert_eq!(ext_range.get((r, 3)), Some(&Data::Float(125.5)));
            assert_eq!(ext_range.get((r, 4)), Some(&Data::Float(0.1255)));
        }
    }
    assert!(found_cas50, "CAS 50-00-0 must be present as string");

    // 2. Internal Detail Mode: 2 sheets (`1_연간물질집계_요약` + `2_수입실적별_상세내역`)
    let int_path = temp_xlsx_path("internal");
    export_annual_report_xlsx(&conn, &filter, ReportMode::InternalDetail, &int_path)
        .expect("export internal xlsx");

    let mut int_wb: Xlsx<_> = open_workbook(&int_path).expect("open internal xlsx");
    assert_eq!(
        int_wb.sheet_names(),
        vec![
            "1_연간물질집계_요약".to_string(),
            "2_수입실적별_상세내역".to_string()
        ]
    );
    let detail_range = int_wb
        .worksheet_range("2_수입실적별_상세내역")
        .expect("detail range");
    let expected_detail_headers = [
        "CAS번호",
        "화학물질명",
        "수입일자",
        "국내 수입사",
        "해외지사",
        "수입제품코드",
        "제품명",
        "제품 수입량 (kg)",
        "함유율 (%)",
        "화학물질 환산 중량 (kg)",
        "화학물질 환산 중량 (ton)",
    ];
    for (col_idx, header) in expected_detail_headers.iter().enumerate() {
        assert_eq!(
            detail_range.get((0, col_idx)),
            Some(&Data::String((*header).to_string()))
        );
    }

    // Verify CAS번호 (col 0) and 수입제품코드 (col 5) are preserved as strings (e.g. '50-00-0', '2026-01-99')
    let mut found_detail_row = false;
    for r in 1..detail_range.height() {
        if detail_range.get((r, 0)) == Some(&Data::String("50-00-0".to_string())) {
            found_detail_row = true;
            assert_eq!(
                detail_range.get((r, 5)),
                Some(&Data::String("2026-01-99".to_string()))
            );
            assert_eq!(detail_range.get((r, 7)), Some(&Data::Float(125.5)));
            assert_eq!(detail_range.get((r, 8)), Some(&Data::Float(100.0)));
            assert_eq!(detail_range.get((r, 9)), Some(&Data::Float(125.5)));
            assert_eq!(detail_range.get((r, 10)), Some(&Data::Float(0.1255)));
        }
    }
    assert!(found_detail_row, "detail row for 50-00-0 / 2026-01-99 must exist");

    drop(ext_wb);
    drop(int_wb);
    let _ = std::fs::remove_file(&ext_path);
    let _ = std::fs::remove_file(&int_path);
    drop(conn);
    cleanup_db(&db_path);
}
