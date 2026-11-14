//! Sidebar footer WAL database backup (`VACUUM INTO`), integrity-checked restore,
//! and Windows 11 WebView2 low-memory configuration helpers.

use crate::db::{get_db_status, open_connection, DbStatus, REQUIRED_TABLES};
use crate::domain::validate_calendar_date;
use crate::error::{AppError, AppResult};
use rusqlite::{Connection, OpenFlags};
use std::collections::HashSet;
use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const SQLITE_HEADER_MAGIC: &[u8; 16] = b"SQLite format 3\0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupResult {
    pub target_path: String,
    pub file_size_bytes: u64,
    pub status: DbStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupValidation {
    pub candidate_path: String,
    pub integrity_ok: bool,
    pub verified_tables: Vec<String>,
    pub status: DbStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreResult {
    pub pre_restore_backup_path: String,
    pub status: DbStatus,
}

/// Validates `candidate_path` read-only first, writes an automatic safety backup
/// (`data.pre-restore.db`) next to `active_db_path`, removes stale `-wal`/`-shm`
/// sidecars (assuming active handles to `active_db_path` are closed), atomically
/// swaps in the verified backup via a staging temp file, and re-opens in `WAL` mode.
pub fn restore_database(
    active_db_path: impl AsRef<Path>,
    candidate_path: impl AsRef<Path>,
) -> AppResult<RestoreResult> {
    let active = active_db_path.as_ref();
    let candidate = candidate_path.as_ref();

    if active.as_os_str().is_empty() || active.is_dir() {
        return Err(AppError::Validation(
            "활성 데이터베이스 경로가 올바르지 않습니다.".to_string(),
        ));
    }

    if same_target_file(active, candidate) {
        return Err(AppError::Validation(
            "현재 활성 데이터베이스 파일 자체는 복원 원본으로 선택할 수 없습니다.".to_string(),
        ));
    }

    // 1. Validate candidate strictly read-only before touching active DB or safety backup
    validate_backup_file(candidate)?;

    let parent = active.parent().unwrap_or_else(|| Path::new("."));
    let effective_parent = if parent.as_os_str().is_empty() {
        Path::new(".")
    } else {
        parent
    };
    std::fs::create_dir_all(effective_parent)?;

    // 2. Create automatic safety backup `data.pre-restore.db` next to active_db_path
    let pre_restore_path = effective_parent.join("data.pre-restore.db");
    if active.exists() {
        create_pre_restore_safety_backup(active, effective_parent, &pre_restore_path)?;
    }

    // 3. Stage verified backup in the same directory so rename is atomic on the same volume
    let staging_restore = unique_sibling_path(effective_parent, "chem_tracker_restore_staging", "tmp");
    if let Err(err) = std::fs::copy(candidate, &staging_restore) {
        let _ = std::fs::remove_file(&staging_restore);
        return Err(AppError::Io(err));
    }
    if let Ok(file) = std::fs::File::open(&staging_restore) {
        let _ = file.sync_all();
    }

    // 4. Remove stale WAL/SHM sidecars after active handles are closed
    remove_if_exists(&sidecar_path(active, "-wal"))?;
    remove_if_exists(&sidecar_path(active, "-shm"))?;

    // 5. Atomically swap staging file into active_db_path
    if let Err(err) = std::fs::rename(&staging_restore, active) {
        let _ = std::fs::remove_file(&staging_restore);
        return Err(AppError::Io(err));
    }

    // 6. Re-open in WAL mode and verify status
    let reopened = open_connection(active)?;
    let status = get_db_status(&reopened, active)?;
    if !status.journal_mode.eq_ignore_ascii_case("wal") || !status.integrity_ok {
        return Err(AppError::Validation(
            "복원 후 데이터베이스 WAL 모드 또는 무결성 확인에 실패했습니다.".to_string(),
        ));
    }

    Ok(RestoreResult {
        pre_restore_backup_path: pre_restore_path.display().to_string(),
        status,
    })
}

/// Convenience wrapper for managed-connection callers: drops the active `Connection`
/// before calling `restore_database` and re-opens it in `WAL` mode afterward.
pub fn restore_database_with_connection(
    active_conn: &mut Option<Connection>,
    active_db_path: impl AsRef<Path>,
    candidate_path: impl AsRef<Path>,
) -> AppResult<RestoreResult> {
    let active = active_db_path.as_ref();
    let candidate = candidate_path.as_ref();

    // Pre-validate before dropping the active connection so errors leave it intact
    validate_backup_file(candidate)?;

    *active_conn = None;
    let res = restore_database(active, candidate)?;
    *active_conn = Some(open_connection(active)?);
    Ok(res)
}

fn create_pre_restore_safety_backup(
    active: &Path,
    effective_parent: &Path,
    pre_restore_path: &Path,
) -> AppResult<()> {
    let staging = unique_sibling_path(effective_parent, "chem_tracker_prerestore_staging", "tmp");
    let vacuum_ok = (|| -> AppResult<()> {
        let conn = Connection::open(active)?;
        let staging_str = staging.to_str().ok_or_else(|| {
            AppError::Validation("유효한 UTF-8 파일 경로가 아닙니다.".to_string())
        })?;
        conn.execute("VACUUM INTO ?1;", [staging_str])?;
        Ok(())
    })();

    if vacuum_ok.is_err() {
        let _ = std::fs::remove_file(&staging);
        std::fs::copy(active, &staging)?;
    }

    if let Err(err) = std::fs::rename(&staging, pre_restore_path) {
        let _ = std::fs::remove_file(&staging);
        return Err(AppError::Io(err));
    }
    Ok(())
}

fn sidecar_path(db_path: &Path, suffix: &str) -> PathBuf {
    let mut os: OsString = db_path.as_os_str().to_os_string();
    os.push(suffix);
    PathBuf::from(os)
}

fn remove_if_exists(path: &Path) -> AppResult<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(AppError::Io(err)),
    }
}

/// Opens a candidate backup file strictly read-only (`SQLITE_OPEN_READ_ONLY`),
/// verifies the SQLite header, runs `PRAGMA integrity_check`, and confirms all
/// 6 required schema tables are present without modifying the active database.
pub fn validate_backup_file(candidate_path: impl AsRef<Path>) -> AppResult<BackupValidation> {
    let candidate = candidate_path.as_ref();
    if candidate.as_os_str().is_empty() || !candidate.exists() {
        return Err(AppError::Validation(format!(
            "백업 파일을 찾을 수 없습니다: {}",
            candidate.display()
        )));
    }
    if !candidate.is_file() {
        return Err(AppError::Validation(
            "백업 복원 대상으로 폴더가 아닌 .db 파일을 선택해 주세요.".to_string(),
        ));
    }

    let meta = std::fs::metadata(candidate)?;
    if meta.len() < 100 {
        return Err(AppError::Validation(
            "선택한 파일이 비어 있거나 올바른 SQLite 데이터베이스 파일(.db)이 아닙니다."
                .to_string(),
        ));
    }

    {
        let mut f = std::fs::File::open(candidate)?;
        let mut header = [0u8; 16];
        f.read_exact(&mut header)?;
        if &header != SQLITE_HEADER_MAGIC {
            return Err(AppError::Validation(
                "선택한 파일은 올바른 SQLite 데이터베이스 파일(.db)이 아닙니다.".to_string(),
            ));
        }
    }

    let conn = Connection::open_with_flags(candidate, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(
        |err| {
            AppError::Validation(format!(
                "백업 파일을 읽기 전용으로 열 수 없습니다: {err}"
            ))
        },
    )?;

    let integrity_rows: Vec<String> = {
        let mut stmt = conn.prepare("PRAGMA integrity_check;").map_err(|err| {
            AppError::Validation(format!(
                "백업 파일이 손상되어 무결성 검사를 실행할 수 없습니다: {err}"
            ))
        })?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|err| {
                AppError::Validation(format!(
                    "백업 파일 무결성 검사 중 오류가 발생했습니다: {err}"
                ))
            })?;
        let mut collected = Vec::new();
        for r in rows {
            collected.push(r.map_err(|err| {
                AppError::Validation(format!(
                    "백업 파일이 손상되어 무결성 검사 결과를 읽을 수 없습니다: {err}"
                ))
            })?);
        }
        collected
    };

    if integrity_rows.len() != 1 || !integrity_rows[0].eq_ignore_ascii_case("ok") {
        let detail = if integrity_rows.is_empty() {
            "결과 없음".to_string()
        } else {
            integrity_rows.join("; ")
        };
        return Err(AppError::Validation(format!(
            "백업 파일 무결성 검사(PRAGMA integrity_check)에 실패했습니다: {detail}"
        )));
    }

    let existing_tables: HashSet<String> = {
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table';")
            .map_err(|err| {
                AppError::Validation(format!(
                    "백업 파일 스키마를 조회할 수 없습니다: {err}"
                ))
            })?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut set = HashSet::new();
        for r in rows {
            set.insert(r?);
        }
        set
    };

    let mut missing = Vec::new();
    let mut verified_tables = Vec::with_capacity(REQUIRED_TABLES.len());
    for &req in REQUIRED_TABLES {
        if existing_tables.contains(req) {
            verified_tables.push(req.to_string());
        } else {
            missing.push(req);
        }
    }
    if !missing.is_empty() {
        return Err(AppError::Validation(format!(
            "올바른 화학물질 수입량 집계 DB 백업 파일이 아닙니다. 필수 테이블이 누락되었습니다: {}",
            missing.join(", ")
        )));
    }

    let status = get_db_status(&conn, candidate).map_err(|err| {
        AppError::Validation(format!(
            "백업 파일의 테이블 데이터를 읽을 수 없습니다(손상 또는 호환 불가): {err}"
        ))
    })?;

    Ok(BackupValidation {
        candidate_path: candidate.display().to_string(),
        integrity_ok: true,
        verified_tables,
        status,
    })
}

/// Executes `VACUUM INTO ?1` while the database remains live in `WAL` mode,
/// producing a standalone compacted SQLite file via a staging temp file.
pub fn backup_database(conn: &Connection, target_path: impl AsRef<Path>) -> AppResult<BackupResult> {
    let target = target_path.as_ref();
    if target.as_os_str().is_empty() {
        return Err(AppError::Validation(
            "백업 저장 경로가 비어 있습니다.".to_string(),
        ));
    }
    if target.is_dir() {
        return Err(AppError::Validation(
            "백업 저장 경로로 폴더가 아닌 .db 파일을 지정해 주세요.".to_string(),
        ));
    }

    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    let effective_parent = if parent.as_os_str().is_empty() {
        Path::new(".")
    } else {
        parent
    };
    std::fs::create_dir_all(effective_parent)?;

    let active_db_path = main_database_path(conn)?;
    if !active_db_path.as_os_str().is_empty() && same_target_file(&active_db_path, target) {
        return Err(AppError::Validation(
            "현재 사용 중인 활성 데이터베이스 파일 경로에는 백업을 저장할 수 없습니다. 다른 파일 이름을 선택해 주세요."
                .to_string(),
        ));
    }

    let staging_path = unique_sibling_path(effective_parent, "chem_tracker_vacuum_staging", "tmp");
    let staging_str = staging_path.to_str().ok_or_else(|| {
        AppError::Validation("유효한 UTF-8 파일 경로가 아닙니다.".to_string())
    })?;

    if let Err(err) = conn.execute("VACUUM INTO ?1;", [staging_str]) {
        let _ = std::fs::remove_file(&staging_path);
        return Err(AppError::Database(err));
    }

    if let Err(err) = std::fs::rename(&staging_path, target) {
        let _ = std::fs::remove_file(&staging_path);
        return Err(AppError::Io(err));
    }

    let file_size_bytes = std::fs::metadata(target)?.len();
    let status_path = if active_db_path.as_os_str().is_empty() {
        PathBuf::from(":memory:")
    } else {
        active_db_path
    };
    let status = get_db_status(conn, &status_path)?;

    Ok(BackupResult {
        target_path: target.display().to_string(),
        file_size_bytes,
        status,
    })
}

fn main_database_path(conn: &Connection) -> AppResult<PathBuf> {
    let mut stmt = conn.prepare("PRAGMA database_list;")?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == "main" {
            let file: String = row.get(2)?;
            return Ok(PathBuf::from(file));
        }
    }
    Ok(PathBuf::new())
}

fn same_target_file(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => {
            if let (Some(ap), Some(af), Some(bp), Some(bf)) =
                (a.parent(), a.file_name(), b.parent(), b.file_name())
            {
                let ap_eff = if ap.as_os_str().is_empty() { Path::new(".") } else { ap };
                let bp_eff = if bp.as_os_str().is_empty() { Path::new(".") } else { bp };
                if let (Ok(cap), Ok(cbp)) = (std::fs::canonicalize(ap_eff), std::fs::canonicalize(bp_eff)) {
                    return cap == cbp && af == bf;
                }
            }
            false
        }
    }
}

fn unique_sibling_path(dir: &Path, prefix: &str, ext: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    dir.join(format!(".{prefix}_{}_{}.{ext}", std::process::id(), nanos))
}

/// Formats the default SQLite backup filename `chem-tracker-backup-YYYYMMDD.db`
/// from a `YYYY-MM-DD` or `YYYYMMDD` calendar date string.
pub fn default_backup_filename(date: &str) -> AppResult<String> {
    let trimmed = date.trim();
    let iso = if trimmed.len() == 8 && trimmed.bytes().all(|b| b.is_ascii_digit()) {
        format!("{}-{}-{}", &trimmed[0..4], &trimmed[4..6], &trimmed[6..8])
    } else if trimmed.len() == 10 {
        trimmed.to_string()
    } else {
        return Err(AppError::Validation(
            "백업 파일명 생성을 위한 날짜 형식이 올바르지 않습니다 (YYYY-MM-DD 또는 YYYYMMDD)."
                .to_string(),
        ));
    };

    let (year, month, day) = validate_calendar_date(&iso)?;
    Ok(format!("chem-tracker-backup-{year:04}{month:02}{day:02}.db"))
}

/// Windows 11 WebView2 low-memory tuning policy (`CoreWebView2MemoryUsageTargetLevel::Low`).
///
/// Offline workspace note: the standalone `chem_tracker` library compiles without `tauri`
/// so `cargo test --offline` succeeds in airgapped environments. Once `tauri` is added to
/// `src-tauri/Cargo.toml`, wire this policy in the Tauri v2 desktop runner setup alongside
/// `WebViewExtWindows::set_memory_usage_level(MemoryUsageLevel::Low)` on the `"main"` window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebView2MemoryPolicy {
    pub memory_usage_target_level: &'static str,
    pub additional_browser_args: &'static str,
    pub single_window_label: &'static str,
}

pub const WINDOWS_WEBVIEW2_POLICY: WebView2MemoryPolicy = WebView2MemoryPolicy {
    memory_usage_target_level: "CoreWebView2MemoryUsageTargetLevel::Low",
    additional_browser_args: "--js-flags=--max-old-space-size=128",
    single_window_label: "main",
};

/// Returns the Windows 11 WebView2 low-memory configuration contract.
pub fn webview2_low_memory_policy() -> WebView2MemoryPolicy {
    WINDOWS_WEBVIEW2_POLICY
}

/// Applies the Windows `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` process environment hint
/// before WebView2 environment initialization.
///
/// Companion host call `WebViewExtWindows::set_memory_usage_level(MemoryUsageLevel::Low)`
/// is pending Tauri v2 desktop runner wiring in `src-tauri/src/main.rs`.
#[cfg(target_os = "windows")]
pub fn apply_windows_webview2_low_memory_hook() -> WebView2MemoryPolicy {
    if std::env::var_os("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS").is_none() {
        std::env::set_var(
            "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
            WINDOWS_WEBVIEW2_POLICY.additional_browser_args,
        );
    }
    WINDOWS_WEBVIEW2_POLICY
}

#[cfg(not(target_os = "windows"))]
pub fn apply_windows_webview2_low_memory_hook() -> WebView2MemoryPolicy {
    WINDOWS_WEBVIEW2_POLICY
}
