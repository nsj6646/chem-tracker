# Design Self-Service Recipe & Legacy Data Import + Chemical Master Seeding

Type: `grilling`
Label: `wayfinder:grilling`
Status: resolved
Blocked by: 04, 05

## Question

Now that [Define Customer's Existing Access Workflow (Tables, Queries, Forms, Reports)](./04-customer-access-workflow.md) established that:
1. [`docs/화학물질_20261109.xlsx`](../../../docs/화학물질_20261109.xlsx) (47,531 rows, 13 columns) is the reference **Chemical Substance Master (`화학물질 마스터`)**, and
2. **Product Recipes (`제품 구성성분`, `CAS번호` + `함유율%`)** and historical **Import Shipments (`수입실적`)** are confidential (`대외비`) so the Korea branch user must import or enter them directly on their Windows 11 PC without handing the raw data to the developer,

How should the application seed and update the 47,531-row Chemical Substance Master, and what exact self-service import format/workflow (e.g., Excel `.xlsx`/`.csv` template columns, validation rules for unmatched `CAS번호` or recipe percentages, and duplicate handling) should the app provide for migrating from Microsoft Access?

## Answer

1. **Zero Dependency on External NCIS Master Excel (Performance First)**:
   - The application **neither pre-bundles nor imports** the 47,531-row external NCIS master file (`화학물질_20261109.xlsx`). Because the target environment is a low-spec Windows 11 PC, runtime performance and lightweight footprint take top priority over non-essential bulk reference data.
   - Instead, **Chemical Substances (`화학물질`)** are managed directly by the customer in their own local database (migrated from their existing MS Access database or entered directly in the app), keyed uniquely by **`CAS번호` (`1 CAS번호 = 1 substance`)** with an optional **`화학물질명` (substance name)**.
   - Consequently, **Annual Substance Total (`연간 화학물질 총 수입량`)** queries and PDF/Excel reports display **`CAS번호` | `화학물질명(선택)` | `연간 총 수입량 (kg)` | `연간 총 수입량 (ton)`** without joining an external 47,531-row regulatory table.

2. **Self-Service Excel (`.xlsx` / `.csv`) Import Workflow & Templates**:
   - To migrate confidential data from MS Access without exposing it to the developer, the app provides downloadable standard `.xlsx` templates and two self-service bulk-import workflows backed by pure-Rust `calamine` / `csv`:
     - **Product & Recipe (BOM) Import**: `해외지사` | `수입제품코드` | `제품명(선택)` | `CAS번호` | `화학물질명(선택)` | `함유율(%)` (automatically creating new `해외지사`, `수입제품코드`, and `CAS번호` records).
     - **Import Shipment Import**: `수입일자(YYYY-MM-DD)` | `국내 수입사` | `해외지사` | `수입제품코드` | `제품 수입량(kg)` (automatically creating new `국내 수입사` records).
   - Every import displays a **Preview & Validation** step (showing total rows, newly created master records, and validation errors) before committing atomically in a single SQLite transaction.

3. **Validation & Re-Import Duplicate Handling**:
   - **Strict 100% Recipe Total**: Each component in a **Product Recipe (`제품 구성성분`)** must satisfy `0 < 함유율(%) <= 100`, and the sum of `함유율(%)` across all components of an **Imported Product (`수입제품코드`)** **must equal `100%` (`100.0`)** using exact decimal arithmetic (`rust_decimal`); recipes that do not sum to `100%` are rejected during form save and Excel import validation.
   - **Duplicate Handling on Re-Import**:
     - **Product Recipes (BOM)**: Re-importing an existing `수입제품코드` defaults to **replacing (overwriting)** that product's component list (with an option to skip existing products).
     - **Import Shipments**: Rows with an identical `(수입일자, 국내 수입사, 해외지사, 수입제품코드, 제품 수입량)` tuple are flagged as suspected duplicates in the preview and **skipped by default** (with an option to force insert).
