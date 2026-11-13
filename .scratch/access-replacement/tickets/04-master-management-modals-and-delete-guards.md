# 04: Compact Master Management Modals (`[관리/수정]`) & Reference-Count Deletion Guards

**What to build:** Deliver the compact Master Management modal (`[관리/수정]`) accessible next to the `Domestic Importer` (`국내 수입사`), `Overseas Branch` (`해외지사`), and `Chemical Substance` (`화학물질`) selectors in Workspaces 1 and 2. Users can inspect how many products, recipes, or shipments reference each master item, rename items or fix typos in place (`name`, `cas_number`, `substance_name`) so all referencing views update automatically via foreign key IDs, and delete unused items while `ON DELETE RESTRICT` blocks deletion of referenced records with an explicit Korean reference-count error.

**Blocked by:** 02: Imported Product (`수입제품`) & Confidential Product Recipe (`제품 구성성분` / BOM) Management, 03: Import Shipment (`수입실적`) Quick-Entry, Filtering & Virtualized History

**Status:** ready-for-agent

- [ ] Clicking `[관리/수정]` next to `해외지사` or `[화학물질 관리/수정]` in Workspace 1, or `[수입사 관리/수정]` in Workspace 2, opens the compact Master Management modal listing all records for that master entity along with their current reference counts (e.g., referencing products, BOM components, or shipments).
- [ ] Inline editing of a `Domestic Importer` `name`, `Overseas Branch` `name`, or `Chemical Substance` `cas_number` / `substance_name` validates uniqueness (case-insensitive for importer/branch names; normalized/trimmed for `CAS번호`) and immediately reflects the updated text across all referencing `Imported Product`, `Product Recipe`, and `Import Shipment` views.
- [ ] Unreferenced master entities (reference count = `0`) can be deleted directly from the modal.
- [ ] Attempting to delete a referenced `Domestic Importer`, `Overseas Branch`, or `Chemical Substance` is blocked by `ON DELETE RESTRICT` and surfaces a clear Korean error message stating the exact number of referencing records.
