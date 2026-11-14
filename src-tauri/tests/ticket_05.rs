use calamine::{open_workbook, Data, Reader, Xlsx};
use chem_tracker::db::{open_connection, seed_sample_data};
use chem_tracker::excel_import::{
    commit_excel_import, default_template_filename, download_import_template, preview_excel_import,
    ImportKind, ImportOptions, PreviewRowStatus,
};
use chem_tracker::products::{get_product_detail, list_products};
use chem_tracker::shipments::{query_shipments, ShipmentFilter};
use rust_xlsxwriter::{Format, Workbook};
use std::path::PathBuf;

fn temp_file(name: &str, ext: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "chem_tracker_t05_{name}_{}_{}.{ext}",
        std::process::id(),
        nanos
    ))
}

struct TempPath(PathBuf);

impl Drop for TempPath {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn slice1_download_import_templates_preserve_cas_text_format() {
    assert_eq!(
        default_template_filename(ImportKind::Bom),
        "제품구성성분_BOM_양식.xlsx"
    );
    assert_eq!(
        default_template_filename(ImportKind::Shipments),
        "수입실적_양식.xlsx"
    );

    // 1. BOM template
    let bom_path = temp_file("bom_template", "xlsx");
    let _bom_guard = TempPath(bom_path.clone());
    let bom_res = download_import_template(ImportKind::Bom, &bom_path)
        .expect("BOM template generation should succeed");
    assert_eq!(bom_res.kind, ImportKind::Bom);
    assert!(bom_path.exists());

    let mut bom_wb: Xlsx<_> = open_workbook(&bom_path).expect("open BOM template");
    let bom_sheet_name = bom_wb.sheet_names()[0].clone();
    let bom_range = bom_wb.worksheet_range(&bom_sheet_name).expect("BOM range");
    let expected_bom_headers = [
        "해외지사",
        "수입제품코드",
        "제품명(선택)",
        "CAS번호",
        "화학물질명(선택)",
        "함유율(%)",
    ];
    for (col, h) in expected_bom_headers.iter().enumerate() {
        assert_eq!(bom_range.get((0, col)), Some(&Data::String((*h).to_string())));
    }
    // Sample rows must include CAS '50-00-0' as literal text string (never date/number)
    // and sum to 100.00%
    assert!(bom_range.height() >= 3);
    assert_eq!(
        bom_range.get((1, 3)),
        Some(&Data::String("50-00-0".to_string()))
    );
    assert_eq!(bom_range.get((1, 5)), Some(&Data::Float(35.25)));
    assert_eq!(bom_range.get((2, 5)), Some(&Data::Float(64.75)));

    // 2. Shipments template
    let ship_path = temp_file("ship_template", "xlsx");
    let _ship_guard = TempPath(ship_path.clone());
    let ship_res = download_import_template(ImportKind::Shipments, &ship_path)
        .expect("Shipments template generation should succeed");
    assert_eq!(ship_res.kind, ImportKind::Shipments);
    assert!(ship_path.exists());

    let mut ship_wb: Xlsx<_> = open_workbook(&ship_path).expect("open Shipments template");
    let ship_sheet_name = ship_wb.sheet_names()[0].clone();
    let ship_range = ship_wb
        .worksheet_range(&ship_sheet_name)
        .expect("Shipments range");
    let expected_ship_headers = [
        "수입일자(YYYY-MM-DD)",
        "국내 수입사",
        "해외지사",
        "수입제품코드",
        "제품 수입량(kg)",
        "비고(선택)",
    ];
    for (col, h) in expected_ship_headers.iter().enumerate() {
        assert_eq!(
            ship_range.get((0, col)),
            Some(&Data::String((*h).to_string()))
        );
    }
    assert!(ship_range.height() >= 2);
    assert_eq!(
        ship_range.get((1, 0)),
        Some(&Data::String("2026-03-15".to_string()))
    );
    assert_eq!(ship_range.get((1, 4)), Some(&Data::Float(1250.5)));
}

fn temp_db(name: &str) -> PathBuf {
    temp_file(name, "db")
}

struct TempDb(PathBuf);

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        let _ = std::fs::remove_file(self.0.with_extension("db-wal"));
        let _ = std::fs::remove_file(self.0.with_extension("db-shm"));
    }
}

#[test]
fn slice2_bom_preview_validates_100_pct_duplicates_formats_and_existing_conflicts() {
    let db_path = temp_db("slice2_bom");
    let _db_guard = TempDb(db_path.clone());
    let mut conn = open_connection(&db_path).expect("open_connection");
    seed_sample_data(&mut conn, &db_path).expect("seed_sample_data");

    // 1. Downloaded BOM template round-trips cleanly through preview
    let tpl_path = temp_file("bom_tpl_roundtrip", "xlsx");
    let _tpl_guard = TempPath(tpl_path.clone());
    download_import_template(ImportKind::Bom, &tpl_path).unwrap();
    let tpl_preview = preview_excel_import(&conn, ImportKind::Bom, &tpl_path)
        .expect("template should preview cleanly");
    assert_eq!(tpl_preview.total_rows, 2);
    assert_eq!(tpl_preview.total_products, 1);
    assert_eq!(tpl_preview.valid_count, 1);
    assert_eq!(tpl_preview.conflict_count, 0);
    assert_eq!(tpl_preview.error_count, 0);
    assert!(tpl_preview.can_commit);
    assert_eq!(
        tpl_preview.rows[0].cas_number.as_deref(),
        Some("50-00-0")
    );

    // 2. CSV BOM with UTF-8 BOM, whitespace, 4-decimal 100.0000% sum, and existing product conflict
    let csv_path = temp_file("bom_valid_conflict", "csv");
    let _csv_guard = TempPath(csv_path.clone());
    let csv_content = "\u{FEFF}해외지사,수입제품코드,제품명(선택),CAS번호,화학물질명(선택),함유율(%)\r\n\
         유럽지사 (독일) , prd-eu-101 , 코팅솔벤트 수정 , 67-64-1 , 아세톤 , 100 \r\n\
        일본지사 (도쿄),PRD-JP-701,,50-00-0,포름알데히드,33.3333\r\n\
        일본지사 (도쿄),PRD-JP-701,일본 코팅제,7732-18-5,,66.6667\r\n";
    std::fs::write(&csv_path, csv_content).unwrap();

    let csv_preview = preview_excel_import(&conn, ImportKind::Bom, &csv_path)
        .expect("csv BOM preview");
    assert_eq!(csv_preview.total_rows, 3);
    assert_eq!(csv_preview.total_products, 2);
    assert_eq!(csv_preview.valid_count, 1);
    assert_eq!(csv_preview.conflict_count, 1);
    assert_eq!(csv_preview.error_count, 0);
    assert!(csv_preview.can_commit);
    assert_eq!(csv_preview.bom_products[0].exists_in_db, true);
    assert_eq!(csv_preview.bom_products[0].status, PreviewRowStatus::Conflict);
    assert_eq!(csv_preview.bom_products[1].exists_in_db, false);
    assert_eq!(csv_preview.bom_products[1].status, PreviewRowStatus::Valid);
    assert_eq!(
        csv_preview.bom_products[1].product_name.as_deref(),
        Some("일본 코팅제")
    );

    // 3. Invalid .xlsx BOM: 99.9999% sum, duplicate CAS within product, >4dp & missing field
    let bad_xlsx = temp_file("bom_invalid", "xlsx");
    let _bad_guard = TempPath(bad_xlsx.clone());
    {
        let mut wb = Workbook::new();
        let text_at = Format::new().set_num_format("@");
        let sh = wb.add_worksheet();
        for (c, h) in [
            "해외지사",
            "수입제품코드",
            "제품명(선택)",
            "CAS번호",
            "화학물질명(선택)",
            "함유율(%)",
        ]
        .iter()
        .enumerate()
        {
            sh.write_string(0, c as u16, *h).unwrap();
        }
        // Product A: 99.9999% sum (rows 2 & 3)
        sh.write_string(1, 0, "유럽지사 (독일)").unwrap();
        sh.write_string(1, 1, "PRD-BAD-SUM").unwrap();
        sh.write_string_with_format(1, 3, "67-64-1", &text_at).unwrap();
        sh.write_number(1, 5, 50.0).unwrap();
        sh.write_string(2, 0, "유럽지사 (독일)").unwrap();
        sh.write_string(2, 1, "PRD-BAD-SUM").unwrap();
        sh.write_string_with_format(2, 3, "108-88-3", &text_at).unwrap();
        sh.write_number(2, 5, 49.9999).unwrap();

        // Product B: duplicate CAS 67-64-1 summing to 100% (rows 4 & 5)
        sh.write_string(3, 0, "유럽지사 (독일)").unwrap();
        sh.write_string(3, 1, "PRD-DUP-CAS").unwrap();
        sh.write_string_with_format(3, 3, "67-64-1", &text_at).unwrap();
        sh.write_number(3, 5, 40.0).unwrap();
        sh.write_string(4, 0, "유럽지사 (독일)").unwrap();
        sh.write_string(4, 1, "PRD-DUP-CAS").unwrap();
        sh.write_string_with_format(4, 3, "67-64-1", &text_at).unwrap();
        sh.write_number(4, 5, 60.0).unwrap();

        // Product C: missing overseas branch + >4 decimals (row 6)
        sh.write_string(5, 0, "  ").unwrap();
        sh.write_string(5, 1, "PRD-BAD-CELL").unwrap();
        sh.write_string_with_format(5, 3, "50-00-0", &text_at).unwrap();
        sh.write_number(5, 5, 50.12345).unwrap();

        // Product D: valid 100% (row 7)
        sh.write_string(6, 0, "북미지사 (미국)").unwrap();
        sh.write_string(6, 1, "PRD-OK-99").unwrap();
        sh.write_string_with_format(6, 3, "50-00-0", &text_at).unwrap();
        sh.write_number(6, 5, 100.0).unwrap();

        wb.save(&bad_xlsx).unwrap();
    }

    let bad_preview = preview_excel_import(&conn, ImportKind::Bom, &bad_xlsx)
        .expect("preview returns validation breakdown instead of aborting");
    assert!(!bad_preview.can_commit);
    assert_eq!(bad_preview.total_rows, 6);
    assert_eq!(bad_preview.total_products, 4);
    assert_eq!(bad_preview.valid_count, 1);
    assert_eq!(bad_preview.error_count, 5);

    // Offending rows carry Korean error descriptions
    let r2_err = bad_preview.rows[0].errors.join(" ");
    assert!(r2_err.contains("99.9999") || r2_err.contains("100%"));
    assert_eq!(bad_preview.rows[0].status, PreviewRowStatus::Error);
    assert_eq!(bad_preview.rows[1].status, PreviewRowStatus::Error);

    let r4_err = bad_preview.rows[2].errors.join(" ");
    assert!(r4_err.contains("중복") && r4_err.contains("67-64-1"));
    assert_eq!(bad_preview.rows[2].status, PreviewRowStatus::Error);
    assert_eq!(bad_preview.rows[3].status, PreviewRowStatus::Error);

    let r6_err = bad_preview.rows[4].errors.join(" ");
    assert!(r6_err.contains("해외지사"));
    assert!(r6_err.contains("소수점 4자리"));
    assert_eq!(bad_preview.rows[4].status, PreviewRowStatus::Error);

    assert_eq!(bad_preview.rows[5].status, PreviewRowStatus::Valid);
}

#[test]
fn slice3_shipment_preview_normalizes_dates_checks_unknown_products_and_detects_5tuple_duplicates()
{
    let db_path = temp_db("slice3_ship");
    let _db_guard = TempDb(db_path.clone());
    let mut conn = open_connection(&db_path).expect("open_connection");
    seed_sample_data(&mut conn, &db_path).expect("seed_sample_data");

    // 1. Downloaded Shipment template previews cleanly against seeded DB
    let tpl_path = temp_file("ship_tpl_roundtrip", "xlsx");
    let _tpl_guard = TempPath(tpl_path.clone());
    download_import_template(ImportKind::Shipments, &tpl_path).unwrap();
    let tpl_preview = preview_excel_import(&conn, ImportKind::Shipments, &tpl_path)
        .expect("shipments template preview");
    assert_eq!(tpl_preview.total_rows, 1);
    assert_eq!(tpl_preview.valid_count, 1);
    assert_eq!(tpl_preview.conflict_count, 0);
    assert_eq!(tpl_preview.error_count, 0);
    assert!(tpl_preview.can_commit);

    // 2. Valid + duplicate (DB & intra-batch) + Excel serial date normalization
    let ship_xlsx = temp_file("ship_valid_dup", "xlsx");
    let _ship_guard = TempPath(ship_xlsx.clone());
    {
        let mut wb = Workbook::new();
        let sh = wb.add_worksheet();
        for (c, h) in [
            "수입일자(YYYY-MM-DD)",
            "국내 수입사",
            "해외지사",
            "수입제품코드",
            "제품 수입량(kg)",
            "비고(선택)",
        ]
        .iter()
        .enumerate()
        {
            sh.write_string(0, c as u16, *h).unwrap();
        }
        // Row 2: Excel serial 46096 => 2026-03-15 (new valid shipment)
        sh.write_number(1, 0, 46096.0).unwrap();
        sh.write_string(1, 1, " (주)신규수입사 ").unwrap();
        sh.write_string(1, 2, "유럽지사 (독일)").unwrap();
        sh.write_string(1, 3, "PRD-EU-101").unwrap();
        sh.write_number(1, 4, 550.25).unwrap();
        sh.write_string(1, 5, "신규 입고").unwrap();

        // Row 3: Intra-batch duplicate of Row 2 using slash date & trailing zeroes
        sh.write_string(2, 0, "2026/3/15").unwrap();
        sh.write_string(2, 1, "(주)신규수입사").unwrap();
        sh.write_string(2, 2, "유럽지사 (독일)").unwrap();
        sh.write_string(2, 3, "prd-eu-101").unwrap();
        sh.write_string(2, 4, "550.2500").unwrap();

        // Row 4: Existing DB duplicate of seeded shipment ('2026-01-15', '(주)대한케미칼', '유럽지사 (독일)', 'PRD-EU-101', 2400)
        sh.write_string(3, 0, "2026.01.15").unwrap();
        sh.write_string(3, 1, "(주)대한케미칼").unwrap();
        sh.write_string(3, 2, "유럽지사 (독일)").unwrap();
        sh.write_string(3, 3, "PRD-EU-101").unwrap();
        sh.write_string(3, 4, "2,400.00").unwrap();

        wb.save(&ship_xlsx).unwrap();
    }

    let dup_preview = preview_excel_import(&conn, ImportKind::Shipments, &ship_xlsx)
        .expect("shipment duplicate preview");
    assert!(dup_preview.can_commit);
    assert_eq!(dup_preview.total_rows, 3);
    assert_eq!(dup_preview.valid_count, 1);
    assert_eq!(dup_preview.conflict_count, 2);
    assert_eq!(dup_preview.error_count, 0);
    assert_eq!(
        dup_preview.rows[0].shipment_date.as_deref(),
        Some("2026-03-15")
    );
    assert_eq!(dup_preview.rows[0].status, PreviewRowStatus::Valid);
    assert_eq!(dup_preview.rows[1].status, PreviewRowStatus::Conflict);
    assert!(dup_preview.rows[1].is_conflict);
    assert_eq!(dup_preview.rows[2].status, PreviewRowStatus::Conflict);
    assert!(dup_preview.rows[2].is_conflict);

    // 3. Invalid shipments CSV: invalid calendar date, non-positive weight, unknown product code, branch mismatch
    let bad_csv = temp_file("ship_invalid", "csv");
    let _bad_guard = TempPath(bad_csv.clone());
    let bad_content = "수입일자(YYYY-MM-DD),국내 수입사,해외지사,수입제품코드,제품 수입량(kg),비고(선택)\n\
        2026-02-29,(주)대한케미칼,유럽지사 (독일),PRD-EU-101,100,\n\
        2026-04-10,(주)대한케미칼,유럽지사 (독일),PRD-EU-101,0,\n\
        2026-04-11,(주)대한케미칼,유럽지사 (독일),PRD-UNKNOWN-999,250.5,\n\
        2026-04-12,(주)대한케미칼,유럽지사 (독일),PRD-US-305,300,\n";
    std::fs::write(&bad_csv, bad_content).unwrap();

    let bad_preview = preview_excel_import(&conn, ImportKind::Shipments, &bad_csv)
        .expect("invalid shipment preview returns errors");
    assert!(!bad_preview.can_commit);
    assert_eq!(bad_preview.total_rows, 4);
    assert_eq!(bad_preview.error_count, 4);
    assert_eq!(
        bad_preview.unknown_product_codes,
        vec!["PRD-UNKNOWN-999".to_string()]
    );
    assert!(bad_preview.rows[0]
        .errors
        .iter()
        .any(|e| e.contains("날짜") || e.contains("수입일자")));
    assert!(bad_preview.rows[1]
        .errors
        .iter()
        .any(|e| e.contains("0보다 커야")));
    assert!(bad_preview.rows[2]
        .errors
        .iter()
        .any(|e| e.contains("PRD-UNKNOWN-999")));
    assert!(bad_preview.rows[3]
        .errors
        .iter()
        .any(|e| e.contains("해외지사")));
}

#[test]
fn slice4_atomic_commit_options_and_full_rollback_on_error() {
    let db_path = temp_db("slice4_commit");
    let _db_guard = TempDb(db_path.clone());
    let mut conn = open_connection(&db_path).expect("open_connection");
    seed_sample_data(&mut conn, &db_path).expect("seed_sample_data");

    // 1. Validation-failing BOM file must be rejected without touching DB
    let invalid_bom_csv = temp_file("commit_invalid_bom", "csv");
    let _inv_guard = TempPath(invalid_bom_csv.clone());
    std::fs::write(
        &invalid_bom_csv,
        "해외지사,수입제품코드,제품명(선택),CAS번호,화학물질명(선택),함유율(%)\n\
         일본지사 (도쿄),PRD-FAIL-01,실패제품,50-00-0,포름알데히드,99.9999\n",
    )
    .unwrap();
    let invalid_res = commit_excel_import(
        &mut conn,
        ImportKind::Bom,
        &invalid_bom_csv,
        ImportOptions::default(),
    );
    assert!(invalid_res.is_err());
    assert_eq!(list_products(&conn, None, None).unwrap().len(), 3);

    // 2. BOM with 1 existing product (PRD-EU-101) and 1 new product (PRD-JP-900)
    let bom_csv = temp_file("commit_bom_mix", "csv");
    let _bom_guard = TempPath(bom_csv.clone());
    std::fs::write(
        &bom_csv,
        "해외지사,수입제품코드,제품명(선택),CAS번호,화학물질명(선택),함유율(%)\n\
         유럽지사 (독일),PRD-EU-101,코팅솔벤트 개정판,50-00-0,포름알데히드,25\n\
         유럽지사 (독일),PRD-EU-101,코팅솔벤트 개정판,7732-18-5,정제수,75\n\
         일본지사 (도쿄),PRD-JP-900,신규 도쿄 제품,67-64-1,,100\n",
    )
    .unwrap();

    // 2a. With overwrite_existing = false: skips PRD-EU-101, inserts PRD-JP-900
    let skip_bom_res = commit_excel_import(
        &mut conn,
        ImportKind::Bom,
        &bom_csv,
        ImportOptions {
            overwrite_existing: false,
            skip_duplicates: true,
        },
    )
    .expect("BOM skip existing commit");
    assert_eq!(skip_bom_res.inserted_count, 1);
    assert_eq!(skip_bom_res.updated_count, 0);
    assert_eq!(skip_bom_res.skipped_count, 1);
    let products_after_skip = list_products(&conn, None, None).unwrap();
    assert_eq!(products_after_skip.len(), 4);
    let eu101_before = products_after_skip
        .iter()
        .find(|p| p.product_code == "PRD-EU-101")
        .unwrap();
    assert_eq!(eu101_before.component_count, 3); // unchanged!

    // 2b. With overwrite_existing = true (default): overwrites PRD-EU-101 even though shipments reference it
    let overwrite_bom_res = commit_excel_import(
        &mut conn,
        ImportKind::Bom,
        &bom_csv,
        ImportOptions {
            overwrite_existing: true,
            skip_duplicates: true,
        },
    )
    .expect("BOM overwrite existing commit");
    assert_eq!(overwrite_bom_res.inserted_count, 0);
    assert_eq!(overwrite_bom_res.updated_count, 2); // PRD-EU-101 + PRD-JP-900 (now exists)
    assert_eq!(overwrite_bom_res.skipped_count, 0);
    let eu101_detail = get_product_detail(&conn, eu101_before.id).unwrap();
    assert_eq!(
        eu101_detail.product_name.as_deref(),
        Some("코팅솔벤트 개정판")
    );
    assert_eq!(eu101_detail.components.len(), 2);
    assert_eq!(eu101_detail.components[0].cas_number, "50-00-0");
    assert_eq!(eu101_detail.components[0].weight_pct_scaled, 250_000);

    // 3. Shipments commit with skip_duplicates = true vs false
    let ship_csv = temp_file("commit_ship_dup", "csv");
    let _ship_guard = TempPath(ship_csv.clone());
    // Row 1: duplicate of seeded ('2026-01-15', '(주)대한케미칼', '유럽지사 (독일)', 'PRD-EU-101', 2400)
    // Row 2: new shipment ('2026-06-01', '(주)신규수입사', '일본지사 (도쿄)', 'PRD-JP-900', 500)
    // Row 3: intra-batch duplicate of Row 2
    std::fs::write(
        &ship_csv,
        "수입일자(YYYY-MM-DD),국내 수입사,해외지사,수입제품코드,제품 수입량(kg),비고(선택)\n\
         2026-01-15,(주)대한케미칼,유럽지사 (독일),PRD-EU-101,2400,중복건\n\
         2026-06-01,(주)신규수입사,일본지사 (도쿄),PRD-JP-900,500,신규1\n\
         2026-06-01,(주)신규수입사,일본지사 (도쿄),PRD-JP-900,500.00,배치중복\n",
    )
    .unwrap();

    // 3a. Default skip_duplicates = true: inserts only Row 2 (skips Row 1 & Row 3)
    let skip_ship_res = commit_excel_import(
        &mut conn,
        ImportKind::Shipments,
        &ship_csv,
        ImportOptions::default(),
    )
    .expect("shipments skip duplicates commit");
    assert_eq!(skip_ship_res.inserted_count, 1);
    assert_eq!(skip_ship_res.skipped_count, 2);
    let q1 = query_shipments(&conn, ShipmentFilter::default()).unwrap();
    assert_eq!(q1.summary.total_count, 7); // 6 seeded + 1 new

    // 3b. Force insert skip_duplicates = false: inserts all 3 rows
    let force_ship_res = commit_excel_import(
        &mut conn,
        ImportKind::Shipments,
        &ship_csv,
        ImportOptions {
            overwrite_existing: true,
            skip_duplicates: false,
        },
    )
    .expect("shipments force insert commit");
    assert_eq!(force_ship_res.inserted_count, 3);
    assert_eq!(force_ship_res.skipped_count, 0);
    let q2 = query_shipments(&conn, ShipmentFilter::default()).unwrap();
    assert_eq!(q2.summary.total_count, 10);

    // 4. Atomic rollback verification: mid-transaction failure rolls back EVERY earlier row
    conn.execute_batch(
        "CREATE TEMP TRIGGER abort_second_shipment
         BEFORE INSERT ON import_shipments
         WHEN NEW.note = 'TRIGGER_FAIL'
         BEGIN
             SELECT RAISE(ABORT, 'simulated mid-batch failure');
         END;",
    )
    .unwrap();

    let rollback_csv = temp_file("commit_rollback", "csv");
    let _rb_guard = TempPath(rollback_csv.clone());
    std::fs::write(
        &rollback_csv,
        "수입일자(YYYY-MM-DD),국내 수입사,해외지사,수입제품코드,제품 수입량(kg),비고(선택)\n\
         2026-07-01,(주)롤백테스트수입사,일본지사 (도쿄),PRD-JP-900,100,FIRST_OK\n\
         2026-07-02,(주)롤백테스트수입사,일본지사 (도쿄),PRD-JP-900,200,TRIGGER_FAIL\n",
    )
    .unwrap();

    let rb_res = commit_excel_import(
        &mut conn,
        ImportKind::Shipments,
        &rollback_csv,
        ImportOptions::default(),
    );
    assert!(rb_res.is_err());

    // Neither the first shipment NOR the new domestic importer was persisted!
    let q3 = query_shipments(&conn, ShipmentFilter::default()).unwrap();
    assert_eq!(q3.summary.total_count, 10);
    let importer_exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM domestic_importers WHERE name = '(주)롤백테스트수입사';",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(importer_exists, 0);
}



