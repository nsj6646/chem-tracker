## Repo map

- **Tauri backend (`src-tauri/src/`)**: `lib.rs` (Tauri builder, `setup`, `generate_handler!`), `commands.rs` (24 `#[tauri::command]` wrappers), `db.rs` (SQLite WAL schema/seed), `domain.rs`/`decimal.rs`/`error.rs` (shared CAS/date/fixed-point helpers), domain modules (`masters.rs`, `products.rs`, `shipments.rs`, `excel_import.rs`, `reports.rs`, `backup.rs`).
- **Local workspace crates (`crates/`)**: in-repo overrides for `rust_decimal`, `csv`, `rust_xlsxwriter`, and `unicode_width` (see `docs/adr/0001-tech-stack.md#amendments-implemented-baseline-2026-11-14`).
- **React frontend (`src/`)**: `lib/ipc.ts` (Tauri/browser adapter & restore events), `workspaces/*Logic.ts` & `components/backupLogic.ts` (pure logic + IPC), `workspaces/*.tsx` & `App.tsx` (UI + `@media print`).
- **Tests & CI**: Rust integration tests in `src-tauri/tests/` (`ticket_01..07.rs`, `tauri_host.rs`, `code_review_fixes.rs`); Bun tests in `tests/` (`ticket-01..07.test.ts`, `code-review-fixes.test.ts`); Windows MSVC + NSIS CI in `.github/workflows/windows-build.yml` (artifacts in `target/x86_64-pc-windows-msvc/`).

## Checks & environment

- **Frontend checks**: `bun test && bunx tsc --noEmit && bun run build` (fall back to `~/.bun/bin/bun` when `bun` is not on `PATH`).
- **Backend checks**: `cargo test --workspace`. On macOS, `#[cfg(target_os = "windows")]` WebView2 COM/manifest paths are only compiled and tested on `windows-latest` CI unless `cargo-xwin` is installed.

## Agent skills

### Issue tracker

Issues and specs for new work live as GitHub issues in `nsj6646/chem-tracker` (via `gh` CLI). See `docs/agents/issue-tracker.md`.

`.scratch/access-replacement/` (`map.md`, `spec.md`, `tickets/01..07.md`) is an **archived** MVP design/ticket history (all tickets resolved on `main`); do not treat it as an active tracker or pass bare numbers to `/implement` expecting GitHub or handoff items.

### Triage labels

Default five canonical triage roles (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context layout (`GLOSSARY.md` and `docs/adr/` at the repo root). See `docs/agents/domain.md`.
