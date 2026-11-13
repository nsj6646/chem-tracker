# Prototype Chemical Import Volume Tracker UI & PDF Report

Type: `prototype`
Label: `wayfinder:prototype`
Status: resolved
Blocked by: 04, 05, 06

## Question

Using the tech stack chosen in [Decide Tech Stack & Cross-Compilation Architecture (ADR)](./05-decide-tech-stack-and-architecture.md), the Korea branch domain workflow from [Define Customer's Existing Access Workflow (Tables, Queries, Forms, Reports)](./04-customer-access-workflow.md), and the customer-managed DB & import rules from [Design Self-Service Recipe & Legacy Data Import + Chemical Master Seeding](./06-self-service-data-import-and-master-seed.md), build a concrete lightweight prototype with synthetic sample recipes and shipments to validate:

1. **Product Recipe (`수입제품코드` + `CAS번호` + `화학물질명(선택)` + `함유율%`, enforcing exact `100%` total) & Import Shipment (`수입일자`, `국내 수입사`, `해외지사`, `제품 수입량 kg`)** form ergonomics and self-service Excel import preview.
2. **Annual Substance Total (`연간 화학물질 총 수입량`)** query view filtering by year and `국내 수입사`, showing `CAS번호`, `화학물질명(선택)`, and aggregated total `kg` and `ton` (with zero dependency on the external 47,531-row NCIS Excel file).
3. **Formatted A4 PDF Report generation** (default external/importer summary mode hiding confidential recipe percentages, plus optional internal product breakdown).

## Assets

- Interactive HTML Prototype (3 UI Variants + Portable Fixed-Point Domain Logic + `@media print` A4 PDF Report): [`prototype-ui-and-pdf-report.html`](../prototype-ui-and-pdf-report.html)

## Answer

Validated via [`prototype-ui-and-pdf-report.html`](../prototype-ui-and-pdf-report.html) and locked the following UI layout, form validation, and reporting decisions for the MVP specification:

1. **Primary Application Layout (`Variant A` Chosen)**:
   - **Left Sidebar Navigation + Tabbed Master-Detail Workspace** (classic MS Access desktop replacement feel) with 4 primary workspaces (`1. 수입제품 및 레시피(BOM)`, `2. 수입실적 관리`, `3. 엑셀 일괄 가져오기`, `4. 연간 물질 집계 & A4 보고서`) and a quick-access single-file SQLite backup action (`VACUUM INTO`) anchored at the bottom of the sidebar.
2. **Product Recipe (BOM) & Import Shipment Form Ergonomics**:
   - **Master-Detail Split Pane for BOM**: Selecting an `수입제품코드` on the left loads its confidential `제품 구성성분(BOM)` (`CAS번호` + optional `화학물질명` + `함유율%`) on the right with a live `합계 %` status pill and strict `100.00%` fixed-point validation (`rust_decimal` / 4-decimal scaled integer math) that blocks saving if the sum deviates from `100%`.
   - **Import Shipment Entry & Table**: Top quick-entry bar (`수입일자`, `국내 수입사`, `수입제품코드` with auto-linked `해외지사`, `제품 수입량 kg`) above a tabular history view showing both `kg` and converted `ton` (`= kg / 1,000`).
   - **Self-Service Excel (`.xlsx` / `.csv`) Import Preview**: Provides downloadable standard `.xlsx` templates for BOM and Shipments, pre-commit validation (blocking any product whose recipe does not sum to `100%`), toggleable overwrite-existing-BOM (`기본값: 덮어쓰기`), and automatic duplicate-tuple detection (`(수입일자, 국내 수입사, 해외지사, 수입제품코드, 제품 수입량)` `기본값: 건너뛰기`).
3. **Annual Substance Total (`연간 화학물질 총 수입량`) & Formatted A4 PDF Report**:
   - Filterable by **Year (`연도`)**, **Domestic Importer (`국내 수입사`)**, and **Overseas Branch (`해외지사`)**, aggregating exact fixed-point `Substance Import Weight` (`kg` and `ton`) per unique `CAS번호` (with optional `화학물질명` and expandable per-shipment drill-down, with zero dependency on the external 47,531-row NCIS Excel file).
   - **A4 PDF Report (`@media print` / `PrintToPdf`)**: Defaults to **External/Importer Summary Mode (`대외 제출용 요약본`)**, which hides confidential product recipes (`함유율%`) and prints only the report header metadata and `CAS번호` annual totals (`kg` / `ton`); includes an explicit toggle for **Internal Verification Detail Mode (`내부 검증용 상세본`)** that appends the per-product shipment and `함유율(%)` breakdown underneath each `CAS번호`.
