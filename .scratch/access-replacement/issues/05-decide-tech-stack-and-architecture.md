# Decide Tech Stack & Cross-Compilation Architecture (ADR)

Type: `grilling`
Label: `wayfinder:grilling`
Status: resolved
Blocked by: 01, 02, 03

## Question

Based on the findings from:
- [Evaluate Lightweight GUI Frameworks for Windows 11 (developed on macOS)](./01-gui-frameworks-and-cross-build.md) (`01`)
- [Evaluate Embedded DBMS & MS Access Migration Path](./02-embedded-dbms-and-migration.md) (`02`)
- [Evaluate PDF/Print Report Generation & Excel Export Stack](./03-reporting-pdf-print-and-excel.md) (`03`)

Which combination of language, GUI framework, embedded DBMS, reporting/Excel libraries, and macOS-to-Windows 11 build/test workflow should we lock in? Record the resulting architectural decision in `docs/adr/0001-tech-stack.md`.

## Answer

Recorded in [`docs/adr/0001-tech-stack.md`](../../../docs/adr/0001-tech-stack.md):

1. **Core GUI & Application Architecture**: **Tauri v2** (Rust backend + Windows 11 built-in WebView2 in single-window mode with `CoreWebView2MemoryUsageTargetLevel::Low`) paired with a lightweight **React 19 + TypeScript + Vite + `@tanstack/react-table` & `@tanstack/react-virtual` + Tailwind CSS** frontend. All 47,531-row `화학물질 마스터` lookups and annual aggregations run in Rust/SQLite, while the frontend renders only virtualized viewport rows (~30–50 DOM nodes) and relies on WebView2's native Windows TSF for bug-free Korean IME input.
2. **Embedded DBMS, Precision Arithmetic & Backups**: Synchronous **`rusqlite`** (`features = ["bundled", "backup", "functions"]`) running in `WAL` mode (`synchronous = NORMAL`, `foreign_keys = ON`), storing the database in `%LOCALAPPDATA%\<AppName>\data.db` or a user-selected local folder. Exact decimal arithmetic via **`rust_decimal`** (and fixed-point integer/decimal representation in SQLite) prevents floating-point drift in `제품 수입량(kg) × 함유율(%)`, and **`VACUUM INTO`** provides one-click atomic single-file backups.
3. **PDF Reporting & Printing**: **WebView2 CSS `@media print` (`@page`, repeating `<thead>`/`<tfoot>`) + `window.print()` / WebView2 `PrintToPdf`**, adding 0 MB to the binary, sharing a single template between on-screen preview and print/PDF output, and using Windows 11's native `Malgun Gothic` (`맑은 고딕`) font.
4. **Excel I/O & Self-Service Legacy Import**: Pure-Rust **`calamine`** (`.xlsx`/`.xls` reader) and **`csv`** for seeding/updating `화학물질_20261109.xlsx` and self-importing confidential customer recipes/shipments without bundling C `mdbtools` DLLs or a JVM (with an optional standalone `mdbtools` CLI script if direct `.mdb`/`.accdb` conversion is ever needed). Pure-Rust **`rust_xlsxwriter` + `unicode-width`** (`constant_memory` mode, explicit `@` text format for `CAS번호` and `KE번호`, Hangul 2-unit column width auto-fit) for `.xlsx` exports.
5. **macOS-to-Windows 11 Cross-Build Pipeline**: Local cross-compilation on macOS via **`cargo-xwin` + `nsis`** (`cargo tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis`) producing a standalone `*-setup.exe` installer and portable `.exe`, supplemented by a GitHub Actions `windows-latest` CI workflow.
