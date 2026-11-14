# 01: Walking Skeleton — Desktop Shell, SQLite WAL Schema, Fixed-Point Core & Sample Data Seed

**What to build:** Launch the single-window Tauri v2 + React 19 desktop application (`화학물질 수입량 집계 시스템 — 코리아 지사`) with the left sidebar (`240px`), 4 primary workspace tabs (`1. 수입제품 및 레시피(BOM)`, `2. 수입실적 관리`, `3. 엑셀 일괄 가져오기`, `4. 연간 물질 집계 & A4 보고서`), and bottom-anchored database panel. On startup, open and initialize the local SQLite database in `WAL` mode (`synchronous = NORMAL`, `foreign_keys = ON`, `temp_store = MEMORY`, `cache_size = -8000`) with the 6 core tables (`domestic_importers`, `overseas_branches`, `chemical_substances`, `imported_products`, `product_components`, `import_shipments`) and indexes. Provide fixed-point decimal conversion and formatting utilities (`rust_decimal`, 4-decimal scaled integers `× 10,000`), display active database status and entity counts in the sidebar, and allow the user to click `샘플 데이터 적재` to populate a realistic synthetic dataset and immediately verify database connectivity end-to-end.

**Blocked by:** None (can start immediately)

**Status:** resolved

## Schema & Fixed-Point Decision (from Prototype & Spec)

```sql
-- Product Recipe / BOM Components (제품 구성성분)
CREATE TABLE IF NOT EXISTS product_components (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id              INTEGER NOT NULL,
    substance_id            INTEGER NOT NULL,
    weight_pct_text         TEXT NOT NULL,    -- Canonical decimal string e.g. '35.25'
    weight_pct_scaled       INTEGER NOT NULL, -- Scaled by 10,000 (1% = 10,000; 100% = 1,000,000)
    CHECK (weight_pct_scaled > 0 AND weight_pct_scaled <= 1000000),
    UNIQUE (product_id, substance_id),
    FOREIGN KEY (product_id) REFERENCES imported_products(id) ON DELETE CASCADE,
    FOREIGN KEY (substance_id) REFERENCES chemical_substances(id) ON DELETE RESTRICT
);
```

- [ ] Application launches into the `Variant A` desktop layout with the fixed left sidebar (`240px`, hidden in `@media print`), active DB status indicator, 4 workspace navigation tabs, and bottom database management panel. *(Implemented in `src/App.tsx`, `src/index.css`, and `src-tauri/tauri.conf.json`; requires online `npm install` + `tauri` crate linking to launch the desktop binary.)*
- [x] Local SQLite database initializes on connection open with `PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA foreign_keys = ON; PRAGMA temp_store = MEMORY; PRAGMA cache_size = -8000;` and creates all 6 tables (`domestic_importers`, `overseas_branches`, `chemical_substances`, `imported_products`, `product_components`, `import_shipments`) plus indexes.
- [x] Fixed-point decimal domain helpers losslessly parse decimal inputs up to 4 decimal places into canonical `Decimal` strings and `× 10,000` scaled integers (`100.0000% = 1_000_000`), and format `kg` (2–4 decimals), `ton = kg / 1,000` (4–5 decimals), and `함유율(%)` (2–4 decimals) without IEEE-754 floating-point drift.
- [x] Clicking `샘플 데이터 적재` populates synthetic `Domestic Importer` (`국내 수입사`), `Overseas Branch` (`해외지사`), `Chemical Substance` (`화학물질`), `Imported Product` (`수입제품` with `100%` `Product Recipe`), and `Import Shipment` (`수입실적`) records inside a single transaction and updates the UI status indicator.
- [x] Automated backend tests verify schema creation, foreign key and `CHECK` constraints, `UNIQUE (product_id, substance_id)` enforcement, and fixed-point decimal conversion/formatting rules.
