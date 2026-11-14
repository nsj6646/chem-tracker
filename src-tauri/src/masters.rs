//! Compact Master Management Modals (`국내 수입사`, `해외지사`, `화학물질`) & delete guards.
//! Implemented in Ticket 04.

use crate::domain::normalize_cas_number as shared_normalize_cas_number;
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MasterEntityKind {
    DomesticImporter,
    OverseasBranch,
    ChemicalSubstance,
}

impl MasterEntityKind {
    pub fn korean_label(self) -> &'static str {
        match self {
            MasterEntityKind::DomesticImporter => "국내 수입사",
            MasterEntityKind::OverseasBranch => "해외지사",
            MasterEntityKind::ChemicalSubstance => "화학물질",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MasterItem {
    pub id: i64,
    pub kind: MasterEntityKind,
    pub name: Option<String>,
    pub cas_number: Option<String>,
    pub substance_name: Option<String>,
    pub note: Option<String>,
    pub reference_count: i64,
    pub product_count: i64,
    pub component_count: i64,
    pub shipment_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateMasterItemInput {
    pub id: i64,
    pub kind: MasterEntityKind,
    pub name: Option<String>,
    pub cas_number: Option<String>,
    pub substance_name: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteMasterResult {
    pub kind: MasterEntityKind,
    pub deleted_id: i64,
    pub deleted_label: String,
}

fn trim_optional(input: Option<&str>) -> Option<String> {
    input
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Normalizes a CAS number (re-exported from `crate::domain::normalize_cas_number`).
pub fn normalize_cas_number(raw: &str) -> String {
    shared_normalize_cas_number(raw)
}

/// Lists all master records of `kind` with their current foreign-key reference counts.
pub fn list_master_items(
    conn: &Connection,
    kind: MasterEntityKind,
) -> AppResult<Vec<MasterItem>> {
    query_master_items(conn, kind, None)
}

/// Queries either all master items of `kind` or a single row filtered directly by primary key `id`.
fn query_master_items(
    conn: &Connection,
    kind: MasterEntityKind,
    target_id: Option<i64>,
) -> AppResult<Vec<MasterItem>> {
    let id_val = target_id.unwrap_or_default();
    let single_param: [&dyn rusqlite::ToSql; 1] = [&id_val];
    let params_slice: &[&dyn rusqlite::ToSql] = if target_id.is_some() {
        &single_param
    } else {
        &[]
    };
    match kind {
        MasterEntityKind::DomesticImporter => {
            let sql = if target_id.is_some() {
                "SELECT
                    di.id,
                    di.name,
                    di.note,
                    (SELECT COUNT(*) FROM import_shipments s WHERE s.domestic_importer_id = di.id) AS shipment_count
                 FROM domestic_importers di
                 WHERE di.id = ?1;"
            } else {
                "SELECT
                    di.id,
                    di.name,
                    di.note,
                    (SELECT COUNT(*) FROM import_shipments s WHERE s.domestic_importer_id = di.id) AS shipment_count
                 FROM domestic_importers di
                 ORDER BY di.name COLLATE NOCASE ASC, di.id ASC;"
            };
            let mut stmt = conn.prepare(sql)?;
            let rows = stmt
                .query_map(params_slice, |row| {
                    let shipment_count: i64 = row.get(3)?;
                    Ok(MasterItem {
                        id: row.get(0)?,
                        kind,
                        name: Some(row.get(1)?),
                        cas_number: None,
                        substance_name: None,
                        note: row.get(2)?,
                        reference_count: shipment_count,
                        product_count: 0,
                        component_count: 0,
                        shipment_count,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        }
        MasterEntityKind::OverseasBranch => {
            let sql = if target_id.is_some() {
                "SELECT
                    ob.id,
                    ob.name,
                    ob.note,
                    (SELECT COUNT(*) FROM imported_products ip WHERE ip.overseas_branch_id = ob.id) AS product_count,
                    (SELECT COUNT(*) FROM import_shipments s WHERE s.overseas_branch_id = ob.id) AS shipment_count
                 FROM overseas_branches ob
                 WHERE ob.id = ?1;"
            } else {
                "SELECT
                    ob.id,
                    ob.name,
                    ob.note,
                    (SELECT COUNT(*) FROM imported_products ip WHERE ip.overseas_branch_id = ob.id) AS product_count,
                    (SELECT COUNT(*) FROM import_shipments s WHERE s.overseas_branch_id = ob.id) AS shipment_count
                 FROM overseas_branches ob
                 ORDER BY ob.name COLLATE NOCASE ASC, ob.id ASC;"
            };
            let mut stmt = conn.prepare(sql)?;
            let rows = stmt
                .query_map(params_slice, |row| {
                    let product_count: i64 = row.get(3)?;
                    let shipment_count: i64 = row.get(4)?;
                    Ok(MasterItem {
                        id: row.get(0)?,
                        kind,
                        name: Some(row.get(1)?),
                        cas_number: None,
                        substance_name: None,
                        note: row.get(2)?,
                        reference_count: product_count + shipment_count,
                        product_count,
                        component_count: 0,
                        shipment_count,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        }
        MasterEntityKind::ChemicalSubstance => {
            let sql = if target_id.is_some() {
                "SELECT
                    cs.id,
                    cs.cas_number,
                    cs.substance_name,
                    (SELECT COUNT(*) FROM product_components pc WHERE pc.substance_id = cs.id) AS component_count
                 FROM chemical_substances cs
                 WHERE cs.id = ?1;"
            } else {
                "SELECT
                    cs.id,
                    cs.cas_number,
                    cs.substance_name,
                    (SELECT COUNT(*) FROM product_components pc WHERE pc.substance_id = cs.id) AS component_count
                 FROM chemical_substances cs
                 ORDER BY cs.cas_number ASC, cs.id ASC;"
            };
            let mut stmt = conn.prepare(sql)?;
            let rows = stmt
                .query_map(params_slice, |row| {
                    let component_count: i64 = row.get(3)?;
                    Ok(MasterItem {
                        id: row.get(0)?,
                        kind,
                        name: None,
                        cas_number: Some(row.get(1)?),
                        substance_name: row.get(2)?,
                        note: None,
                        reference_count: component_count,
                        product_count: component_count,
                        component_count,
                        shipment_count: 0,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        }
    }
}

fn fetch_master_item(
    conn: &Connection,
    kind: MasterEntityKind,
    id: i64,
) -> AppResult<MasterItem> {
    query_master_items(conn, kind, Some(id))?
        .into_iter()
        .next()
        .ok_or_else(|| {
            AppError::Validation(format!(
                "{} 항목을 찾을 수 없습니다 (ID: {id}).",
                kind.korean_label()
            ))
        })
}

fn update_named_master_tx(
    tx: &Transaction<'_>,
    table: &str,
    label: &str,
    id: i64,
    raw_name: Option<&str>,
    raw_note: Option<&str>,
) -> AppResult<()> {
    let trimmed_name = raw_name.map(str::trim).unwrap_or("");
    if trimmed_name.is_empty() {
        return Err(AppError::Validation(format!(
            "{label} 이름을 입력해 주세요."
        )));
    }

    let exists: Option<i64> = tx
        .query_row(
            &format!("SELECT id FROM {table} WHERE id = ?1;"),
            [id],
            |row| row.get(0),
        )
        .optional()?;
    if exists.is_none() {
        return Err(AppError::Validation(format!(
            "수정할 {label}을(를) 찾을 수 없습니다 (ID: {id})."
        )));
    }

    let conflict: Option<i64> = tx
        .query_row(
            &format!("SELECT id FROM {table} WHERE name = ?1 COLLATE NOCASE AND id != ?2;"),
            params![trimmed_name, id],
            |row| row.get(0),
        )
        .optional()?;
    if conflict.is_some() {
        return Err(AppError::Validation(format!(
            "이미 존재하는 {label}입니다: '{trimmed_name}'"
        )));
    }

    let note = trim_optional(raw_note);
    tx.execute(
        &format!(
            "UPDATE {table}
             SET name = ?1,
                 note = ?2,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
             WHERE id = ?3;"
        ),
        params![trimmed_name, note, id],
    )?;
    Ok(())
}

/// Validates and updates a master record (`DomesticImporter`, `OverseasBranch`, or
/// `ChemicalSubstance`) in place so all referencing views update via foreign key IDs.
pub fn update_master_item(
    conn: &mut Connection,
    input: &UpdateMasterItemInput,
) -> AppResult<MasterItem> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

    match input.kind {
        MasterEntityKind::DomesticImporter => {
            update_named_master_tx(
                &tx,
                "domestic_importers",
                input.kind.korean_label(),
                input.id,
                input.name.as_deref(),
                input.note.as_deref(),
            )?;
        }
        MasterEntityKind::OverseasBranch => {
            update_named_master_tx(
                &tx,
                "overseas_branches",
                input.kind.korean_label(),
                input.id,
                input.name.as_deref(),
                input.note.as_deref(),
            )?;
        }
        MasterEntityKind::ChemicalSubstance => {
            let cas = normalize_cas_number(input.cas_number.as_deref().unwrap_or(""));
            if cas.is_empty() {
                return Err(AppError::Validation(
                    "CAS번호를 입력해 주세요.".to_string(),
                ));
            }

            let exists: Option<i64> = tx
                .query_row(
                    "SELECT id FROM chemical_substances WHERE id = ?1;",
                    [input.id],
                    |row| row.get(0),
                )
                .optional()?;
            if exists.is_none() {
                return Err(AppError::Validation(format!(
                    "수정할 화학물질을 찾을 수 없습니다 (ID: {}).",
                    input.id
                )));
            }

            let conflict: Option<i64> = tx
                .query_row(
                    "SELECT id FROM chemical_substances WHERE cas_number = ?1 AND id != ?2;",
                    params![cas, input.id],
                    |row| row.get(0),
                )
                .optional()?;
            if conflict.is_some() {
                return Err(AppError::Validation(format!(
                    "이미 존재하는 CAS번호입니다: '{cas}'"
                )));
            }

            let substance_name = trim_optional(input.substance_name.as_deref());
            tx.execute(
                "UPDATE chemical_substances
                 SET cas_number = ?1,
                     substance_name = ?2,
                     updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                 WHERE id = ?3;",
                params![cas, substance_name, input.id],
            )?;
        }
    }

    tx.commit()?;
    fetch_master_item(conn, input.kind, input.id)
}

fn format_delete_blocked_message(item: &MasterItem) -> String {
    match item.kind {
        MasterEntityKind::DomesticImporter => {
            let label = item.name.as_deref().unwrap_or("");
            format!(
                "해당 국내 수입사('{label}')을(를) 참조하는 수입실적이 {}건 존재하여 삭제할 수 없습니다. 수입실적을 먼저 삭제하거나 수정해 주세요.",
                item.shipment_count
            )
        }
        MasterEntityKind::OverseasBranch => {
            let label = item.name.as_deref().unwrap_or("");
            if item.product_count > 0 && item.shipment_count > 0 {
                format!(
                    "해당 해외지사('{label}')을(를) 참조하는 레코드가 총 {}건(수입제품 {}건, 수입실적 {}건) 존재하여 삭제할 수 없습니다.",
                    item.reference_count, item.product_count, item.shipment_count
                )
            } else if item.product_count > 0 {
                format!(
                    "해당 해외지사('{label}')을(를) 참조하는 수입제품이 {}건 존재하여 삭제할 수 없습니다.",
                    item.product_count
                )
            } else {
                format!(
                    "해당 해외지사('{label}')을(를) 참조하는 수입실적이 {}건 존재하여 삭제할 수 없습니다.",
                    item.shipment_count
                )
            }
        }
        MasterEntityKind::ChemicalSubstance => {
            let cas = item.cas_number.as_deref().unwrap_or("");
            format!(
                "해당 화학물질('{cas}')을(를) 참조하는 제품 구성성분이 {}건 존재하여 삭제할 수 없습니다.",
                item.component_count
            )
        }
    }
}

/// Deletes an unreferenced master item (`DomesticImporter`, `OverseasBranch`, or
/// `ChemicalSubstance`). If any product, BOM component, or shipment references it,
/// deletion is blocked by reference-count verification and `ON DELETE RESTRICT` with
/// an explicit Korean error stating the exact number of referencing records.
pub fn delete_master_item(
    conn: &mut Connection,
    kind: MasterEntityKind,
    id: i64,
) -> AppResult<DeleteMasterResult> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let item = query_master_items(&tx, kind, Some(id))?
        .into_iter()
        .next()
        .ok_or_else(|| {
            AppError::Validation(format!(
                "삭제할 {}을(를) 찾을 수 없습니다 (ID: {id}).",
                kind.korean_label()
            ))
        })?;

    if item.reference_count > 0 {
        return Err(AppError::Validation(format_delete_blocked_message(&item)));
    }

    let (table, deleted_label) = match kind {
        MasterEntityKind::DomesticImporter => (
            "domestic_importers",
            item.name.clone().unwrap_or_default(),
        ),
        MasterEntityKind::OverseasBranch => (
            "overseas_branches",
            item.name.clone().unwrap_or_default(),
        ),
        MasterEntityKind::ChemicalSubstance => (
            "chemical_substances",
            item.cas_number.clone().unwrap_or_default(),
        ),
    };

    if let Err(err) = tx.execute(&format!("DELETE FROM {table} WHERE id = ?1;"), [id]) {
        if let rusqlite::Error::SqliteFailure(sqlite_err, _) = &err {
            if sqlite_err.code == rusqlite::ErrorCode::ConstraintViolation {
                return Err(AppError::Validation(format_delete_blocked_message(&item)));
            }
        }
        return Err(err.into());
    }

    tx.commit()?;
    Ok(DeleteMasterResult {
        kind,
        deleted_id: id,
        deleted_label,
    })
}
