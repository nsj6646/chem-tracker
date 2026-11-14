# 05: Self-Service Excel (`.xlsx` / `.csv`) Template Download, Preview Validation & Atomic Import

**What to build:** Deliver Workspace 3 (`3. 엑셀 일괄 가져오기`) end-to-end so the Korea branch user can self-migrate confidential `Product Recipe` (`제품 구성성분` / BOM) and `Import Shipment` (`수입실적`) data from Excel/CSV files without developer intervention. Users can download pre-formatted `.xlsx` templates (`제품구성성분_BOM_양식.xlsx` with `@` text format on `CAS번호`, and `수입실적_양식.xlsx`), select a `.xlsx`, `.xls`, or `.csv` file to parse and preview in a validation table before committing, review highlighted row/product errors, configure conflict toggles (overwrite vs. skip existing products for BOMs; skip vs. force-insert duplicate 5-tuples for shipments), and commit valid data atomically in a single SQLite transaction.

**Blocked by:** 02: Imported Product (`수입제품`) & Confidential Product Recipe (`제품 구성성분` / BOM) Management, 03: Import Shipment (`수입실적`) Quick-Entry, Filtering & Virtualized History

**Status:** resolved

- [x] Users can download `제품구성성분_BOM_양식.xlsx` (`해외지사`, `수입제품코드`, `제품명(선택)`, `CAS번호`, `화학물질명(선택)`, `함유율(%)` with `@` text formatting on the `CAS번호` column so values like `50-00-0` are never converted to dates) and `수입실적_양식.xlsx` (`수입일자(YYYY-MM-DD)`, `국내 수입사`, `해외지사`, `수입제품코드`, `제품 수입량(kg)`, `비고(선택)`).
- [x] Uploading a `.xlsx`, `.xls`, or `.csv` file parses and normalizes text/numeric/date cells via `calamine` / `csv` and renders a pre-commit preview showing total rows/products, valid count, conflict/duplicate count, and validation error count.
- [x] For BOM imports, rows are grouped by `수입제품코드`; if any product in the file has a `함유율(%)` sum $\neq 100.0000\%$, duplicate `CAS번호` within the same product, or missing required fields, the offending product/rows are highlighted in red with exact Korean error descriptions and the commit button is disabled until a valid file is loaded.
- [x] For BOM imports, existing `수입제품코드` matches are flagged in the preview and controlled by the toggle `기존 수입제품 레시피 덮어쓰기 (기본값: 켜짐)` vs. `기존 제품 건너뛰기`.
- [x] For Shipment imports, invalid dates, non-positive weights, or unknown `수입제품코드` values disable commit with clear errors, while suspected duplicates matching `(수입일자, 국내 수입사, 해외지사, 수입제품코드, 제품 수입량)` against the DB or within the upload batch are flagged and controlled by the toggle `중복 수입실적 건너뛰기 (기본값: 켜짐)` vs. `중복 포함 강제 추가`.
- [x] Clicking commit executes all inserts/overwrites atomically inside a single `BEGIN IMMEDIATE ... COMMIT` transaction, rolling back completely if any error occurs.
