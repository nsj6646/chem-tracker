# 03: Import Shipment (`수입실적`) Quick-Entry, Filtering & Virtualized History

**What to build:** Deliver Workspace 2 (`2. 수입실적 관리`) end-to-end so the user can rapidly record, edit, filter, and delete dated `Import Shipment` (`수입실적`) records. A keyboard-friendly top quick-entry bar (`Tab` $\to$ `Enter`) lets the user input `수입일자` (`YYYY-MM-DD`), `Domestic Importer` (`국내 수입사`, with inline creation), `Imported Product` (`수입제품코드`, which automatically populates and locks its linked `Overseas Branch` / `해외지사`), `Product Import Weight` (`제품 수입량 kg` up to 4 decimals with live converted `ton` preview), and optional `비고`. Below the form, a filter and summary strip filters by year, `국내 수입사`, `해외지사`, and `수입제품코드` search text, displays total matching shipment count and summed `kg`/`ton`, and renders the shipment history in a smooth virtualized table (`@tanstack/react-virtual`).

**Blocked by:** 01: Walking Skeleton — Desktop Shell, SQLite WAL Schema, Fixed-Point Core & Sample Data Seed

**Status:** resolved

- [x] Top quick-entry / edit form bar supports creating and updating `Import Shipment` (`수입실적`) records with `수입일자` (`YYYY-MM-DD`), `국내 수입사` (including inline creation of new importers), `수입제품코드`, `제품 수입량 (kg)` (`> 0`, up to 4 decimal places), and `비고 (선택)` using a keyboard-friendly `Tab` $\to$ `Enter` flow.
- [x] Selecting an `Imported Product` (`수입제품코드`) in the shipment form automatically populates and locks its associated `Overseas Branch` (`해외지사`), and typing `제품 수입량 (kg)` shows a live fixed-point `ton` (`= kg / 1,000`) preview.
- [x] Filter and summary strip filters shipments by `연도` (`shipment_year`), `국내 수입사`, `해외지사`, and `수입제품코드`/`제품명` query, and displays exact total matching shipment count, sum of `제품 수입량 (kg)`, and sum of `환산 중량 (ton)`.
- [x] Shipment history table uses row virtualization (~30–50 DOM nodes rendered at once via `computeVirtualWindow`; ready to swap to `@tanstack/react-virtual` after online `npm install`) displaying `수입일자`, `국내 수입사`, `해외지사`, `수입제품코드` (`제품명`), `제품 수입량 (kg)`, `제품 환산 중량 (ton)`, `비고`, and inline `수정 / 삭제` actions.
- [x] Automated tests verify shipment CRUD, `shipment_year` generated column indexing/filtering, positive weight validation, and exact `kg`/`ton` summary totals.
