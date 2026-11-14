# Wayfinder Map: Lightweight Windows 11 Access Replacement

Label: `wayfinder:map`
Status: **Archived** (MVP implemented and merged to `main`; use GitHub Issues via `docs/agents/issue-tracker.md` for all new work)

## Destination

Lock the optimal lightweight tech stack (language, GUI framework, embedded DBMS, PDF/print & Excel libraries, and macOS-to-Windows 11 build/test pipeline) and produce a complete MVP specification (`spec.md`) for a custom business GUI application that replaces the customer's Microsoft Access workflow (database tables, queries, data-entry forms, and PDF/print + Excel reports) on a low-spec Windows 11 PC.

## Notes

- **Target User & Environment**: Single non-developer user on a low-spec Windows 11 PC (local single-file database, zero runtime setup friction, fast startup, low RAM/CPU footprint, full Korean IME support).
- **Developer Environment**: Built on macOS; must cross-compile or CI-build cleanly to Windows 11 `.exe` / installer.
- **Product Shape**: A custom business application tailored to the customer's specific data, forms, queries, and reports (not a generic no-code Access clone), migrating data once from legacy `.mdb`/`.accdb`.
- **Skills to consult**: `grilling`, `domain-modeling`, `research`, `prototype`.

## Decisions so far

<!-- the index: one line per closed ticket, enough to judge relevance, then zoom the link for the detail the ticket holds -->

- [Evaluate Lightweight GUI Frameworks for Windows 11 (developed on macOS)](./issues/01-gui-frameworks-and-cross-build.md): `Tauri v2` (Rust + built-in Win11 WebView2, ~120–250 MB RAM, 3–8 MB installer, native `cargo-xwin` + `nsis` cross-build) is safest for Korean IME, data grids, and printing; among pure-native Rust GUIs, `gpui` + `gpui-component` has the best table/form widgets (DirectX 11, ~40–90 MB RAM) but requires Windows `fxc.exe` for shaders and has custom Win32 Korean IME edge cases, while `iced` (`wgpu` + `tiny-skia` CPU fallback, ~20–45 MB RAM) cross-compiles cleanly from macOS ([research report](./research/01-gui-frameworks.md)).
- [Evaluate Embedded DBMS & MS Access Migration Path](./issues/02-embedded-dbms-and-migration.md): Use SQLite via synchronous `rusqlite` (`bundled`, `backup`) cross-compiled with `cargo-xwin`, `VACUUM INTO` for atomic single-file backups in WAL mode, and `mdbtools` (`--insert sqlite` / `mdb-queries`) or Jackcess for direct UTF-8 migration from `.mdb`/`.accdb` ([research report](./research/02-embedded-dbms-and-migration.md)).
- [Evaluate PDF/Print Report Generation & Excel Export Stack](./issues/03-reporting-pdf-print-and-excel.md): Use WebView2 CSS `@media print` + `window.print()` (if Tauri v2) or embedded `Typst` (`typst-as-lib` + `typst-pdf` with Windows `malgun.ttf`, 10–50 ms compile) for multi-page Korean PDF reports, and `rust_xlsxwriter` + `unicode-width` for fast (<200 ms), constant-memory `.xlsx` exports ([research report](./research/03-reporting-pdf-print-and-excel.md)).
- [Define Customer's Existing Access Workflow (Tables, Queries, Forms, Reports)](./issues/04-customer-access-workflow.md): Korea branch tracks **Domestic Importers (`국내 수입사`)**, **Overseas Branches (`해외지사`)**, **Imported Products (`수입제품코드`)** with confidential **Product Recipes (`CAS번호` + `함유율%`, self-imported by customer)**, and dated **Import Shipments (`수입일자`, `제품 수입량 kg`)** to aggregate **Annual Substance Totals (`kg`/`ton`)** by `CAS번호` and export formatted PDF reports (`GLOSSARY.md`).
- [Decide Tech Stack & Cross-Compilation Architecture (ADR)](./issues/05-decide-tech-stack-and-architecture.md): Lock in **Tauri v2** (Rust + Windows 11 WebView2 `Low` memory target + React 19 / TypeScript / `@tanstack/react-table` & `@tanstack/react-virtual` / Tailwind CSS), synchronous **`rusqlite`** (`bundled`, WAL mode, `VACUUM INTO` backups) + **`rust_decimal`** fixed-point arithmetic, **WebView2 `@media print` / `PrintToPdf`** for A4 Korean PDF reports, pure-Rust **`calamine` + `csv`** (self-service import) and **`rust_xlsxwriter` + `unicode-width`** (`.xlsx` export with `@` text formatting for `CAS번호`), cross-built from macOS via **`cargo-xwin` + `nsis`** ([`docs/adr/0001-tech-stack.md`](../../docs/adr/0001-tech-stack.md)).
- [Design Self-Service Recipe & Legacy Data Import + Chemical Master Seeding](./issues/06-self-service-data-import-and-master-seed.md): Exclude external 47,531-row NCIS master Excel bundling/import to maximize performance on low-spec Windows 11 PCs; manage customer's own **Chemical Substances (`CAS번호` + optional `화학물질명`)** directly in SQLite, provide downloadable `.xlsx` templates + preview/validation for self-importing **Product Recipes** and **Import Shipments**, enforce a strict `100%` sum of `함유율(%)` per product, and default to product-level overwrite for recipes and duplicate-row skip for shipments.
- [Prototype Chemical Import Volume Tracker UI & PDF Report](./issues/07-prototype-ui-and-pdf-report.md): Adopt **Variant A (Left Sidebar + 4-Tab Master-Detail Desktop Layout)** with bottom-anchored `VACUUM INTO` backup, master-detail BOM editor with live `100.00%` sum gauge/blocker, Excel `.xlsx` import preview (blocking non-`100%` BOMs, overwriting existing product BOMs by default, and skipping duplicate shipment tuples), and WebView2 `@media print` A4 PDF report defaulting to **External Importer Summary** (hiding confidential `함유율%`) with an **Internal Verification Detail** toggle ([interactive prototype](./prototype-ui-and-pdf-report.html)).
- [Write Complete MVP Specification (`spec.md`)](./issues/08-write-mvp-spec.md): Consolidate the locked Tauri v2 + `rusqlite` (WAL) + `rust_decimal` architecture, 6-table SQLite DDL (`UNIQUE(product_id, substance_id)`, `ON DELETE RESTRICT` guards, 4-decimal scaled fixed-point math), 4-workspace desktop UI with compact `[관리/수정]` modals and `VACUUM INTO` backup/restore, and mode-aware A4 PDF + 1-or-2-sheet `.xlsx` export into the final MVP specification ([`spec.md`](./spec.md)).

## Not yet specified

<!-- Map complete: destination reached and all tickets resolved. -->

## Out of scope

<!-- see "Out of scope": work ruled beyond the destination; closed, never graduates -->

- **Generic no-code Access builder**: End-user schema designer, arbitrary visual query builder, or drag-and-drop form/report designer.
- **Multi-user concurrent database server**: Networked DB server or real-time multi-PC sync (single local PC workflow only).
- **Two-way live sync with MS Access**: Ongoing `.accdb` read/write coexistence after the one-time migration.
- **Bundling or importing the 47,531-row external NCIS master Excel (`화학물질_20261109.xlsx`)**: Ruled out in [Design Self-Service Recipe & Legacy Data Import + Chemical Master Seeding](./issues/06-self-service-data-import-and-master-seed.md) because the customer manages their own chemical database directly and downloaded NCIS files are external reference material that would add unnecessary bloat/overhead on a low-spec Windows 11 PC.
