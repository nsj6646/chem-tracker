# MVP Specification: Chemical Import Volume Tracker (Windows 11 Access Replacement)

## 1. Overview & Architecture

### 1.1 Purpose & Scope

The **Chemical Import Volume Tracker (`화학물질 수입량 집계 시스템`)** is a lightweight, single-user desktop database application for the **Korea branch office (`코리아 지사`)**. It replaces a legacy Microsoft Access (`.mdb`/`.accdb`) workflow on a low-spec Windows 11 PC, enabling the user to:

1. Manage **Overseas Branches (`해외지사`)**, **Domestic Importers (`국내 수입사`)**, **Chemical Substances (`화학물질`: `CAS번호` + optional `화학물질명`)**, and **Imported Products (`수입제품`)** with confidential **Product Recipes (`제품 구성성분` / BOM: `CAS번호` + `함유율 %`, summing to exactly `100%`)**.
2. Record and bulk-import dated **Import Shipments (`수입실적`: `수입일자`, `국내 수입사`, `해외지사`, `수입제품코드`, `제품 수입량 kg`)**.
3. Aggregate **Annual Substance Totals (`연간 화학물질 총 수입량`)** in `kg` and `ton` (`= kg / 1,000`) per `국내 수입사` and `CAS번호` using exact fixed-point decimal arithmetic.
4. Generate formatted A4 PDF/print reports (defaulting to an **External Importer Summary** that hides confidential recipe percentages, with an optional **Internal Verification Detail** mode) and export `.xlsx` workbooks.

> **Out of Scope**:
> - Bundling or importing the 47,531-row external NCIS master Excel file (`화학물질_20261109.xlsx`). All chemical substances are managed directly in the customer's local SQLite database for maximum performance on low-spec hardware.
> - Generic no-code schema/query/form builders, multi-user network sync, or ongoing two-way live sync with MS Access.

### 1.2 Locked Technology Stack ([ADR-0001](../../docs/adr/0001-tech-stack.md))

| Layer | Technology | Rationale & Configuration |
| :--- | :--- | :--- |
| **Desktop Shell** | **Tauri v2** (Rust + Windows 11 Edge WebView2) | Uses OS-bundled WebView2 (3–8 MB installer), single-window architecture, configured with `CoreWebView2MemoryUsageTargetLevel::Low` and native Windows TSF Korean IME support. |
| **Frontend UI** | **React 19 + TypeScript + Vite + Tailwind CSS** | Virtualized grids via `@tanstack/react-table` & `@tanstack/react-virtual` (~30–50 DOM nodes rendered at once). |
| **Embedded DBMS** | **SQLite** via synchronous **`rusqlite`** | Features: `["bundled", "backup", "functions"]`. Runs in `WAL` mode (`synchronous = NORMAL`, `foreign_keys = ON`). Zero async runtime overhead on DB queries. |
| **Decimal Math** | **`rust_decimal`** | 128-bit fixed-point decimal arithmetic (up to 4 decimal places for input, zero IEEE-754 floating-point drift). |
| **Excel Import** | **`calamine` + `csv`** | Pure-Rust `.xlsx`/`.xls`/`.csv` reader for self-service migration of confidential recipes and shipments. |
| **Excel Export** | **`rust_xlsxwriter` + `unicode-width`** | Pure-Rust `.xlsx` writer (`constant_memory` mode), `@` text format for `CAS번호`, and Hangul 2-unit column width auto-fit. |
| **PDF & Print** | **WebView2 CSS `@media print` + `window.print()` / `PrintToPdf`** | 0 MB binary overhead, uses Windows 11 native `Malgun Gothic` (`맑은 고딕`) font, shared React component for preview and print/PDF. |
| **Cross-Build** | **`cargo-xwin` + `nsis`** (macOS $\to$ Windows 11) | `cargo tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis` + GitHub Actions `windows-latest` CI. |

---

## 2. Domain Model, SQLite Schema & Fixed-Point Decimal Rules

### 2.1 Canonical Domain Terms ([`GLOSSARY.md`](../../GLOSSARY.md))

- **Domestic Importer (`국내 수입사`)**: South Korean company importing products; annual substance totals are tracked per Domestic Importer.
- **Overseas Branch (`해외지사`)**: Overseas manufacturing affiliate/branch supplying products.
- **Chemical Substance (`화학물질`)**: Chemical component keyed uniquely by `CAS번호` (`1 CAS번호 = 1 substance`) with an optional `화학물질명`.
- **Imported Product (`수입제품`)**: Commercial mixture identified by `수입제품코드` and supplied by an `해외지사`.
- **Product Recipe (`제품 구성성분` / BOM)**: Confidential list of `(CAS번호, 함유율 %)` for an `수입제품`, where `함유율 (%)` across a product must sum to exactly `100%`.
- **Import Shipment (`수입실적`)**: Dated shipment record (`수입일자`, `국내 수입사`, `해외지사`, `수입제품코드`, `제품 수입량 kg`).
- **Substance Import Weight (`화학물질 환산 중량`)**: $\text{Product Import Weight (kg)} \times (\text{Weight Percentage} / 100)$, expressed in `kg` and `ton` (`= kg / 1,000`).
- **Annual Substance Total (`연간 화학물질 총 수입량`)**: Aggregated sum (`SUM`) of `Substance Import Weight` (`kg` and `ton`) per unique `CAS번호` for a `국내 수입사` within a calendar year/period.

### 2.2 SQLite Database Location & Connection Pragmas

- **Database Path**: `%LOCALAPPDATA%\com.koreabranch.chemtracker\data.db` on Windows 11 (via Tauri `app_local_data_dir()`).
- **Initialization Pragmas** (executed on every connection open):
  ```sql
  PRAGMA journal_mode = WAL;
  PRAGMA synchronous = NORMAL;
  PRAGMA foreign_keys = ON;
  PRAGMA temp_store = MEMORY;
  PRAGMA cache_size = -8000; -- ~8 MB page cache budget for low-spec PC
  ```

### 2.3 SQLite Schema (DDL)

Decimal values are stored both as canonical normalized decimal strings (`TEXT`, parsed losslessly by `rust_decimal::Decimal`) and as 4-decimal scaled integers (`INTEGER`, value $\times 10{,}000$, e.g., `100.0000%` = `1_000_000`) to enable exact integer `SUM` checks in SQLite triggers/queries alongside `rust_decimal` verification in Rust.

```sql
-- 1. Domestic Importers (국내 수입사)
CREATE TABLE IF NOT EXISTS domestic_importers (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL UNIQUE COLLATE NOCASE,
    note        TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- 2. Overseas Branches (해외지사)
CREATE TABLE IF NOT EXISTS overseas_branches (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL UNIQUE COLLATE NOCASE,
    note        TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- 3. Chemical Substances (화학물질 — customer-managed, 1 CAS번호 = 1 substance)
CREATE TABLE IF NOT EXISTS chemical_substances (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    cas_number      TEXT NOT NULL UNIQUE, -- Trimmed, normalized e.g. '50-00-0'
    substance_name  TEXT,                 -- Optional Korean/English name (화학물질명)
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- 4. Imported Products (수입제품)
CREATE TABLE IF NOT EXISTS imported_products (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    product_code        TEXT NOT NULL UNIQUE COLLATE NOCASE, -- 수입제품코드
    product_name        TEXT,                                -- 제품명 (선택)
    overseas_branch_id  INTEGER NOT NULL,
    created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    FOREIGN KEY (overseas_branch_id) REFERENCES overseas_branches(id) ON DELETE RESTRICT
);

-- 5. Product Recipe / BOM Components (제품 구성성분)
CREATE TABLE IF NOT EXISTS product_components (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id              INTEGER NOT NULL,
    substance_id            INTEGER NOT NULL,
    weight_pct_text         TEXT NOT NULL,    -- Canonical decimal string e.g. '35.25'
    weight_pct_scaled       INTEGER NOT NULL, -- Scaled by 10,000 (1% = 10,000; 100% = 1,000,000)
    CHECK (weight_pct_scaled > 0 AND weight_pct_scaled <= 1000000),
    UNIQUE (product_id, substance_id),        -- Disallow duplicate CAS번호 within the same product
    FOREIGN KEY (product_id) REFERENCES imported_products(id) ON DELETE CASCADE,
    FOREIGN KEY (substance_id) REFERENCES chemical_substances(id) ON DELETE RESTRICT
);

-- 6. Import Shipments (수입실적)
CREATE TABLE IF NOT EXISTS import_shipments (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    shipment_date           TEXT NOT NULL,    -- ISO-8601 date 'YYYY-MM-DD' (수입일자)
    shipment_year           INTEGER GENERATED ALWAYS AS (CAST(substr(shipment_date, 1, 4) AS INTEGER)) STORED,
    domestic_importer_id    INTEGER NOT NULL,
    overseas_branch_id      INTEGER NOT NULL,
    product_id              INTEGER NOT NULL,
    import_weight_kg_text   TEXT NOT NULL,    -- Canonical decimal string e.g. '1250.5'
    import_weight_kg_scaled INTEGER NOT NULL, -- Scaled by 10,000 (1 kg = 10,000)
    note                    TEXT,
    created_at              TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at              TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    CHECK (import_weight_kg_scaled > 0),
    FOREIGN KEY (domestic_importer_id) REFERENCES domestic_importers(id) ON DELETE RESTRICT,
    FOREIGN KEY (overseas_branch_id) REFERENCES overseas_branches(id) ON DELETE RESTRICT,
    FOREIGN KEY (product_id) REFERENCES imported_products(id) ON DELETE RESTRICT
);

-- Indexes for fast filtering & aggregation
CREATE INDEX IF NOT EXISTS idx_imported_products_branch
    ON imported_products(overseas_branch_id);
CREATE INDEX IF NOT EXISTS idx_product_components_product
    ON product_components(product_id);
CREATE INDEX IF NOT EXISTS idx_product_components_substance
    ON product_components(substance_id);
CREATE INDEX IF NOT EXISTS idx_shipments_year_importer
    ON import_shipments(shipment_year, domestic_importer_id);
CREATE INDEX IF NOT EXISTS idx_shipments_date
    ON import_shipments(shipment_date);
CREATE INDEX IF NOT EXISTS idx_shipments_product
    ON import_shipments(product_id);
```

### 2.4 Referential Integrity & Deletion Policy

1. **Unique `CAS번호` Within a Product**: `UNIQUE (product_id, substance_id)` on `product_components` prevents the same `CAS번호` from appearing twice in one product's BOM. Both the BOM editor and Excel importer reject duplicate `CAS번호` rows within the same `수입제품코드`.
2. **Deleting an `Imported Product` (`수입제품`)**:
   - Allowed only if zero rows in `import_shipments` reference `product_id` (`ON DELETE RESTRICT`). If shipments exist, the backend returns a clear Korean error: `"해당 수입제품으로 등록된 수입실적이 N건 존재하여 삭제할 수 없습니다. 수입실적을 먼저 삭제하거나 수정해 주세요."`
   - When no shipments reference the product, deleting the product automatically cascades (`ON DELETE CASCADE`) to its `product_components`.
3. **Deleting Master Entities (`국내 수입사`, `해외지사`, `화학물질`)**:
   - Guarded by `ON DELETE RESTRICT`. If referenced by any product, recipe component, or shipment, deletion is blocked with an explicit reference count message. Renaming/updating (`name`, `substance_name`, `cas_number`) in place via the Master Management modal updates all referencing views automatically via foreign key IDs.

### 2.5 Fixed-Point Decimal & Rounding Rules (`rust_decimal`)

| Value | Input Precision | Exact Calculation Rule (`rust_decimal::Decimal`) | Display & Export Formatting |
| :--- | :--- | :--- | :--- |
| **`함유율 (%)`** | Up to **4 decimal places** (`0.0001` – `100.0000`) | Sum of `weight_pct_scaled` across all components of a product **must equal `1_000_000` (`100.0000%`)**. Any deviation blocks saving/importing. | Minimum 2 decimals, up to 4 significant decimals (e.g., `35.00%`, `12.345%`). |
| **`제품 수입량 (kg)`** | Up to **4 decimal places** (`> 0`) | Stored as normalized `Decimal` text and `× 10,000` scaled `i64`. | Comma-grouped, minimum 2 decimals, up to 4 significant decimals (e.g., `1,250.50`). |
| **`화학물질 환산 중량 (kg)`** | Exact up to **8 decimal places** internally | $\text{Substance kg} = \text{Import Weight kg} \times (\text{Weight Pct} / 100)$ with zero intermediate rounding. | Rounded half-up (`MidpointAwayFromZero`) to **2 decimal places** (or up to **4 decimals** if `< 0.01 kg` or fractional digits exist). |
| **`연간 총 수입량 (kg)`** | Exact sum of unrounded `Substance kg` across shipments | $\sum \text{exact Substance kg}$, rounded only at final display/export. | Comma-grouped, **2 decimal places** (up to **4 decimals** for non-zero trailing precision). |
| **`환산 / 연간 총 수입량 (ton)`** | Exact division by `1,000` | $\text{Substance ton} = (\sum \text{exact Substance kg}) / 1000$. | Comma-grouped, **4 decimal places** (up to **5 decimals** if needed to avoid rounding small non-zero quantities to `0.0000`). |

---

## 3. Screens, UI Layout & Workflows

### 3.1 Primary Application Layout (`Variant A` — Desktop Sidebar + 4 Workspaces)

Validated in [`prototype-ui-and-pdf-report.html`](./prototype-ui-and-pdf-report.html):

- **Left Sidebar Navigation (Fixed width `240px`, hidden in `@media print`)**:
  - Application title (`화학물질 수입량 집계 시스템 — 코리아 지사`) & active DB status indicator.
  - **4 Primary Navigation Tabs**:
    1. `1. 수입제품 및 레시피(BOM)` (Products & Confidential Recipes)
    2. `2. 수입실적 관리` (Import Shipments)
    3. `3. 엑셀 일괄 가져오기` (Self-Service Excel Import)
    4. `4. 연간 물질 집계 & A4 보고서` (Annual Substance Totals, PDF Print & Excel Export)
  - **Bottom-Anchored Database Management Panel**:
    - `DB 백업 저장 (.db)` (`VACUUM INTO`)
    - `DB 백업 불러오기 (복원)` (`PRAGMA integrity_check` + atomic DB swap)
    - `샘플 데이터 적재` (populates synthetic test dataset when testing on an empty DB)

### 3.2 Workspace 1: Product & Recipe (BOM) Management (`1. 수입제품 및 레시피(BOM)`)

- **Left Pane — Imported Product Master List**:
  - Search input (`수입제품코드`, `제품명`, `해외지사` filter) + `+ 신규 수입제품 등록` button.
  - Table displaying `수입제품코드`, `제품명(선택)`, `해외지사`, component count (`구성 물질 수`), and recipe validity badge (`100.00% 정상`).
- **Right Pane — Confidential Product Recipe (`제품 구성성분`) Editor**:
  - Header fields: `수입제품코드` (required), `제품명` (optional), `해외지사` selector (with inline `+ 새 해외지사 추가` and a **`[관리/수정]`** button opening the compact Master Edit Modal).
  - **Live Total Percentage Gauge**: Displays $\sum \text{함유율(\%)}$ in real time:
    - Green badge (`합계: 100.00% — 저장 가능`) when `sum == 100.0000%`.
    - Red blocker banner (`합계: XX.XX% — 합계가 정확히 100%여야 저장할 수 있습니다 (차이: ±Y.YY%)`) when `sum != 100.0000%`.
  - **BOM Component Grid**:
    - Columns: `CAS번호` (autocomplete from existing `chemical_substances` or type a new `CAS번호` to create automatically), `화학물질명 (선택)` (auto-filled if known, editable inline), `함유율 (%)` (numeric input up to 4 decimals), and `삭제` button.
    - Includes a **`[화학물질 관리/수정]`** button to fix typos in `CAS번호` / `화학물질명` or delete unused substances in a compact modal.
  - **Save Action (`save_product_with_bom` Tauri command)**:
    - Validates inside a single SQLite transaction that:
      1. `product_code` and `overseas_branch` are non-empty.
      2. At least 1 component row exists, all `0 < weight_pct <= 100`, no duplicate `CAS번호` exists within the product, and $\sum \text{weight\_pct} == 100$.
      3. Upserts `overseas_branches`, `chemical_substances`, `imported_products`, and replaces `product_components`.

### 3.3 Workspace 2: Import Shipment Management (`2. 수입실적 관리`)

- **Top Quick-Entry / Edit Form Bar**:
  - Inputs:
    - `수입일자` (`YYYY-MM-DD` date picker + keyboard input)
    - `국내 수입사` (combobox with inline creation + **`[수입사 관리/수정]`** modal button)
    - `수입제품코드` (combobox; selecting a product automatically populates and locks its linked `해외지사`)
    - `제품 수입량 (kg)` (decimal input up to 4 decimals, showing live converted `ton` preview)
    - `비고 (선택)`
  - Keyboard-friendly flow (`Tab` $\to$ `Enter` to submit immediately for rapid multi-row entry).
- **Filter & Summary Strip**:
  - Filter by `연도`, `국내 수입사`, `해외지사`, and `수입제품코드` search text.
  - Displays total matching shipment count, sum of `제품 수입량 (kg)`, and sum of `제품 수입량 (ton)`.
- **Virtualized Shipment History Table (`@tanstack/react-virtual`)**:
  - Columns: `수입일자` | `국내 수입사` | `해외지사` | `수입제품코드` (`제품명`) | `제품 수입량 (kg)` | `환산 중량 (ton)` | `비고` | `수정 / 삭제`.

### 3.4 Workspace 3: Self-Service Excel (`.xlsx` / `.csv`) Import (`3. 엑셀 일괄 가져오기`)

Designed so the Korea branch user can migrate confidential recipes and shipment histories directly from MS Access Excel exports without exposing raw data to the developer:

1. **Template Download (`download_import_template` Tauri command)**:
   - **Product & Recipe (BOM) Template (`제품구성성분_BOM_양식.xlsx`)**:
     - Columns: `해외지사` | `수입제품코드` | `제품명(선택)` | `CAS번호` | `화학물질명(선택)` | `함유율(%)`
     - `CAS번호` column pre-formatted with Excel `@` text format so inputs like `50-00-0` are never converted to dates.
   - **Import Shipment Template (`수입실적_양식.xlsx`)**:
     - Columns: `수입일자(YYYY-MM-DD)` | `국내 수입사` | `해외지사` | `수입제품코드` | `제품 수입량(kg)` | `비고(선택)`
2. **Parse, Preview & Pre-Commit Validation (`preview_excel_import` Tauri command)**:
   - Reads `.xlsx` / `.xls` / `.csv` via `calamine` / `csv`. Normalizes cell types (handling both text and numeric cells, trimming whitespace, normalizing dates).
   - **For BOM Import**:
     - Groups rows by `수입제품코드`.
     - Validates each product's component sum ($\sum \text{함유율(\%)} == 100$) and checks for duplicate `CAS번호` within the same product or missing required fields.
     - Identifies existing `수입제품코드` records in SQLite and provides the toggle: **`기존 수입제품 레시피 덮어쓰기 (기본값: 켜짐)`** vs. **`기존 제품 건너뛰기`**.
     - **Blocking Rule**: If any product in the uploaded file fails validation (e.g., sum $\neq 100\%$), the commit button is disabled until the file is fixed, and offending products/rows are highlighted in red with exact error descriptions.
   - **For Shipment Import**:
     - Validates `수입일자` (`YYYY-MM-DD`), positive `제품 수입량(kg)`, and verifies that each referenced `수입제품코드` exists in `imported_products` (or fails with a clear error listing unknown product codes).
     - Detects suspected duplicate shipments matching the 5-tuple `(수입일자, 국내 수입사, 해외지사, 수입제품코드, 제품 수입량)` (either against existing DB rows or within the same upload batch) and provides the toggle: **`중복 수입실적 건너뛰기 (기본값: 켜짐)`** vs. **`중복 포함 강제 추가`**.
3. **Atomic Commit (`commit_excel_import` Tauri command)**:
   - Executes all inserts/overwrites inside a single `BEGIN IMMEDIATE ... COMMIT` SQLite transaction.

### 3.5 Compact Master Management Modal (`[관리/수정]`) & Database Backup/Restore

- **Master Management Modal**:
  - Accessible from the `[관리/수정]` button next to `국내 수입사`, `해외지사`, and `화학물질` selectors.
  - Supports inline renaming (`name` / `cas_number` / `substance_name`) and deleting unreferenced items (showing the current reference count so the user immediately sees why an active item cannot be deleted).
- **Database Backup & Restore (Sidebar Footer)**:
  - **`DB 백업 저장 (.db)`**: Opens native Windows save file dialog (default filename: `chem-tracker-backup-YYYYMMDD.db`) and executes `VACUUM INTO ?1` to produce a consistent, compacted single-file SQLite backup while live in `WAL` mode.
  - **`DB 백업 불러오기 (복원)`**: Opens native file picker for a `.db` file, opens it read-only to verify `PRAGMA integrity_check` and required schema tables, creates an automatic safety backup of the current DB (`data.pre-restore.db`), and replaces the active database.

---

## 4. Queries, PDF Reporting & Excel Export

### 4.1 Annual Substance Total (`연간 화학물질 총 수입량`) Query & Calculation

In Workspace 4 (`4. 연간 물질 집계 & A4 보고서`), the user filters by:
- **Target Year (`대상 연도`, required, e.g., `2026`)** or custom date range (`시작일 ~ 종료일`)
- **Domestic Importer (`국내 수입사`, required or `전체 수입사` option)**
- **Overseas Branch (`해외지사`, optional, defaults to `전체`)**
- **Imported Product (`수입제품코드`, optional, defaults to `전체`)**

**Execution Flow (`query_annual_substance_totals` Tauri command)**:
1. Query matching shipment + component rows joined across `import_shipments`, `imported_products`, `product_components`, `chemical_substances`, `domestic_importers`, and `overseas_branches`:
   ```sql
   SELECT
       s.id AS shipment_id,
       s.shipment_date,
       di.name AS domestic_importer_name,
       ob.name AS overseas_branch_name,
       ip.product_code,
       ip.product_name,
       s.import_weight_kg_text,
       cs.id AS substance_id,
       cs.cas_number,
       cs.substance_name,
       pc.weight_pct_text
   FROM import_shipments s
   JOIN domestic_importers di ON di.id = s.domestic_importer_id
   JOIN overseas_branches ob  ON ob.id = s.overseas_branch_id
   JOIN imported_products ip  ON ip.id = s.product_id
   JOIN product_components pc ON pc.product_id = ip.id
   JOIN chemical_substances cs ON cs.id = pc.substance_id
   WHERE s.shipment_year = ?1
     AND (?2 IS NULL OR s.domestic_importer_id = ?2)
     AND (?3 IS NULL OR s.overseas_branch_id = ?3)
     AND (?4 IS NULL OR s.product_id = ?4)
   ORDER BY cs.cas_number ASC, s.shipment_date ASC, s.id ASC;
   ```
2. In Rust using `rust_decimal::Decimal`:
   - For each row, compute exact `substance_weight_kg = import_weight_kg * (weight_pct / Decimal::ONE_HUNDRED)` and `substance_weight_ton = substance_weight_kg / Decimal::ONE_THOUSAND`.
   - Accumulate per `cas_number`:
     - `total_substance_kg = SUM(substance_weight_kg)`
     - `total_substance_ton = total_substance_kg / Decimal::ONE_THOUSAND`
     - `shipment_count` and contributing shipment breakdown rows (for drill-down and internal detail report mode).

### 4.2 Formatted A4 PDF Report & Printing (`@media print` / `PrintToPdf`)

- **Two Report Modes (Controlled by explicit UI toggle)**:
  1. **External Importer Summary Mode (`대외 제출용 요약본` — Default)**:
     - **Confidentiality Guarantee**: Strictly omits per-product `함유율(%)` and recipe breakdown from both the DOM print tree and PDF output.
     - **Report Header**: Title (`연간 화학물질 수입량 집계 보고서`), Target Year/Period, Domestic Importer (`국내 수입사`), Overseas Branch (`해외지사`), Issue Date (`발행일자`), and summary statistics (total unique `CAS번호` count, total imported product weight `kg`/`ton`, total aggregated substance weight `kg`/`ton`).
     - **Summary Table Columns**: `NO` | `CAS번호` | `화학물질명` | `연간 총 수입량 (kg)` | `연간 총 수입량 (ton)`.
  2. **Internal Verification Detail Mode (`내부 검증용 상세본` — Optional Toggle)**:
     - Appends an indented breakdown sub-table underneath each `CAS번호` row (or in a dedicated detail section) showing: `수입일자` | `국내 수입사` | `해외지사` | `수입제품코드` | `제품 수입량 (kg)` | `함유율 (%)` | `화학물질 환산 중량 (kg / ton)`.
- **Print & PDF Styling Rules**:
  - `@page { size: A4 portrait; margin: 14mm 12mm; }`
  - Font family: `'Malgun Gothic', '맑은 고딕', sans-serif`, `print-color-adjust: exact`.
  - Multi-page table pagination: `thead { display: table-header-group; }`, `tfoot { display: table-footer-group; }`, `tr { break-inside: avoid; }`.

### 4.3 Excel (`.xlsx`) Export (`export_annual_report_xlsx` Tauri command)

Powered by `rust_xlsxwriter` + `unicode-width`:
- **When External Importer Summary Mode (`대외 제출용 요약본`) is active**:
  - Generates a **single-sheet** `.xlsx` workbook containing only `연간물질집계_요약` (`NO`, `CAS번호`, `화학물질명`, `연간 총 수입량 (kg)`, `연간 총 수입량 (ton)`), with zero confidential recipe (`함유율 %`) data anywhere in the file.
- **When Internal Verification Detail Mode (`내부 검증용 상세본`) is active**:
  - Generates a **two-sheet** `.xlsx` workbook:
    1. Sheet 1 (`1_연간물질집계_요약`): Aggregated substance totals by `CAS번호`.
    2. Sheet 2 (`2_수입실적별_상세내역`): Detailed rows (`CAS번호`, `화학물질명`, `수입일자`, `국내 수입사`, `해외지사`, `수입제품코드`, `제품명`, `제품 수입량 (kg)`, `함유율 (%)`, `화학물질 환산 중량 (kg)`, `화학물질 환산 중량 (ton)`).
- **Formatting Safeguards**:
  - `CAS번호` and `수입제품코드` are written with explicit `@` text format (`Format::new().set_num_format("@")`) via `write_string_with_format()` so Excel never corrupts `CAS번호` values into dates.
  - Numeric weight columns use `# ,##0.00` / `#,##0.0000` number formats so the customer can sum/filter cells in Excel immediately.
  - Column widths are auto-fitted using `unicode_width::UnicodeWidthStr::width()` (counting Korean Hangul syllables as 2 width units + padding).

---

## 5. Performance Targets, Cross-Build Pipeline & Verification

### 5.1 Low-Spec Windows 11 Resource & Latency Targets

| Metric | Target Budget (Low-Spec Windows 11, 8 GB RAM, Integrated GPU) | Design Mechanism |
| :--- | :--- | :--- |
| **Installer Size** | **< 10 MB** (`*-setup.exe`) | OS-bundled WebView2 + bundled SQLite + pure-Rust crates; zero external master Excel bundled. |
| **Cold Startup** | **< 1.0 second** to interactive window | Synchronous SQLite init, no upfront large dataset load, `Low` WebView2 memory target. |
| **Idle / Working RAM** | **~120–200 MB** total working set | Single WebView2 window + `CoreWebView2MemoryUsageTargetLevel::Low` + virtualized tables. |
| **Annual Aggregation Query** | **< 20 ms** (for 10,000+ shipments) | Indexed SQLite join + compiled `rust_decimal` aggregation in Rust. |
| **`.xlsx` Export** | **< 200 ms** | `rust_xlsxwriter` in `constant_memory` mode. |

### 5.2 macOS-to-Windows 11 Cross-Build Pipeline

1. **Local macOS Cross-Compilation**:
   ```bash
   # One-time setup on macOS
   rustup target add x86_64-pc-windows-msvc
   cargo install --locked cargo-xwin tauri-cli
   brew install nsis llvm

   # Build Windows 11 .exe and NSIS installer from macOS
   cargo tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis
   ```
   - Produces `src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/*-setup.exe` and the standalone `.exe`.
2. **GitHub Actions `windows-latest` CI Verification**:
   - Runs `cargo test` (unit and integration tests for SQLite schema, `100%` BOM validation, Excel import/export, and `rust_decimal` annual aggregation) and builds the signed/verified Windows NSIS installer on native MSVC.

### 5.3 Acceptance Verification Checklist

- [ ] **BOM `100%` Invariant & Unique `CAS번호`**: Saving or importing a product recipe whose `함유율(%)` sum is `99.9999%` or `100.0001%`, or that repeats the same `CAS번호` twice in one product, is blocked with a clear Korean error message.
- [ ] **Referential Integrity Guards (`ON DELETE RESTRICT`)**: Deleting an `수입제품`, `국내 수입사`, `해외지사`, or `화학물질` that is referenced by existing shipments/recipes is blocked without data loss, while deleting an unreferenced `수입제품` cascades cleanly to its `product_components`.
- [ ] **Self-Service Excel Import**: Downloading `.xlsx` templates, importing BOMs (with default overwrite of existing `수입제품코드`) and shipments (with default skip of duplicate 5-tuples), and verifying atomic rollback when validation errors are present.
- [ ] **Exact Fixed-Point Annual Aggregation**: `연간 화학물질 총 수입량` matches exact decimal math ($\sum [\text{kg} \times (\text{pct} / 100)]$) in `kg` and `ton` (`= kg / 1,000`) with zero IEEE-754 float drift.
- [ ] **Confidentiality in External Reports**: Default A4 PDF report and `.xlsx` export in `대외 제출용 요약본` mode omit all per-product `함유율(%)` data; toggling `내부 검증용 상세본` includes the breakdown in both A4 PDF and the 2-sheet `.xlsx` export, with `CAS번호` formatted as `@` text.
- [ ] **Live WAL Backup & Restore**: `DB 백업 저장 (.db)` creates a valid standalone SQLite file via `VACUUM INTO`, and `DB 백업 불러오기 (복원)` validates integrity before replacing the active DB.
