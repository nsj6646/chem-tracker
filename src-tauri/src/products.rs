//! Workspace 1: Imported Product (`수입제품`) & Product Recipe (`제품 구성성분`/BOM) management.
//! Implemented in Ticket 02.

use crate::decimal::{
    format_percentage, from_scaled_4dp, parse_decimal_4dp, HUNDRED_PERCENT_SCALED,
};
use crate::domain::{
    normalize_cas_number, upsert_chemical_substance, upsert_overseas_branch,
};
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductListItem {
    pub id: i64,
    pub product_code: String,
    pub product_name: Option<String>,
    pub overseas_branch_id: i64,
    pub overseas_branch_name: String,
    pub component_count: i64,
    pub weight_pct_sum_scaled: i64,
    pub is_valid_100_pct: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductComponentDetail {
    pub id: i64,
    pub substance_id: i64,
    pub cas_number: String,
    pub substance_name: Option<String>,
    pub weight_pct_text: String,
    pub weight_pct_scaled: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductDetail {
    pub id: i64,
    pub product_code: String,
    pub product_name: Option<String>,
    pub overseas_branch_id: i64,
    pub overseas_branch_name: String,
    pub weight_pct_sum_scaled: i64,
    pub is_valid_100_pct: bool,
    pub components: Vec<ProductComponentDetail>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverseasBranchOption {
    pub id: i64,
    pub name: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChemicalSubstanceOption {
    pub id: i64,
    pub cas_number: String,
    pub substance_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveBomComponentInput {
    pub cas_number: String,
    pub substance_name: Option<String>,
    pub weight_pct_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveProductInput {
    pub id: Option<i64>,
    pub product_code: String,
    pub product_name: Option<String>,
    pub overseas_branch_name: String,
    pub components: Vec<SaveBomComponentInput>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveProductResult {
    pub product: ProductDetail,
    pub created: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteProductResult {
    pub deleted_product_id: i64,
    pub deleted_product_code: String,
}

struct ValidatedComponent {
    cas_number: String,
    substance_name: Option<String>,
    weight_pct_text: String,
    weight_pct_scaled: i64,
}

fn trim_optional(input: Option<&str>) -> Option<String> {
    input
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

/// Lists all imported products with optional search text (`수입제품코드`, `제품명`, `해외지사`)
/// and `overseas_branch_id` filter.
pub fn list_products(
    conn: &Connection,
    search: Option<&str>,
    overseas_branch_id: Option<i64>,
) -> AppResult<Vec<ProductListItem>> {
    let query = search.map(|s| s.trim()).unwrap_or("");
    let mut stmt = conn.prepare(
        "SELECT
            ip.id,
            ip.product_code,
            ip.product_name,
            ip.overseas_branch_id,
            ob.name AS overseas_branch_name,
            COUNT(pc.id) AS component_count,
            COALESCE(SUM(pc.weight_pct_scaled), 0) AS weight_pct_sum_scaled
         FROM imported_products ip
         JOIN overseas_branches ob ON ob.id = ip.overseas_branch_id
         LEFT JOIN product_components pc ON pc.product_id = ip.id
         WHERE (?1 = ''
                OR instr(lower(ip.product_code), lower(?1)) > 0
                OR instr(lower(COALESCE(ip.product_name, '')), lower(?1)) > 0
                OR instr(lower(ob.name), lower(?1)) > 0)
           AND (?2 IS NULL OR ip.overseas_branch_id = ?2)
         GROUP BY ip.id, ip.product_code, ip.product_name, ip.overseas_branch_id, ob.name
         ORDER BY ip.product_code COLLATE NOCASE ASC, ip.id ASC;",
    )?;

    let rows = stmt
        .query_map(params![query, overseas_branch_id], |row| {
            let sum_scaled: i64 = row.get(6)?;
            Ok(ProductListItem {
                id: row.get(0)?,
                product_code: row.get(1)?,
                product_name: row.get(2)?,
                overseas_branch_id: row.get(3)?,
                overseas_branch_name: row.get(4)?,
                component_count: row.get(5)?,
                weight_pct_sum_scaled: sum_scaled,
                is_valid_100_pct: sum_scaled == HUNDRED_PERCENT_SCALED,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows)
}

/// Fetches a single product header and its ordered BOM components.
pub fn get_product_detail(conn: &Connection, product_id: i64) -> AppResult<ProductDetail> {
    let header = conn
        .query_row(
            "SELECT
                ip.id,
                ip.product_code,
                ip.product_name,
                ip.overseas_branch_id,
                ob.name
             FROM imported_products ip
             JOIN overseas_branches ob ON ob.id = ip.overseas_branch_id
             WHERE ip.id = ?1;",
            [product_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| {
            AppError::Validation(format!("수입제품을 찾을 수 없습니다 (ID: {product_id})."))
        })?;

    let mut stmt = conn.prepare(
        "SELECT
            pc.id,
            pc.substance_id,
            cs.cas_number,
            cs.substance_name,
            pc.weight_pct_text,
            pc.weight_pct_scaled
         FROM product_components pc
         JOIN chemical_substances cs ON cs.id = pc.substance_id
         WHERE pc.product_id = ?1
         ORDER BY pc.id ASC;",
    )?;

    let components = stmt
        .query_map([product_id], |row| {
            Ok(ProductComponentDetail {
                id: row.get(0)?,
                substance_id: row.get(1)?,
                cas_number: row.get(2)?,
                substance_name: row.get(3)?,
                weight_pct_text: row.get(4)?,
                weight_pct_scaled: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let weight_pct_sum_scaled: i64 = components.iter().map(|c| c.weight_pct_scaled).sum();

    Ok(ProductDetail {
        id: header.0,
        product_code: header.1,
        product_name: header.2,
        overseas_branch_id: header.3,
        overseas_branch_name: header.4,
        weight_pct_sum_scaled,
        is_valid_100_pct: weight_pct_sum_scaled == HUNDRED_PERCENT_SCALED,
        components,
    })
}

/// Lists all overseas branches ordered alphabetically (case-insensitive).
pub fn list_overseas_branches(conn: &Connection) -> AppResult<Vec<OverseasBranchOption>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, note
         FROM overseas_branches
         ORDER BY name COLLATE NOCASE ASC, id ASC;",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(OverseasBranchOption {
                id: row.get(0)?,
                name: row.get(1)?,
                note: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Searches chemical substances by `CAS번호` or `화학물질명` substring.
pub fn search_chemical_substances(
    conn: &Connection,
    query: Option<&str>,
) -> AppResult<Vec<ChemicalSubstanceOption>> {
    let q = query.map(|s| s.trim()).unwrap_or("");
    let mut stmt = conn.prepare(
        "SELECT id, cas_number, substance_name
         FROM chemical_substances
         WHERE (?1 = ''
                OR instr(lower(cas_number), lower(?1)) > 0
                OR instr(lower(COALESCE(substance_name, '')), lower(?1)) > 0)
         ORDER BY cas_number ASC, id ASC;",
    )?;
    let rows = stmt
        .query_map([q], |row| {
            Ok(ChemicalSubstanceOption {
                id: row.get(0)?,
                cas_number: row.get(1)?,
                substance_name: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn validate_save_input(
    input: &SaveProductInput,
) -> AppResult<(String, Option<String>, String, Vec<ValidatedComponent>)> {
    let product_code = input.product_code.trim().to_string();
    if product_code.is_empty() {
        return Err(AppError::Validation(
            "수입제품코드를 입력해 주세요.".to_string(),
        ));
    }

    let branch_name = input.overseas_branch_name.trim().to_string();
    if branch_name.is_empty() {
        return Err(AppError::Validation(
            "해외지사를 선택하거나 입력해 주세요.".to_string(),
        ));
    }

    if input.components.is_empty() {
        return Err(AppError::Validation(
            "제품 구성성분(BOM)을 최소 1개 이상 입력해 주세요.".to_string(),
        ));
    }

    let product_name = trim_optional(input.product_name.as_deref());
    let mut seen_cas: HashSet<String> = HashSet::with_capacity(input.components.len());
    let mut validated: Vec<ValidatedComponent> = Vec::with_capacity(input.components.len());
    let mut total_scaled: i64 = 0;

    for (idx, comp) in input.components.iter().enumerate() {
        let cas = normalize_cas_number(&comp.cas_number);
        if cas.is_empty() {
            return Err(AppError::Validation(format!(
                "{}번째 구성성분의 CAS번호를 입력해 주세요.",
                idx + 1
            )));
        }
        if !seen_cas.insert(cas.clone()) {
            return Err(AppError::Validation(format!(
                "동일한 제품 내에 중복된 CAS번호가 있습니다: '{cas}'"
            )));
        }

        let parsed = parse_decimal_4dp(&comp.weight_pct_text)?;
        if parsed.scaled_i64 <= 0 || parsed.scaled_i64 > HUNDRED_PERCENT_SCALED {
            return Err(AppError::Validation(format!(
                "함유율(%)은 0보다 크고 100 이하이어야 합니다: '{cas}' ({})",
                comp.weight_pct_text.trim()
            )));
        }

        total_scaled = total_scaled
            .checked_add(parsed.scaled_i64)
            .ok_or_else(|| AppError::Validation("함유율 합계가 허용 범위를 초과했습니다.".to_string()))?;

        validated.push(ValidatedComponent {
            cas_number: cas,
            substance_name: trim_optional(comp.substance_name.as_deref()),
            weight_pct_text: parsed.canonical_text,
            weight_pct_scaled: parsed.scaled_i64,
        });
    }

    if total_scaled != HUNDRED_PERCENT_SCALED {
        let sum_str = format_percentage(from_scaled_4dp(total_scaled));
        let diff_scaled = total_scaled - HUNDRED_PERCENT_SCALED;
        let diff_str = if diff_scaled > 0 {
            format!("+{}", format_percentage(from_scaled_4dp(diff_scaled)))
        } else {
            format!("-{}", format_percentage(from_scaled_4dp(-diff_scaled)))
        };
        return Err(AppError::Validation(format!(
            "함유율 합계가 정확히 100%여야 저장할 수 있습니다. (현재 합계: {sum_str}%, 차이: {diff_str}%)"
        )));
    }

    Ok((product_code, product_name, branch_name, validated))
}

fn upsert_product_row(
    tx: &Transaction<'_>,
    id: Option<i64>,
    product_code: &str,
    product_name: Option<&str>,
    overseas_branch_id: i64,
) -> AppResult<(i64, bool)> {
    match id {
        Some(target_id) => {
            let exists: Option<i64> = tx
                .query_row(
                    "SELECT id FROM imported_products WHERE id = ?1;",
                    [target_id],
                    |row| row.get(0),
                )
                .optional()?;
            if exists.is_none() {
                return Err(AppError::Validation(format!(
                    "수정할 수입제품을 찾을 수 없습니다 (ID: {target_id})."
                )));
            }

            let conflict: Option<i64> = tx
                .query_row(
                    "SELECT id FROM imported_products WHERE product_code = ?1 COLLATE NOCASE AND id != ?2;",
                    params![product_code, target_id],
                    |row| row.get(0),
                )
                .optional()?;
            if conflict.is_some() {
                return Err(AppError::Validation(format!(
                    "이미 사용 중인 수입제품코드입니다: '{product_code}'"
                )));
            }

            tx.execute(
                "UPDATE imported_products
                 SET product_code = ?1,
                     product_name = ?2,
                     overseas_branch_id = ?3,
                     updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                 WHERE id = ?4;",
                params![product_code, product_name, overseas_branch_id, target_id],
            )?;
            Ok((target_id, false))
        }
        None => {
            if let Some(existing_id) = tx
                .query_row(
                    "SELECT id FROM imported_products WHERE product_code = ?1 COLLATE NOCASE;",
                    [product_code],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?
            {
                tx.execute(
                    "UPDATE imported_products
                     SET product_code = ?1,
                         product_name = ?2,
                         overseas_branch_id = ?3,
                         updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                     WHERE id = ?4;",
                    params![product_code, product_name, overseas_branch_id, existing_id],
                )?;
                Ok((existing_id, false))
            } else {
                tx.execute(
                    "INSERT INTO imported_products (product_code, product_name, overseas_branch_id)
                     VALUES (?1, ?2, ?3);",
                    params![product_code, product_name, overseas_branch_id],
                )?;
                Ok((tx.last_insert_rowid(), true))
            }
        }
    }
}

/// Validates and saves an imported product and its 100.0000% BOM inside a single
/// `BEGIN IMMEDIATE ... COMMIT` SQLite transaction.
pub fn save_product_with_bom(
    conn: &mut Connection,
    input: &SaveProductInput,
) -> AppResult<SaveProductResult> {
    let (product_code, product_name, branch_name, components) = validate_save_input(input)?;

    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let branch_id = upsert_overseas_branch(&tx, &branch_name)?;
    let (product_id, created) = upsert_product_row(
        &tx,
        input.id,
        &product_code,
        product_name.as_deref(),
        branch_id,
    )?;

    tx.execute(
        "DELETE FROM product_components WHERE product_id = ?1;",
        [product_id],
    )?;

    for comp in components {
        let substance_id = upsert_chemical_substance(
            &tx,
            &comp.cas_number,
            comp.substance_name.as_deref(),
        )?;
        tx.execute(
            "INSERT INTO product_components (product_id, substance_id, weight_pct_text, weight_pct_scaled)
             VALUES (?1, ?2, ?3, ?4);",
            params![
                product_id,
                substance_id,
                comp.weight_pct_text,
                comp.weight_pct_scaled
            ],
        )?;
    }

    tx.commit()?;

    let product = get_product_detail(conn, product_id)?;
    Ok(SaveProductResult { product, created })
}

/// Deletes an imported product if zero `import_shipments` rows reference it.
/// Cascades to `product_components` via `ON DELETE CASCADE`.
pub fn delete_product(conn: &mut Connection, product_id: i64) -> AppResult<DeleteProductResult> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let product_code: String = tx
        .query_row(
            "SELECT product_code FROM imported_products WHERE id = ?1;",
            [product_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| {
            AppError::Validation(format!("삭제할 수입제품을 찾을 수 없습니다 (ID: {product_id})."))
        })?;

    let shipment_count: i64 = tx.query_row(
        "SELECT COUNT(*) FROM import_shipments WHERE product_id = ?1;",
        [product_id],
        |row| row.get(0),
    )?;

    if shipment_count > 0 {
        return Err(AppError::Validation(format!(
            "해당 수입제품으로 등록된 수입실적이 {shipment_count}건 존재하여 삭제할 수 없습니다. 수입실적을 먼저 삭제하거나 수정해 주세요."
        )));
    }

    tx.execute(
        "DELETE FROM imported_products WHERE id = ?1;",
        [product_id],
    )?;
    tx.commit()?;

    Ok(DeleteProductResult {
        deleted_product_id: product_id,
        deleted_product_code: product_code,
    })
}
