# Write Complete MVP Specification (`spec.md`)

Type: `grilling`
Label: `wayfinder:grilling`
Status: resolved
Blocked by: 07

## Question

Consolidate all decisions from the Wayfinder map ([Define Customer's Existing Access Workflow (Tables, Queries, Forms, Reports)](./04-customer-access-workflow.md), [Decide Tech Stack & Cross-Compilation Architecture (ADR)](./05-decide-tech-stack-and-architecture.md), [Design Self-Service Recipe & Legacy Data Import + Chemical Master Seeding](./06-self-service-data-import-and-master-seed.md), and [Prototype Chemical Import Volume Tracker UI & PDF Report](./07-prototype-ui-and-pdf-report.md)) into `.scratch/access-replacement/spec.md` covering:

1. **Domain Model & SQLite Schema**: Tables, primary/foreign keys, indexes, and fixed-point decimal rules (`rust_decimal`) for `국내 수입사`, `해외지사`, `화학물질`(`CAS번호`, `화학물질명`), `수입제품`, `제품 구성성분`(sum = `100%`), and `수입실적`.
2. **Screens & Workflows**: Product & Recipe (BOM) management, Import Shipment management, Self-Service Excel (`.xlsx`/`.csv`) Import with preview/validation, and single-file SQLite backup (`VACUUM INTO`).
3. **Queries & Reports**: Annual Substance Total (`연간 화학물질 총 수입량`, `kg` & `ton`) aggregation query, A4 PDF report (`@media print` / `PrintToPdf` external summary vs. internal detail), and `.xlsx` export (`rust_xlsxwriter`).
4. **Performance & Cross-Build Pipeline**: Low-spec Windows 11 memory/startup targets and macOS-to-Windows 11 `cargo-xwin` + `nsis` build & verification steps.

## Answer

Consolidated the complete MVP specification into [`.scratch/access-replacement/spec.md`](../spec.md):

1. **SQLite Schema, Constraints & Fixed-Point Arithmetic (`rust_decimal`)**:
   - Defined normalized SQLite tables (`domestic_importers`, `overseas_branches`, `chemical_substances`, `imported_products`, `product_components`, `import_shipments`) running in `WAL` mode (`synchronous = NORMAL`, `foreign_keys = ON`).
   - Enforced `UNIQUE (product_id, substance_id)` on `product_components` (disallowing duplicate `CAS번호` within a single product's BOM), `ON DELETE CASCADE` from `imported_products` to `product_components`, and `ON DELETE RESTRICT` across all shipment and master foreign keys to prevent accidental deletion of referenced master records or products with shipment history.
   - Locked fixed-point storage and precision rules: canonical decimal `TEXT` + 4-decimal scaled `INTEGER` ($\times 10{,}000$), up to 4 decimal places for `함유율(%)` (requiring exact `100.0000%` sum per product) and `제품 수입량(kg)`, exact unrounded `rust_decimal::Decimal` aggregation, and formatted output (`kg` to 2–4 decimals, `ton = kg / 1,000` to 4–5 decimals).
2. **Screens, Compact Master Editing & DB Lifecycle**:
   - Locked `Variant A` (Left Sidebar + 4-Tab Master-Detail Workspace), inline auto-creation of `국내 수입사`, `해외지사`, and `CAS번호` paired with compact `[관리/수정]` modals for fixing typos or deleting unreferenced master records, and bottom-anchored `DB 백업 저장 (.db)` (`VACUUM INTO`), `DB 백업 불러오기 (복원)` (`PRAGMA integrity_check` + atomic swap), and `샘플 데이터 적재`.
3. **Annual Substance Aggregation, A4 PDF & Mode-Aware `.xlsx` Export**:
   - Specified the `연간 화학물질 총 수입량` SQLite join + `rust_decimal` aggregation, `@media print` / `PrintToPdf` A4 report, and `rust_xlsxwriter` `.xlsx` export producing a 1-sheet summary workbook (`연간물질집계_요약`, omitting confidential `함유율%`) in **External Importer Summary Mode (`대외 제출용 요약본`)** and a 2-sheet workbook (`1_연간물질집계_요약` + `2_수입실적별_상세내역`) in **Internal Verification Detail Mode (`내부 검증용 상세본`)**, with `@` text formatting on `CAS번호`.
4. **Performance Budgets & Cross-Build Pipeline**:
   - Locked low-spec Windows 11 budgets (<10 MB installer, <1.0s cold start, ~120–200 MB RAM with `CoreWebView2MemoryUsageTargetLevel::Low`, <20 ms aggregation) and the macOS `cargo-xwin` + `nsis` build command and acceptance checklist.
