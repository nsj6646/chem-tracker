# 07: Live WAL Database Backup (`VACUUM INTO`), Integrity-Checked Restore & Windows 11 Cross-Build Pipeline

**What to build:** Deliver the sidebar footer database lifecycle actions—`DB 백업 저장 (.db)` and `DB 백업 불러오기 (복원)`—along with the macOS-to-Windows 11 cross-compilation configuration (`cargo-xwin` + `nsis`), Windows 11 WebView2 low-memory tuning (`CoreWebView2MemoryUsageTargetLevel::Low`), and the GitHub Actions `windows-latest` CI workflow for automated test and NSIS installer verification.

**Blocked by:** 01: Walking Skeleton — Desktop Shell, SQLite WAL Schema, Fixed-Point Core & Sample Data Seed

**Status:** resolved

- [x] Clicking `DB 백업 저장 (.db)` opens a native save file dialog (defaulting to `chem-tracker-backup-YYYYMMDD.db`) and executes `VACUUM INTO ?1` to create a standalone, compacted single-file SQLite backup while the active database remains online in `WAL` mode.
- [x] Clicking `DB 백업 불러오기 (복원)` opens a native file picker for a `.db` file, opens the candidate file read-only to verify `PRAGMA integrity_check` and the presence of all 6 required schema tables, writes an automatic safety backup of the current database (`data.pre-restore.db`), atomically swaps in the restored database, and refreshes the UI state.
- [x] Corrupt or schema-incompatible `.db` files selected for restore are rejected with a clear Korean error message without modifying or corrupting the active database.
- [x] Windows 11 WebView2 is configured with `CoreWebView2MemoryUsageTargetLevel::Low` for low-spec hardware, and the project builds cleanly for `x86_64-pc-windows-msvc` (`nsis` bundle) and includes a GitHub Actions `windows-latest` workflow running `cargo test` and building the Windows 11 NSIS installer (`*-setup.exe`). *(Verified on GitHub Actions `windows-latest` CI runs `#43573337113` and `#43573583192`: `cargo test` including `tauri_host` passed and NSIS `*-setup.exe` artifact built cleanly; physical Windows 11 PC smoke test remains manual.)*
