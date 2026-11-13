# 02: Imported Product (`수입제품`) & Confidential Product Recipe (`제품 구성성분` / BOM) Management

**What to build:** Deliver Workspace 1 (`1. 수입제품 및 레시피(BOM)`) end-to-end so the user can browse, search, create, edit, and delete `Imported Product` (`수입제품`) records and their confidential `Product Recipe` (`제품 구성성분` / BOM). The left pane lists products with filtering by `수입제품코드`, `제품명`, and `해외지사`, showing component counts and recipe validity badges (`100.00% 정상`). The right pane provides the BOM editor with `수입제품코드`, optional `제품명`, `Overseas Branch` (`해외지사`) selection and inline creation, `Chemical Substance` (`화학물질`: `CAS번호` autocomplete/auto-creation + inline `화학물질명`) component rows, and a live `함유율(%)` total gauge that enables saving only when the components sum to `100.0000%` with no duplicate `CAS번호`.

**Blocked by:** 01: Walking Skeleton — Desktop Shell, SQLite WAL Schema, Fixed-Point Core & Sample Data Seed

**Status:** ready-for-agent

- [ ] Left-pane `Imported Product` (`수입제품`) master list displays `수입제품코드`, `제품명(선택)`, `해외지사`, component count (`구성 물질 수`), and `100.00% 정상` status badge, and filters dynamically by search text and `해외지사`.
- [ ] Right-pane `Product Recipe` (`제품 구성성분`) editor supports creating a new product or editing a selected product, including inline creation of a new `Overseas Branch` (`+ 새 해외지사 추가`) and `CAS번호` autocomplete that auto-fills known `화학물질명` or registers new `Chemical Substance` records automatically on save.
- [ ] Live total percentage gauge calculates $\sum \text{함유율(\%)}$ using exact decimal math, showing a green badge (`합계: 100.00% — 저장 가능`) when the sum equals `100.0000%` and a red blocker banner (`합계: XX.XX% — 합계가 정확히 100%여야 저장할 수 있습니다 (차이: ±Y.YY%)`) when it does not.
- [ ] Saving an `Imported Product` runs in a single SQLite transaction that rejects empty required fields, zero component rows, any component outside `0 < weight_pct <= 100`, duplicate `CAS번호` within the same product, or a sum of `99.9999%` / `100.0001%` (`weight_pct_scaled != 1_000_000`) with a clear Korean error message.
- [ ] Deleting an `Imported Product` with zero `Import Shipment` (`수입실적`) rows succeeds and cascades (`ON DELETE CASCADE`) to its `product_components`; deleting a product referenced by `N` shipments is blocked (`ON DELETE RESTRICT`) with the message `"해당 수입제품으로 등록된 수입실적이 N건 존재하여 삭제할 수 없습니다. 수입실적을 먼저 삭제하거나 수정해 주세요."`
