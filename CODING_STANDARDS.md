# Coding Standards

Judgement-call conventions for `chem-tracker` code authors and code reviewers.
Mechanical checks (`tsc --noEmit`, `bun test`, `vite build`, `tests/repo-guardrails.test.ts`, `cargo test --workspace`, `cargo clippy --workspace --all-targets`) are enforced by `bun run check` and CI and are intentionally omitted here.

## 1. Domain Vocabulary (`GLOSSARY.md`)

- Match canonical terms from [GLOSSARY.md](GLOSSARY.md) in new types, IPC field names, documentation, and UI copy (`Domestic Importer` / `국내 수입사`, `Overseas Branch` / `해외지사`, `Imported Product` / `수입제품`, `Product Recipe` / `제품 배합`, `Import Shipment` / `수입 이력`, `Chemical Substance` / `화학물질`, `Substance Import Weight`, `Annual Substance Total`).
- Flag avoided synonyms (`buyer`, `client`, `supplier`, `vendor`, `formula`, `mixture`, `delivery`, `transaction`, `chemical`, `material`, etc. per `GLOSSARY.md`) when introduced in user-facing labels or new domain APIs. Internal SQL table names (`product_components`, `import_shipments`) and existing legacy aliases listed in `GLOSSARY.md` remain as-is.

## 2. Shared Helpers & Exact Arithmetic

### Rust (`src-tauri/src/`)

- Reuse `crate::domain` (`normalize_cas_number`, `validate_calendar_date`, `days_in_month`, `upsert_overseas_branch`, `upsert_domestic_importer`, `upsert_chemical_substance`) instead of writing local normalization, date validation, or upsert queries in feature modules (`masters.rs`, `products.rs`, `shipments.rs`, `excel_import.rs`, `reports.rs`, `backup.rs`).
- When adding a small helper needed in more than one Rust module (for example optional string trimming), place it in `crate::domain` rather than duplicating private `fn trim_optional` helpers across files.
- Keep all weight, percentage, and report calculations in exact fixed-point integer arithmetic (`crate::decimal`: `parse_decimal_4dp`, `to_scaled_4dp`, `SCALE_FACTOR`, `HUNDRED_PERCENT_SCALED`, `rust_decimal`)—never introduce IEEE-754 float math for weights, percentages, or totals.
- Wrap multi-step writes, master deletes/updates, imports, and backup/restore operations in SQLite transactions via `AppState::with_conn` / `with_conn_mut`, preserving WAL mode and foreign-key constraints.

### TypeScript & React (`src/`)

- Reuse `src/lib/decimal.ts` (`parseScaled4dp`, `formatPct`, `formatBigIntFixed`, `roundHalfAway`, `normalizeCasNumber`, `validateIsoCalendarDate`) instead of duplicating parsing, rounding, CAS normalization, or date checks in workspace modules.
- Separate pure state/calculation/IPC logic in `src/workspaces/*Logic.ts` and `src/components/backupLogic.ts` (runnable in `bun test` without a DOM) from React rendering in `*.tsx`.
- Reuse `detectTauriInvoke` from `src/lib/ipc.ts` in feature IPC adapters and provide deterministic browser/test fallbacks where needed.
- Any workspace or component displaying database-backed state must accept `refreshKey` (`dbRevision` from `src/App.tsx`) and subscribe to `DB_RESTORED_EVENT` (`subscribeDbRestoredEvent`) so seeding or restoring a backup refreshes every view.

## 3. Windows & Cross-Platform Parity

- When adding or editing `#[cfg(target_os = "windows")]` blocks, `src-tauri/build.rs` linker flags, or Windows-specific WebView2/COM code, note that macOS `cargo test --workspace` and `cargo clippy` do not compile Windows-only branches. Keep Windows-conditional code minimal and well-isolated, and verify `.github/workflows/windows-build.yml` covers it.
- Keep build outputs workspace-relative (`target/x86_64-pc-windows-msvc/...`, not `src-tauri/target/...`) per [docs/adr/0001-tech-stack.md](docs/adr/0001-tech-stack.md#amendments-implemented-baseline-2026-11-14).

## 4. Module Size & Focus

- Keep new or refactored modules single-purpose and well under ~20 KB (~5,000 tokens).
- Do not pile unrelated logic onto already-large files (`src-tauri/src/excel_import.rs`, `src-tauri/src/reports.rs`, `src/workspaces/*Workspace.tsx`, `src/workspaces/excelImportLogic.ts`, `src/workspaces/annualReportLogic.ts`, `src-tauri/tests/ticket_05.rs`); split cohesive submodules or test files before or while extending them.
