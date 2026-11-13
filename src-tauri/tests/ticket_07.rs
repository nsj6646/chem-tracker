use chem_tracker::backup::{
    apply_windows_webview2_low_memory_hook, backup_database, default_backup_filename,
    restore_database, validate_backup_file, webview2_low_memory_policy,
};
use chem_tracker::db::{open_connection, seed_sample_data, REQUIRED_TABLES};
use rusqlite::{Connection, OpenFlags};
use std::path::PathBuf;

fn temp_dir_for(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "chem_tracker_t07_{name}_{}_{}",
        std::process::id(),
        nanos
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn slice1_default_backup_filename_formats_and_validates_dates() {
    assert_eq!(
        default_backup_filename("2026-11-14").expect("valid ISO date"),
        "chem-tracker-backup-20261114.db"
    );
    assert_eq!(
        default_backup_filename("20261114").expect("valid compact date"),
        "chem-tracker-backup-20261114.db"
    );
    assert_eq!(
        default_backup_filename(" 2024-02-29 ").expect("leap year date"),
        "chem-tracker-backup-20240229.db"
    );

    assert!(default_backup_filename("").is_err());
    assert!(default_backup_filename("2026-02-29").is_err()); // 2026 is not leap year
    assert!(default_backup_filename("2026-13-01").is_err());
    assert!(default_backup_filename("2026-00-10").is_err());
    assert!(default_backup_filename("abcd-ef-gh").is_err());
}

#[test]
fn slice2_backup_database_vacuums_into_standalone_file_while_live_in_wal() {
    let dir = temp_dir_for("slice2");
    let active_path = dir.join("data.db");
    let backup_path = dir.join("chem-tracker-backup-20261114.db");

    let mut conn = open_connection(&active_path).expect("open_connection");
    seed_sample_data(&mut conn, &active_path).expect("seed_sample_data");

    // Insert an extra shipment that stays in WAL (not explicitly checkpointed)
    conn.execute(
        "INSERT INTO import_shipments (shipment_date, domestic_importer_id, overseas_branch_id, product_id, import_weight_kg_text, import_weight_kg_scaled)
         VALUES ('2026-11-14', 1, 1, 1, '500', 5000000);",
        [],
    )
    .expect("extra wal shipment");

    let res = backup_database(&conn, &backup_path).expect("backup_database should succeed");
    assert!(res.file_size_bytes > 0);
    assert_eq!(res.status.import_shipments_count, 7);
    assert!(backup_path.exists());
    assert!(!dir.join("chem-tracker-backup-20261114.db-wal").exists());
    assert!(!dir.join("chem-tracker-backup-20261114.db-shm").exists());

    // Standalone backup is readable read-only without sidecars and contains all 7 shipments
    let ro_conn = Connection::open_with_flags(&backup_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .expect("open backup read-only");
    let count: i64 = ro_conn
        .query_row("SELECT COUNT(*) FROM import_shipments;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 7);
    drop(ro_conn);

    // Overwriting an existing destination safely reflects newly added WAL rows
    conn.execute(
        "INSERT INTO import_shipments (shipment_date, domestic_importer_id, overseas_branch_id, product_id, import_weight_kg_text, import_weight_kg_scaled)
         VALUES ('2026-11-15', 1, 1, 1, '250', 2500000);",
        [],
    )
    .expect("8th shipment");
    let res2 = backup_database(&conn, &backup_path).expect("overwrite backup should succeed");
    assert_eq!(res2.status.import_shipments_count, 8);

    // Reject targeting the active DB path itself or a directory
    assert!(backup_database(&conn, &active_path).is_err());
    assert!(backup_database(&conn, &dir).is_err());

    drop(conn);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn slice3_validate_backup_file_checks_integrity_and_schema_read_only() {
    let dir = temp_dir_for("slice3");
    let active_path = dir.join("data.db");
    let valid_backup = dir.join("valid.db");

    let mut conn = open_connection(&active_path).expect("open_connection");
    seed_sample_data(&mut conn, &active_path).expect("seed_sample_data");
    backup_database(&conn, &valid_backup).expect("backup_database");
    drop(conn);

    // 1. Valid backup passes even when OS file permission is read-only
    let mut perms = std::fs::metadata(&valid_backup).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&valid_backup, perms).unwrap();

    let validation = validate_backup_file(&valid_backup).expect("valid read-only backup passes");
    assert!(validation.integrity_ok);
    assert_eq!(validation.verified_tables.len(), REQUIRED_TABLES.len());
    assert_eq!(validation.status.imported_products_count, 3);
    assert_eq!(validation.status.import_shipments_count, 6);

    // Restore writable permission for cleanup
    let mut perms_w = std::fs::metadata(&valid_backup).unwrap().permissions();
    perms_w.set_readonly(false);
    std::fs::set_permissions(&valid_backup, perms_w).unwrap();

    // 2. Missing file rejected
    let missing_err = validate_backup_file(dir.join("missing.db")).unwrap_err();
    assert!(missing_err.to_string().contains("백업 파일"));

    // 3. 0-byte file rejected
    let empty_file = dir.join("empty.db");
    std::fs::write(&empty_file, b"").unwrap();
    assert!(validate_backup_file(&empty_file).is_err());

    // 4. Non-SQLite text file rejected
    let text_file = dir.join("not_sqlite.db");
    std::fs::write(&text_file, b"This is not a SQLite database file.").unwrap();
    assert!(validate_backup_file(&text_file).is_err());

    // 5. Corrupted SQLite file rejected by PRAGMA integrity_check
    let corrupt_file = dir.join("corrupt.db");
    let mut bytes = std::fs::read(&valid_backup).unwrap();
    assert!(bytes.len() > 8192);
    // Corrupt page 2 (beyond header) so SQLite opens it but PRAGMA integrity_check fails
    for b in &mut bytes[4096..4300] {
        *b ^= 0xFF;
    }
    std::fs::write(&corrupt_file, &bytes).unwrap();
    assert!(validate_backup_file(&corrupt_file).is_err());

    // 6. Valid SQLite DB missing required schema tables rejected
    let wrong_schema = dir.join("wrong_schema.db");
    let other_conn = Connection::open(&wrong_schema).unwrap();
    other_conn
        .execute_batch(
            "CREATE TABLE domestic_importers (id INTEGER PRIMARY KEY, name TEXT);
             CREATE TABLE unrelated_table (id INTEGER PRIMARY KEY);",
        )
        .unwrap();
    drop(other_conn);
    let schema_err = validate_backup_file(&wrong_schema).unwrap_err();
    let msg = schema_err.to_string();
    assert!(msg.contains("import_shipments") || msg.contains("필수 테이블"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn slice4_restore_database_creates_safety_backup_clears_wal_sidecars_and_swaps_atomically() {
    let dir = temp_dir_for("slice4");
    let active_path = dir.join("data.db");
    let backup_v1 = dir.join("backup_v1.db");
    let corrupt_file = dir.join("corrupt.db");
    let pre_restore_path = dir.join("data.pre-restore.db");

    // 1. Seed initial state and take backup_v1 (3 importers, 6 shipments)
    let mut conn = open_connection(&active_path).expect("open_connection");
    seed_sample_data(&mut conn, &active_path).expect("seed_sample_data");
    backup_database(&conn, &backup_v1).expect("backup_v1");

    // 2. Mutate active DB in WAL mode (4 importers, 7 shipments)
    conn.execute(
        "INSERT INTO domestic_importers (name) VALUES ('복원전임시수입사');",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO import_shipments (shipment_date, domestic_importer_id, overseas_branch_id, product_id, import_weight_kg_text, import_weight_kg_scaled)
         VALUES ('2026-11-14', 4, 1, 1, '999', 9990000);",
        [],
    )
    .unwrap();
    drop(conn);

    // 3. Reject corrupt candidate without touching active DB or creating data.pre-restore.db
    std::fs::write(&corrupt_file, b"not a valid sqlite file").unwrap();
    let bad_restore = restore_database(&active_path, &corrupt_file);
    assert!(bad_restore.is_err());
    assert!(!pre_restore_path.exists());

    // Confirm active DB still has 4 importers and 7 shipments
    {
        let check_conn = open_connection(&active_path).unwrap();
        let imp_count: i64 = check_conn
            .query_row("SELECT COUNT(*) FROM domestic_importers;", [], |r| r.get(0))
            .unwrap();
        let ship_count: i64 = check_conn
            .query_row("SELECT COUNT(*) FROM import_shipments;", [], |r| r.get(0))
            .unwrap();
        assert_eq!(imp_count, 4);
        assert_eq!(ship_count, 7);
    }

    // 4. Reject restoring active_path onto itself
    assert!(restore_database(&active_path, &active_path).is_err());

    // Simulate stale WAL/SHM sidecars left on disk before restore
    let wal_sidecar = dir.join("data.db-wal");
    let shm_sidecar = dir.join("data.db-shm");
    std::fs::write(&wal_sidecar, b"stale wal bytes").unwrap();
    std::fs::write(&shm_sidecar, b"stale shm bytes").unwrap();

    // 5. Perform valid restore
    let res = restore_database(&active_path, &backup_v1).expect("restore_database should succeed");
    assert_eq!(res.status.journal_mode.to_lowercase(), "wal");
    assert!(res.status.integrity_ok);
    assert_eq!(res.status.domestic_importers_count, 3);
    assert_eq!(res.status.import_shipments_count, 6);
    assert_eq!(
        std::path::PathBuf::from(&res.pre_restore_backup_path),
        pre_restore_path
    );
    assert!(pre_restore_path.exists());

    // Verify data.pre-restore.db preserved the pre-restore state (4 importers, 7 shipments)
    let pre_validation = validate_backup_file(&pre_restore_path).expect("pre-restore backup valid");
    assert_eq!(pre_validation.status.domestic_importers_count, 4);
    assert_eq!(pre_validation.status.import_shipments_count, 7);

    // Verify re-opened active DB is healthy in WAL mode with restored v1 data
    let reopened = open_connection(&active_path).expect("reopen active_path");
    let restored_shipments: i64 = reopened
        .query_row("SELECT COUNT(*) FROM import_shipments;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(restored_shipments, 6);
    let integrity: String = reopened
        .query_row("PRAGMA integrity_check;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");

    drop(reopened);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn slice5_windows_webview2_low_memory_and_build_config() {
    let policy = webview2_low_memory_policy();
    assert_eq!(
        policy.memory_usage_target_level,
        "CoreWebView2MemoryUsageTargetLevel::Low"
    );
    assert!(policy.additional_browser_args.contains("--max-old-space-size="));
    assert_eq!(policy.single_window_label, "main");

    let applied = apply_windows_webview2_low_memory_hook();
    assert_eq!(applied, policy);

    // Verify tauri.conf.json single-window, identifier, NSIS bundle, and low-memory args
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let tauri_conf = std::fs::read_to_string(repo_root.join("src-tauri/tauri.conf.json"))
        .expect("read tauri.conf.json");
    assert!(tauri_conf.contains("com.koreabranch.chemtracker"));
    assert!(tauri_conf.contains("\"nsis\""));
    assert!(tauri_conf.contains("\"label\": \"main\""));
    assert!(tauri_conf.contains("additionalBrowserArgs"));

    // Verify GitHub Actions windows-latest CI workflow
    let workflow = std::fs::read_to_string(repo_root.join(".github/workflows/windows-build.yml"))
        .expect("read windows-build.yml");
    assert!(workflow.contains("windows-latest"));
    assert!(workflow.contains("cargo test"));
    assert!(workflow.contains("x86_64-pc-windows-msvc"));
    assert!(workflow.contains("--bundles nsis"));
}
