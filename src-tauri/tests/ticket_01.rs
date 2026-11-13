use calamine::{open_workbook_from_rs, Data, Reader, Xlsx};
use chem_tracker::db::{
    get_db_status, open_connection, seed_sample_data, REQUIRED_INDEXES, REQUIRED_TABLES,
};
use chem_tracker::decimal::{
    compute_substance_weight_kg, format_kg, format_percentage, format_ton, parse_decimal_4dp,
    to_scaled_4dp, HUNDRED_PERCENT_SCALED,
};
use csv::{ReaderBuilder, Trim};
use rust_decimal::{Decimal, RoundingStrategy};
use rust_xlsxwriter::{Format, Workbook};
use std::io::Cursor;
use std::str::FromStr;
use unicode_width::UnicodeWidthStr;

#[test]
fn slice1_fixed_point_decimal_parsing_and_formatting() {
    // Parsing up to 4 decimal places -> canonical string + ×10,000 scaled integer
    let parsed = parse_decimal_4dp("100.0000").expect("100.0000 should parse");
    assert_eq!(parsed.canonical_text, "100");
    assert_eq!(parsed.scaled_i64, HUNDRED_PERCENT_SCALED);
    assert_eq!(HUNDRED_PERCENT_SCALED, 1_000_000);

    let p2 = parse_decimal_4dp(" 35.2500 ").expect("whitespace trimmed");
    assert_eq!(p2.canonical_text, "35.25");
    assert_eq!(p2.scaled_i64, 352_500);

    let p3 = parse_decimal_4dp("1,250.5").expect("comma-grouped input allowed");
    assert_eq!(p3.canonical_text, "1250.5");
    assert_eq!(p3.scaled_i64, 12_505_000);

    // Reject more than 4 decimal places
    assert!(parse_decimal_4dp("12.34567").is_err());
    assert!(parse_decimal_4dp("").is_err());
    assert!(parse_decimal_4dp("abc").is_err());

    // Exact arithmetic with zero IEEE-754 drift: 0.1 + 0.2 == 0.3
    let a = Decimal::from_str("0.1").unwrap();
    let b = Decimal::from_str("0.2").unwrap();
    assert_eq!((a + b).normalize().to_string(), "0.3");

    // 1250.5 kg * (35.25 / 100) = 440.80125 kg (exact unrounded)
    let import_kg = Decimal::from_str("1250.5").unwrap();
    let weight_pct = Decimal::from_str("35.25").unwrap();
    let substance_kg = compute_substance_weight_kg(import_kg, weight_pct);
    assert_eq!(substance_kg.normalize().to_string(), "440.80125");
    assert_eq!(to_scaled_4dp(Decimal::from_str("100").unwrap()).unwrap(), 1_000_000);

    // MidpointAwayFromZero rounding check: 440.80125 -> 4 decimals = 440.8013
    assert_eq!(
        substance_kg
            .round_dp_with_strategy(4, RoundingStrategy::MidpointAwayFromZero)
            .normalize()
            .to_string(),
        "440.8013"
    );

    // Formatting rules:
    // 함유율(%): min 2 decimals, up to 4 significant decimals
    assert_eq!(format_percentage(Decimal::from_str("35").unwrap()), "35.00");
    assert_eq!(format_percentage(Decimal::from_str("12.345").unwrap()), "12.345");
    assert_eq!(format_percentage(Decimal::from_str("0.0001").unwrap()), "0.0001");

    // kg: comma-grouped, min 2 decimals, up to 4 decimals
    assert_eq!(format_kg(Decimal::from_str("1250.5").unwrap()), "1,250.50");
    assert_eq!(format_kg(substance_kg), "440.8013");
    assert_eq!(format_kg(Decimal::from_str("0.005").unwrap()), "0.005");

    // ton = kg / 1,000: comma-grouped, 4-5 decimals
    let ton = substance_kg / Decimal::ONE_THOUSAND; // 0.44080125
    assert_eq!(format_ton(ton), "0.4408");
    let small_ton = Decimal::from_str("0.00004").unwrap(); // 0.04 kg -> 0.00004 ton (needs 5 decimals)
    assert_eq!(format_ton(small_ton), "0.00004");
    let big_ton = Decimal::from_str("1234.5").unwrap();
    assert_eq!(format_ton(big_ton), "1,234.5000");
}

#[test]
fn slice2_workspace_crates_width_csv_and_xlsx_roundtrip() {
    // 1. unicode_width: Hangul = 2, ASCII = 1, mixed
    assert_eq!("CAS번호".width(), 3 + 4); // C(1) A(1) S(1) 번(2) 호(2) = 7
    assert_eq!("아세톤 (Acetone)".width(), 6 + 10); // 아세톤(6) + " (Acetone)"(10) = 16
    assert_eq!("50-00-0".width(), 7);

    // 2. csv: UTF-8 BOM, quoted fields, escaped quotes, CRLF/LF
    let raw_csv = "\u{FEFF}해외지사,수입제품코드,CAS번호,함유율(%)\r\n\"유럽지사 (독일)\",PRD-EU-101,\"50-00-0\",45.25\n\"A\"\"B 지사\",PRD-2,108-88-3,\"54.75\"\r\n";
    let mut rdr = ReaderBuilder::new()
        .has_headers(true)
        .trim(Trim::All)
        .from_reader(raw_csv.as_bytes());
    let headers = rdr.headers().expect("headers").clone();
    assert_eq!(headers.get(0), Some("해외지사"));
    assert_eq!(headers.get(2), Some("CAS번호"));
    let records: Vec<_> = rdr.records().map(|r| r.expect("valid csv row")).collect();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].get(0), Some("유럽지사 (독일)"));
    assert_eq!(records[0].get(2), Some("50-00-0"));
    assert_eq!(records[1].get(0), Some("A\"B 지사"));

    // 3. rust_xlsxwriter -> calamine round-trip
    let mut wb = Workbook::new();
    let text_fmt = Format::new().set_num_format("@");
    let num_fmt = Format::new().set_num_format("#,##0.00");
    let sheet = wb.add_worksheet();
    sheet.set_name("연간물질집계_요약").unwrap();
    sheet.set_column_width(0, 14.0).unwrap();
    sheet.write_string(0, 0, "CAS번호").unwrap();
    sheet.write_string(0, 1, "연간 총 수입량 (kg)").unwrap();
    sheet.write_string_with_format(1, 0, "50-00-0", &text_fmt).unwrap();
    sheet.write_number_with_format(1, 1, 1250.5, &num_fmt).unwrap();

    let bytes = wb.save_to_buffer().expect("xlsx buffer");
    let mut excel: Xlsx<_> = open_workbook_from_rs(Cursor::new(bytes)).expect("calamine open");
    assert_eq!(excel.sheet_names(), vec!["연간물질집계_요약".to_string()]);
    let range = excel.worksheet_range("연간물질집계_요약").expect("sheet range");
    assert_eq!(range.get((0, 0)), Some(&Data::String("CAS번호".to_string())));
    assert_eq!(range.get((1, 0)), Some(&Data::String("50-00-0".to_string())));
    assert_eq!(range.get((1, 1)), Some(&Data::Float(1250.5)));
}

fn temp_db_path(name: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("chem_tracker_{name}_{}_{}.db", std::process::id(), nanos))
}

#[test]
fn slice3_sqlite_pragmas_schema_indexes_and_constraints() {
    let db_path = temp_db_path("slice3");
    let conn = open_connection(&db_path).expect("open_connection should succeed");

    // Verify all 5 pragmas
    let journal_mode: String = conn
        .query_row("PRAGMA journal_mode;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(journal_mode.to_lowercase(), "wal");

    let synchronous: i64 = conn
        .query_row("PRAGMA synchronous;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(synchronous, 1); // NORMAL = 1

    let foreign_keys: i64 = conn
        .query_row("PRAGMA foreign_keys;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(foreign_keys, 1);

    let temp_store: i64 = conn
        .query_row("PRAGMA temp_store;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(temp_store, 2); // MEMORY = 2

    let cache_size: i64 = conn
        .query_row("PRAGMA cache_size;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(cache_size, -8000);

    // Verify all 6 tables exist
    for table in REQUIRED_TABLES {
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1;",
                [table],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "missing table {table}");
    }

    // Verify all 6 indexes exist
    for index in REQUIRED_INDEXES {
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1;",
                [index],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "missing index {index}");
    }

    // Insert base entities to test constraints
    conn.execute("INSERT INTO domestic_importers (name) VALUES ('(주)대한케미칼');", [])
        .unwrap();
    conn.execute("INSERT INTO overseas_branches (name) VALUES ('유럽지사 (독일)');", [])
        .unwrap();
    conn.execute(
        "INSERT INTO chemical_substances (cas_number, substance_name) VALUES ('67-64-1', '아세톤');",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO imported_products (product_code, product_name, overseas_branch_id) VALUES ('PRD-1', '제품1', 1);",
        [],
    )
    .unwrap();

    // CHECK (weight_pct_scaled > 0 AND weight_pct_scaled <= 1000000)
    assert!(conn
        .execute(
            "INSERT INTO product_components (product_id, substance_id, weight_pct_text, weight_pct_scaled) VALUES (1, 1, '0', 0);",
            [],
        )
        .is_err());
    assert!(conn
        .execute(
            "INSERT INTO product_components (product_id, substance_id, weight_pct_text, weight_pct_scaled) VALUES (1, 1, '100.0001', 1000001);",
            [],
        )
        .is_err());

    // Valid component insert + UNIQUE (product_id, substance_id)
    conn.execute(
        "INSERT INTO product_components (product_id, substance_id, weight_pct_text, weight_pct_scaled) VALUES (1, 1, '100', 1000000);",
        [],
    )
    .unwrap();
    assert!(conn
        .execute(
            "INSERT INTO product_components (product_id, substance_id, weight_pct_text, weight_pct_scaled) VALUES (1, 1, '10', 100000);",
            [],
        )
        .is_err());

    // ON DELETE RESTRICT on chemical_substances while referenced by product_components
    assert!(conn
        .execute("DELETE FROM chemical_substances WHERE id = 1;", [])
        .is_err());

    // CHECK (import_weight_kg_scaled > 0)
    assert!(conn
        .execute(
            "INSERT INTO import_shipments (shipment_date, domestic_importer_id, overseas_branch_id, product_id, import_weight_kg_text, import_weight_kg_scaled) VALUES ('2026-03-15', 1, 1, 1, '0', 0);",
            [],
        )
        .is_err());

    // Valid shipment + generated shipment_year + ON DELETE RESTRICT on product_id
    conn.execute(
        "INSERT INTO import_shipments (shipment_date, domestic_importer_id, overseas_branch_id, product_id, import_weight_kg_text, import_weight_kg_scaled) VALUES ('2026-03-15', 1, 1, 1, '250.5', 2505000);",
        [],
    )
    .unwrap();
    let shipment_year: i64 = conn
        .query_row("SELECT shipment_year FROM import_shipments WHERE id = 1;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(shipment_year, 2026);
    assert!(conn
        .execute("DELETE FROM imported_products WHERE id = 1;", [])
        .is_err());

    // Deleting shipment unlocks product deletion, which cascades to product_components
    conn.execute("DELETE FROM import_shipments WHERE id = 1;", []).unwrap();
    conn.execute("DELETE FROM imported_products WHERE id = 1;", []).unwrap();
    let remaining_components: i64 = conn
        .query_row("SELECT COUNT(*) FROM product_components;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(remaining_components, 0);

    drop(conn);
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
    let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
}

#[test]
fn slice4_db_status_and_sample_seed_transaction() {
    let db_path = temp_db_path("slice4");
    let mut conn = open_connection(&db_path).expect("open_connection");

    // 1. Empty DB status
    let empty_status = get_db_status(&conn, &db_path).expect("empty status");
    assert_eq!(empty_status.journal_mode.to_lowercase(), "wal");
    assert!(empty_status.integrity_ok);
    assert_eq!(empty_status.domestic_importers_count, 0);
    assert_eq!(empty_status.overseas_branches_count, 0);
    assert_eq!(empty_status.chemical_substances_count, 0);
    assert_eq!(empty_status.imported_products_count, 0);
    assert_eq!(empty_status.product_components_count, 0);
    assert_eq!(empty_status.import_shipments_count, 0);

    // 2. Seed sample data in single transaction
    let seed_res = seed_sample_data(&mut conn, &db_path).expect("seed_sample_data");
    assert!(!seed_res.already_seeded);
    assert!(seed_res.inserted_importers >= 3);
    assert!(seed_res.inserted_branches >= 3);
    assert!(seed_res.inserted_substances >= 6);
    assert!(seed_res.inserted_products >= 3);
    assert!(seed_res.inserted_shipments >= 5);

    let seeded_status = get_db_status(&conn, &db_path).expect("seeded status");
    assert_eq!(
        seeded_status.domestic_importers_count,
        seed_res.status.domestic_importers_count
    );
    assert_eq!(seeded_status.imported_products_count, 3);
    assert_eq!(seeded_status.import_shipments_count, 6);

    // 3. Every seeded product's BOM components must sum to exactly 1_000_000 (100.0000%)
    let mut stmt = conn
        .prepare(
            "SELECT ip.product_code, SUM(pc.weight_pct_scaled)
             FROM imported_products ip
             JOIN product_components pc ON pc.product_id = ip.id
             GROUP BY ip.id;",
        )
        .unwrap();
    let sums: Vec<(String, i64)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(sums.len(), 3);
    for (code, total_scaled) in sums {
        assert_eq!(
            total_scaled, HUNDRED_PERCENT_SCALED,
            "product {code} BOM sum must equal 100%"
        );
    }
    drop(stmt);

    // 4. Calling seed_sample_data again is safe and does not duplicate records
    let second = seed_sample_data(&mut conn, &db_path).expect("second seed call");
    assert!(second.already_seeded);
    assert_eq!(second.status.import_shipments_count, 6);

    drop(conn);
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
    let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
}
