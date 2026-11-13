//! Workspace 2: Import Shipment (`수입실적`) quick-entry, filtering & virtualized history.
//! Implemented in Ticket 03.

use crate::decimal::{format_kg, format_ton, from_scaled_4dp, parse_decimal_4dp};
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection, OptionalExtension};
use rust_decimal::Decimal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverseasBranchOption {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomesticImporterOption {
    pub id: i64,
    pub name: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductOption {
    pub product_id: i64,
    pub product_code: String,
    pub product_name: Option<String>,
    pub overseas_branch_id: i64,
    pub overseas_branch_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ShipmentFilter {
    pub year: Option<i32>,
    pub domestic_importer_id: Option<i64>,
    pub overseas_branch_id: Option<i64>,
    pub search_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShipmentSummary {
    pub total_count: usize,
    pub total_weight_kg_text: String,
    pub total_weight_ton_text: String,
    pub total_weight_kg_canonical: String,
    pub total_weight_kg_scaled: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShipmentQueryResult {
    pub rows: Vec<ShipmentRecord>,
    pub summary: ShipmentSummary,
    pub available_years: Vec<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveShipmentInput {
    pub id: Option<i64>,
    pub shipment_date: String,
    pub domestic_importer_name: String,
    pub product_id: i64,
    pub import_weight_kg: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShipmentRecord {
    pub id: i64,
    pub shipment_date: String,
    pub shipment_year: i32,
    pub domestic_importer_id: i64,
    pub domestic_importer_name: String,
    pub overseas_branch_id: i64,
    pub overseas_branch_name: String,
    pub product_id: i64,
    pub product_code: String,
    pub product_name: Option<String>,
    pub import_weight_kg_text: String,
    pub import_weight_kg_scaled: i64,
    pub import_weight_kg_display: String,
    pub import_weight_ton_display: String,
    pub note: Option<String>,
}

/// Lists all Overseas Branches (`해외지사`) ordered alphabetically by name.
pub fn list_overseas_branches(conn: &Connection) -> AppResult<Vec<OverseasBranchOption>> {
    let mut stmt = conn.prepare(
        "SELECT id, name
         FROM overseas_branches
         ORDER BY name COLLATE NOCASE ASC, id ASC;",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(OverseasBranchOption {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Lists all Domestic Importers (`국내 수입사`) ordered alphabetically by name.
pub fn list_domestic_importers(conn: &Connection) -> AppResult<Vec<DomesticImporterOption>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, note
         FROM domestic_importers
         ORDER BY name COLLATE NOCASE ASC, id ASC;",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(DomesticImporterOption {
                id: row.get(0)?,
                name: row.get(1)?,
                note: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Lists all Imported Products (`수입제품`) together with their locked Overseas Branch (`해외지사`).
pub fn list_product_options(conn: &Connection) -> AppResult<Vec<ProductOption>> {
    let mut stmt = conn.prepare(
        "SELECT ip.id, ip.product_code, ip.product_name, ob.id, ob.name
         FROM imported_products ip
         JOIN overseas_branches ob ON ob.id = ip.overseas_branch_id
         ORDER BY ip.product_code COLLATE NOCASE ASC, ip.id ASC;",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ProductOption {
                product_id: row.get(0)?,
                product_code: row.get(1)?,
                product_name: row.get(2)?,
                overseas_branch_id: row.get(3)?,
                overseas_branch_name: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Validates an ISO-8601 `YYYY-MM-DD` calendar date and returns its normalized string.
pub fn validate_shipment_date(raw: &str) -> AppResult<String> {
    let trimmed = raw.trim();
    let bytes = trimmed.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !bytes[0..4].iter().all(|b| b.is_ascii_digit())
        || !bytes[5..7].iter().all(|b| b.is_ascii_digit())
        || !bytes[8..10].iter().all(|b| b.is_ascii_digit())
    {
        return Err(AppError::Validation(format!(
            "수입일자는 YYYY-MM-DD 형식이어야 합니다: '{raw}'"
        )));
    }

    let year: i32 = trimmed[0..4].parse().unwrap();
    let month: u32 = trimmed[5..7].parse().unwrap();
    let day: u32 = trimmed[8..10].parse().unwrap();

    if !(1900..=2100).contains(&year) || !(1..=12).contains(&month) {
        return Err(AppError::Validation(format!(
            "올바른 수입일자(연/월)가 아닙니다: '{raw}'"
        )));
    }

    let max_day = days_in_month(year, month);
    if day < 1 || day > max_day {
        return Err(AppError::Validation(format!(
            "존재하지 않는 날짜입니다: '{raw}'"
        )));
    }

    Ok(trimmed.to_string())
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            let is_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
            if is_leap {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// Creates or updates an Import Shipment (`수입실적`), performing inline creation of
/// new Domestic Importers and locking `overseas_branch_id` to the selected product's branch.
pub fn save_shipment(conn: &Connection, input: SaveShipmentInput) -> AppResult<ShipmentRecord> {
    let shipment_date = validate_shipment_date(&input.shipment_date)?;

    let importer_name = input.domestic_importer_name.trim();
    if importer_name.is_empty() {
        return Err(AppError::Validation(
            "국내 수입사를 입력해 주세요.".to_string(),
        ));
    }

    let parsed_weight = parse_decimal_4dp(&input.import_weight_kg)?;
    if parsed_weight.scaled_i64 <= 0 {
        return Err(AppError::Validation(
            "제품 수입량(kg)은 0보다 커야 합니다.".to_string(),
        ));
    }

    let note = input
        .note
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let tx = conn.unchecked_transaction()?;

    // Lock overseas_branch_id to the imported_product's authoritative branch
    let overseas_branch_id: i64 = tx
        .query_row(
            "SELECT overseas_branch_id FROM imported_products WHERE id = ?1;",
            [input.product_id],
            |r| r.get(0),
        )
        .optional()?
        .ok_or_else(|| {
            AppError::Validation("선택한 수입제품이 존재하지 않습니다.".to_string())
        })?;

    // Inline upsert for domestic importer (name is UNIQUE COLLATE NOCASE)
    let domestic_importer_id: i64 = match tx
        .query_row(
            "SELECT id FROM domestic_importers WHERE name = ?1;",
            [importer_name],
            |r| r.get(0),
        )
        .optional()?
    {
        Some(existing_id) => existing_id,
        None => {
            tx.execute(
                "INSERT INTO domestic_importers (name) VALUES (?1);",
                [importer_name],
            )?;
            tx.last_insert_rowid()
        }
    };

    let shipment_id = match input.id {
        Some(existing_id) => {
            let affected = tx.execute(
                "UPDATE import_shipments
                 SET shipment_date = ?1,
                     domestic_importer_id = ?2,
                     overseas_branch_id = ?3,
                     product_id = ?4,
                     import_weight_kg_text = ?5,
                     import_weight_kg_scaled = ?6,
                     note = ?7,
                     updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                 WHERE id = ?8;",
                params![
                    shipment_date,
                    domestic_importer_id,
                    overseas_branch_id,
                    input.product_id,
                    parsed_weight.canonical_text,
                    parsed_weight.scaled_i64,
                    note,
                    existing_id
                ],
            )?;
            if affected == 0 {
                return Err(AppError::Validation(format!(
                    "수정할 수입실적(ID: {existing_id})을 찾을 수 없습니다."
                )));
            }
            existing_id
        }
        None => {
            tx.execute(
                "INSERT INTO import_shipments (
                    shipment_date,
                    domestic_importer_id,
                    overseas_branch_id,
                    product_id,
                    import_weight_kg_text,
                    import_weight_kg_scaled,
                    note
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7);",
                params![
                    shipment_date,
                    domestic_importer_id,
                    overseas_branch_id,
                    input.product_id,
                    parsed_weight.canonical_text,
                    parsed_weight.scaled_i64,
                    note
                ],
            )?;
            tx.last_insert_rowid()
        }
    };

    tx.commit()?;
    fetch_shipment_by_id(conn, shipment_id)
}

/// Deletes an Import Shipment (`수입실적`) by ID.
pub fn delete_shipment(conn: &Connection, shipment_id: i64) -> AppResult<()> {
    let affected = conn.execute("DELETE FROM import_shipments WHERE id = ?1;", [shipment_id])?;
    if affected == 0 {
        return Err(AppError::Validation(format!(
            "삭제할 수입실적(ID: {shipment_id})을 찾을 수 없습니다."
        )));
    }
    Ok(())
}

/// Queries Import Shipments (`수입실적`) filtered by year (`shipment_year`),
/// domestic importer, overseas branch, and product code/name search text,
/// returning rows and exact fixed-point kg/ton summary totals.
pub fn query_shipments(
    conn: &Connection,
    filter: ShipmentFilter,
) -> AppResult<ShipmentQueryResult> {
    let search_pattern = filter
        .search_text
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(escape_like_contains);

    let mut stmt = conn.prepare(
        "SELECT
            s.id,
            s.shipment_date,
            s.shipment_year,
            s.domestic_importer_id,
            di.name,
            s.overseas_branch_id,
            ob.name,
            s.product_id,
            ip.product_code,
            ip.product_name,
            s.import_weight_kg_text,
            s.import_weight_kg_scaled,
            s.note
         FROM import_shipments s
         JOIN domestic_importers di ON di.id = s.domestic_importer_id
         JOIN overseas_branches ob ON ob.id = s.overseas_branch_id
         JOIN imported_products ip ON ip.id = s.product_id
         WHERE (?1 IS NULL OR s.shipment_year = ?1)
           AND (?2 IS NULL OR s.domestic_importer_id = ?2)
           AND (?3 IS NULL OR s.overseas_branch_id = ?3)
           AND (
               ?4 IS NULL
               OR ip.product_code LIKE ?4 ESCAPE '\\'
               OR COALESCE(ip.product_name, '') LIKE ?4 ESCAPE '\\'
           )
         ORDER BY s.shipment_date DESC, s.id DESC;",
    )?;

    let rows = stmt
        .query_map(
            params![
                filter.year,
                filter.domestic_importer_id,
                filter.overseas_branch_id,
                search_pattern
            ],
            map_shipment_row,
        )?
        .collect::<Result<Vec<_>, _>>()?;

    let mut total_kg_dec = Decimal::ZERO;
    let mut total_kg_scaled: i64 = 0;
    for r in &rows {
        let row_dec = r
            .import_weight_kg_text
            .parse::<Decimal>()
            .unwrap_or_else(|_| from_scaled_4dp(r.import_weight_kg_scaled));
        total_kg_dec += row_dec;
        total_kg_scaled = total_kg_scaled.saturating_add(r.import_weight_kg_scaled);
    }

    let total_ton_dec = total_kg_dec / Decimal::ONE_THOUSAND;
    let summary = ShipmentSummary {
        total_count: rows.len(),
        total_weight_kg_text: format_kg(total_kg_dec),
        total_weight_ton_text: format_ton(total_ton_dec),
        total_weight_kg_canonical: total_kg_dec.normalize().to_string(),
        total_weight_kg_scaled: total_kg_scaled,
    };

    let mut years_stmt = conn.prepare(
        "SELECT DISTINCT shipment_year
         FROM import_shipments
         ORDER BY shipment_year DESC;",
    )?;
    let available_years = years_stmt
        .query_map([], |row| row.get(0))?
        .collect::<Result<Vec<i32>, _>>()?;

    Ok(ShipmentQueryResult {
        rows,
        summary,
        available_years,
    })
}

fn escape_like_contains(term: &str) -> String {
    let mut escaped = String::with_capacity(term.len() + 4);
    escaped.push('%');
    for ch in term.chars() {
        match ch {
            '\\' | '%' | '_' => {
                escaped.push('\\');
                escaped.push(ch);
            }
            _ => escaped.push(ch),
        }
    }
    escaped.push('%');
    escaped
}

fn fetch_shipment_by_id(conn: &Connection, shipment_id: i64) -> AppResult<ShipmentRecord> {
    conn.query_row(
        "SELECT
            s.id,
            s.shipment_date,
            s.shipment_year,
            s.domestic_importer_id,
            di.name,
            s.overseas_branch_id,
            ob.name,
            s.product_id,
            ip.product_code,
            ip.product_name,
            s.import_weight_kg_text,
            s.import_weight_kg_scaled,
            s.note
         FROM import_shipments s
         JOIN domestic_importers di ON di.id = s.domestic_importer_id
         JOIN overseas_branches ob ON ob.id = s.overseas_branch_id
         JOIN imported_products ip ON ip.id = s.product_id
         WHERE s.id = ?1;",
        [shipment_id],
        map_shipment_row,
    )
    .map_err(AppError::from)
}

fn map_shipment_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ShipmentRecord> {
    let weight_text: String = row.get(10)?;
    let weight_scaled: i64 = row.get(11)?;
    let weight_dec = weight_text
        .parse::<Decimal>()
        .unwrap_or_else(|_| crate::decimal::from_scaled_4dp(weight_scaled));
    let ton_dec = weight_dec / Decimal::ONE_THOUSAND;

    Ok(ShipmentRecord {
        id: row.get(0)?,
        shipment_date: row.get(1)?,
        shipment_year: row.get(2)?,
        domestic_importer_id: row.get(3)?,
        domestic_importer_name: row.get(4)?,
        overseas_branch_id: row.get(5)?,
        overseas_branch_name: row.get(6)?,
        product_id: row.get(7)?,
        product_code: row.get(8)?,
        product_name: row.get(9)?,
        import_weight_kg_text: weight_text,
        import_weight_kg_scaled: weight_scaled,
        import_weight_kg_display: format_kg(weight_dec),
        import_weight_ton_display: format_ton(ton_dec),
        note: row.get(12)?,
    })
}
