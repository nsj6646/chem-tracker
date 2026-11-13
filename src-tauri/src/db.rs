use crate::decimal::{parse_decimal_4dp, HUNDRED_PERCENT_SCALED};
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection, TransactionBehavior};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const REQUIRED_TABLES: &[&str] = &[
    "domestic_importers",
    "overseas_branches",
    "chemical_substances",
    "imported_products",
    "product_components",
    "import_shipments",
];

pub const REQUIRED_INDEXES: &[&str] = &[
    "idx_imported_products_branch",
    "idx_product_components_product",
    "idx_product_components_substance",
    "idx_shipments_year_importer",
    "idx_shipments_date",
    "idx_shipments_product",
];

pub const SCHEMA_DDL: &str = r#"
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
    cas_number      TEXT NOT NULL UNIQUE,
    substance_name  TEXT,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- 4. Imported Products (수입제품)
CREATE TABLE IF NOT EXISTS imported_products (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    product_code        TEXT NOT NULL UNIQUE COLLATE NOCASE,
    product_name        TEXT,
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
    weight_pct_text         TEXT NOT NULL,
    weight_pct_scaled       INTEGER NOT NULL,
    CHECK (weight_pct_scaled > 0 AND weight_pct_scaled <= 1000000),
    UNIQUE (product_id, substance_id),
    FOREIGN KEY (product_id) REFERENCES imported_products(id) ON DELETE CASCADE,
    FOREIGN KEY (substance_id) REFERENCES chemical_substances(id) ON DELETE RESTRICT
);

-- 6. Import Shipments (수입실적)
CREATE TABLE IF NOT EXISTS import_shipments (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    shipment_date           TEXT NOT NULL,
    shipment_year           INTEGER GENERATED ALWAYS AS (CAST(substr(shipment_date, 1, 4) AS INTEGER)) STORED,
    domestic_importer_id    INTEGER NOT NULL,
    overseas_branch_id      INTEGER NOT NULL,
    product_id              INTEGER NOT NULL,
    import_weight_kg_text   TEXT NOT NULL,
    import_weight_kg_scaled INTEGER NOT NULL,
    note                    TEXT,
    created_at              TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at              TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    CHECK (import_weight_kg_scaled > 0),
    FOREIGN KEY (domestic_importer_id) REFERENCES domestic_importers(id) ON DELETE RESTRICT,
    FOREIGN KEY (overseas_branch_id) REFERENCES overseas_branches(id) ON DELETE RESTRICT,
    FOREIGN KEY (product_id) REFERENCES imported_products(id) ON DELETE RESTRICT
);

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
"#;

pub fn apply_pragmas(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA temp_store = MEMORY;
         PRAGMA cache_size = -8000;",
    )?;
    Ok(())
}

pub fn init_schema(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(SCHEMA_DDL)?;
    Ok(())
}

/// Opens (or creates) a SQLite database at `path`, executes the 5 required pragmas,
/// and initializes all 6 tables and 6 indexes.
pub fn open_connection(path: impl AsRef<Path>) -> AppResult<Connection> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let conn = Connection::open(path)?;
    apply_pragmas(&conn)?;
    init_schema(&conn)?;
    Ok(conn)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbStatus {
    pub db_path: String,
    pub journal_mode: String,
    pub integrity_ok: bool,
    pub domestic_importers_count: i64,
    pub overseas_branches_count: i64,
    pub chemical_substances_count: i64,
    pub imported_products_count: i64,
    pub product_components_count: i64,
    pub import_shipments_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedResult {
    pub inserted_importers: usize,
    pub inserted_branches: usize,
    pub inserted_substances: usize,
    pub inserted_products: usize,
    pub inserted_shipments: usize,
    pub already_seeded: bool,
    pub status: DbStatus,
}

pub fn default_db_path() -> PathBuf {
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        if !local_app_data.trim().is_empty() {
            return PathBuf::from(local_app_data)
                .join("com.koreabranch.chemtracker")
                .join("data.db");
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if !home.trim().is_empty() {
            return PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("com.koreabranch.chemtracker")
                .join("data.db");
        }
    }
    PathBuf::from("data.db")
}

pub fn get_db_status(conn: &Connection, db_path: impl AsRef<Path>) -> AppResult<DbStatus> {
    let journal_mode: String = conn.query_row("PRAGMA journal_mode;", [], |r| r.get(0))?;
    let integrity_msg: String = conn.query_row("PRAGMA quick_check(1);", [], |r| r.get(0))?;
    let integrity_ok = integrity_msg.eq_ignore_ascii_case("ok");

    Ok(DbStatus {
        db_path: db_path.as_ref().display().to_string(),
        journal_mode,
        integrity_ok,
        domestic_importers_count: count_table(conn, "domestic_importers")?,
        overseas_branches_count: count_table(conn, "overseas_branches")?,
        chemical_substances_count: count_table(conn, "chemical_substances")?,
        imported_products_count: count_table(conn, "imported_products")?,
        product_components_count: count_table(conn, "product_components")?,
        import_shipments_count: count_table(conn, "import_shipments")?,
    })
}

fn count_table(conn: &Connection, table: &str) -> AppResult<i64> {
    if !REQUIRED_TABLES.contains(&table) {
        return Err(AppError::Validation(format!("알 수 없는 테이블: {table}")));
    }
    let sql = format!("SELECT COUNT(*) FROM {table};");
    Ok(conn.query_row(&sql, [], |r| r.get(0))?)
}

/// Populates a realistic synthetic dataset (Domestic Importers, Overseas Branches,
/// Chemical Substances, Imported Products with 100% BOMs, and Import Shipments)
/// inside a single `BEGIN IMMEDIATE` transaction.
/// If products or shipments already exist, returns `already_seeded = true` without duplicating data.
pub fn seed_sample_data(conn: &mut Connection, db_path: impl AsRef<Path>) -> AppResult<SeedResult> {
    let existing_products = count_table(conn, "imported_products")?;
    let existing_shipments = count_table(conn, "import_shipments")?;
    if existing_products > 0 || existing_shipments > 0 {
        let status = get_db_status(conn, db_path)?;
        return Ok(SeedResult {
            inserted_importers: 0,
            inserted_branches: 0,
            inserted_substances: 0,
            inserted_products: 0,
            inserted_shipments: 0,
            already_seeded: true,
            status,
        });
    }

    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let importers = [
        ("(주)대한케미칼", Some("서울 본사 / 연간 보고 대상")),
        ("한성정밀화학(주)", Some("울산 공장")),
        ("세진소재산업(주)", None),
    ];
    let mut importer_ids: HashMap<&str, i64> = HashMap::new();
    let mut inserted_importers = 0usize;
    for (name, note) in importers {
        tx.execute(
            "INSERT OR IGNORE INTO domestic_importers (name, note) VALUES (?1, ?2);",
            params![name, note],
        )?;
        inserted_importers += tx.changes() as usize;
        let id: i64 = tx.query_row(
            "SELECT id FROM domestic_importers WHERE name = ?1;",
            [name],
            |r| r.get(0),
        )?;
        importer_ids.insert(name, id);
    }

    let branches = [
        ("유럽지사 (독일)", Some("Frankfurt Hub")),
        ("북미지사 (미국)", Some("Houston Plant")),
        ("일본지사 (도쿄)", Some("Tokyo Office")),
    ];
    let mut branch_ids: HashMap<&str, i64> = HashMap::new();
    let mut inserted_branches = 0usize;
    for (name, note) in branches {
        tx.execute(
            "INSERT OR IGNORE INTO overseas_branches (name, note) VALUES (?1, ?2);",
            params![name, note],
        )?;
        inserted_branches += tx.changes() as usize;
        let id: i64 = tx.query_row(
            "SELECT id FROM overseas_branches WHERE name = ?1;",
            [name],
            |r| r.get(0),
        )?;
        branch_ids.insert(name, id);
    }

    let substances = [
        ("67-64-1", Some("아세톤 (Acetone)")),
        ("108-88-3", Some("톨루엔 (Toluene)")),
        ("141-78-6", Some("초산에틸 (Ethyl acetate)")),
        ("71-36-3", Some("1-부탄올 (1-Butanol)")),
        ("7732-18-5", Some("정제수 (Water)")),
        ("1330-20-7", Some("크실렌 (Xylene)")),
        ("9002-88-4", None),
    ];
    let mut substance_ids: HashMap<&str, i64> = HashMap::new();
    let mut inserted_substances = 0usize;
    for (cas, name) in substances {
        tx.execute(
            "INSERT OR IGNORE INTO chemical_substances (cas_number, substance_name) VALUES (?1, ?2);",
            params![cas, name],
        )?;
        inserted_substances += tx.changes() as usize;
        let id: i64 = tx.query_row(
            "SELECT id FROM chemical_substances WHERE cas_number = ?1;",
            [cas],
            |r| r.get(0),
        )?;
        substance_ids.insert(cas, id);
    }

    struct SeedProduct<'a> {
        code: &'a str,
        name: &'a str,
        branch: &'a str,
        components: &'a [(&'a str, &'a str)],
    }

    let products = [
        SeedProduct {
            code: "PRD-EU-101",
            name: "코팅솔벤트 A-100",
            branch: "유럽지사 (독일)",
            components: &[
                ("67-64-1", "45.25"),
                ("108-88-3", "34.75"),
                ("141-78-6", "20"),
            ],
        },
        SeedProduct {
            code: "PRD-EU-204",
            name: "고기능 세정제 B-200",
            branch: "유럽지사 (독일)",
            components: &[
                ("67-64-1", "60"),
                ("71-36-3", "25.5"),
                ("7732-18-5", "14.5"),
            ],
        },
        SeedProduct {
            code: "PRD-US-305",
            name: "표면처리제 C-300",
            branch: "북미지사 (미국)",
            components: &[
                ("1330-20-7", "72.125"),
                ("108-88-3", "17.875"),
                ("9002-88-4", "10"),
            ],
        },
    ];

    let mut product_ids: HashMap<&str, (i64, i64)> = HashMap::new();
    let mut inserted_products = 0usize;
    for prod in products {
        let branch_id = branch_ids[prod.branch];
        tx.execute(
            "INSERT INTO imported_products (product_code, product_name, overseas_branch_id) VALUES (?1, ?2, ?3);",
            params![prod.code, prod.name, branch_id],
        )?;
        let product_id = tx.last_insert_rowid();
        inserted_products += 1;

        let mut total_pct_scaled: i64 = 0;
        for (cas, pct_str) in prod.components {
            let parsed = parse_decimal_4dp(pct_str)?;
            total_pct_scaled += parsed.scaled_i64;
            let substance_id = substance_ids[cas];
            tx.execute(
                "INSERT INTO product_components (product_id, substance_id, weight_pct_text, weight_pct_scaled) VALUES (?1, ?2, ?3, ?4);",
                params![product_id, substance_id, parsed.canonical_text, parsed.scaled_i64],
            )?;
        }
        if total_pct_scaled != HUNDRED_PERCENT_SCALED {
            return Err(AppError::Validation(format!(
                "샘플 레시피 {}의 함유율 합계가 100%가 아닙니다.",
                prod.code
            )));
        }
        product_ids.insert(prod.code, (product_id, branch_id));
    }

    let shipments = [
        ("2026-01-15", "(주)대한케미칼", "PRD-EU-101", "2400", Some("1분기 정기 입고")),
        ("2026-03-10", "(주)대한케미칼", "PRD-EU-204", "1500.5", None),
        ("2026-05-22", "(주)대한케미칼", "PRD-US-305", "3200", Some("상반기 추가 물량")),
        ("2026-08-04", "(주)대한케미칼", "PRD-EU-101", "1100.25", None),
        ("2026-04-18", "한성정밀화학(주)", "PRD-EU-204", "4000", Some("울산 공장 직송")),
        ("2025-11-12", "(주)대한케미칼", "PRD-EU-101", "1800", Some("전년도 비교용 실적")),
    ];

    let mut inserted_shipments = 0usize;
    for (date, importer_name, product_code, weight_str, note) in shipments {
        let importer_id = importer_ids[importer_name];
        let (product_id, branch_id) = product_ids[product_code];
        let weight = parse_decimal_4dp(weight_str)?;
        tx.execute(
            "INSERT INTO import_shipments (shipment_date, domestic_importer_id, overseas_branch_id, product_id, import_weight_kg_text, import_weight_kg_scaled, note)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7);",
            params![
                date,
                importer_id,
                branch_id,
                product_id,
                weight.canonical_text,
                weight.scaled_i64,
                note
            ],
        )?;
        inserted_shipments += 1;
    }

    tx.commit()?;

    let status = get_db_status(conn, db_path)?;
    Ok(SeedResult {
        inserted_importers,
        inserted_branches,
        inserted_substances,
        inserted_products,
        inserted_shipments,
        already_seeded: false,
        status,
    })
}
