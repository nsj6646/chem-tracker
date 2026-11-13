# Define Customer's Existing Access Workflow (Tables, Queries, Forms, Reports)

Type: `grilling`
Label: `wayfinder:grilling`
Status: resolved

## Question

What is the exact domain model and daily workflow in the customer's current Microsoft Access database?

Through a grilling & domain-modeling session with the user, establish and record in `GLOSSARY.md`:

1. **Tables & Core Entities**: What tables exist in the Access database, roughly how many rows do they hold, and how do they relate?
2. **Forms (Data Entry & Lookup)**: What screens/forms does the customer use to input, edit, or search records? What validations or auto-calculations happen on those forms?
3. **Queries (Filtering & Aggregation)**: What recurring searches, date ranges, or aggregations does the customer run?
4. **Reports (Print/PDF & Excel)**: What exact reports are generated, what is their layout (e.g., header, line items table, subtotals/totals), and how often are they printed or exported to Excel?

## Answer

The domain model and workflow are defined in [`GLOSSARY.md`](../../../GLOSSARY.md) for the **Korea branch office (`코리아 지사`)**, which sits between **Overseas Branches (`해외지사`)** that supply chemical products and **Domestic Importers (`국내 수입사`)** that import them into South Korea:

1. **Tables & Core Entities (Data Scale & Confidentiality)**:
   - **Chemical Substance Master (`화학물질 마스터`)**: Pre-loaded reference table of ~47,531 regulated/existing chemical substances from [`docs/화학물질_20261109.xlsx`](../../../docs/화학물질_20261109.xlsx) (`NO`, `CAS번호`, `영문명`, `국문명`, `기존`/`KE번호`, `급성·만성·생태`, `사고대비`, `제한/금지/허가`, `중점`, `잔류`, `유해특성분류 및 혼합물 함량기준(%)`, `등록대상기존화학물질`, `기존물질여부`). Can be updated by re-importing newer `.xlsx` master files.
   - **Domestic Importer (`국내 수입사`)** & **Overseas Branch (`해외지사`)**: Master tables for the importing domestic companies and supplying overseas affiliates.
   - **Imported Product (`수입제품`)** & **Product Recipe (`제품 구성성분` / BOM)**: Keyed by `수입제품코드` (linked to an `해외지사`), each product has a confidential 1:N composition list of `CAS번호` + `함유율(%)` (single numeric weight percentage used for calculation). Because recipes are strictly confidential to the customer, development uses synthetic sample data, and the Korea branch user imports existing recipes/history directly on their Windows 11 PC via self-service Excel (`.xlsx`/`.csv`) or Access (`.accdb`) import or manual form entry.
   - **Import Shipment (`수입실적`)**: Dated shipment records (`수입일자`, `국내 수입사`, `해외지사`, `수입제품코드`, `제품 수입량(kg)`), at the scale of thousands to tens of thousands of rows total.

2. **Forms (Data Entry & Self-Service Import)**:
   - **Product & Recipe (BOM) Form**: Register/edit `수입제품코드` and `해외지사`, with a sub-grid for `CAS번호` (auto-completing/displaying `영문명`, `국문명`, `KE번호`, and regulatory flags from the master table) and `함유율(%)` (showing total % reference).
   - **Import Shipment Form**: Enter or bulk-import (`Excel`/paste) dated import shipments (`수입일자`, `국내 수입사`, `해외지사`, `수입제품코드`, `제품 수입량(kg)`).
   - **Master & Self-Service Data Management Screen**: Manage `국내 수입사` and `해외지사`, refresh the `화학물질 마스터` from `.xlsx`, and run self-service bulk import of legacy Access/Excel recipes and shipment history.

3. **Queries (Filtering & Annual Substance Aggregation)**:
   - Filter by **Year / Date Range (`연간` / `기간`)**, **Domestic Importer (`국내 수입사`)**, and optionally **Overseas Branch (`해외지사`)** or **Imported Product (`수입제품코드`)**.
   - Compute per-shipment **Substance Import Weight** $= \text{Product Import Weight (kg)} \times (\text{Weight Percentage (\%)} / 100)$ using exact/fixed-point decimal arithmetic, then aggregate (`SUM`) by **`CAS번호`** to produce the **Annual Substance Total (`연간 화학물질 총 수입량`)** in both **`kg`** and **`ton` (`= kg / 1,000`)**, joined with `화학물질 마스터` columns (`영문명`, `국문명`, `기존(KE번호)`, `기존물질여부`, and regulatory classifications).
   - Include an internal drill-down view showing which `수입일자` and `수입제품코드` contributed to each `CAS번호` total.

4. **Reports (Formatted PDF & Excel Export)**:
   - **Default External/Importer Summary PDF Report**: Displays header metadata (`국내 수입사`, target year/period, summary of imported products) and the aggregated table of **`CAS번호` | `물질명(영문/국문)` | `KE번호(기존)` | `연간 총 수입량 (kg / ton)` | `기존물질여부 및 규제정보`**, omitting confidential per-product `함유율(%)` recipes.
   - **Optional Internal Detail Mode**: Checkbox option to include product-by-product breakdown for internal Korea branch verification, plus `.xlsx` export of query results.
