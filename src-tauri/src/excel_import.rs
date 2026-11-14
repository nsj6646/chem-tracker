//! Workspace 3: Self-service Excel/CSV template download, preview validation & atomic import.
//! Implemented in Ticket 05.

use crate::decimal::{
    format_percentage, from_scaled_4dp, parse_decimal_4dp, HUNDRED_PERCENT_SCALED,
};
use crate::domain::{
    normalize_cas_number, upsert_chemical_substance, upsert_domestic_importer,
    upsert_overseas_branch,
};
use crate::error::{AppError, AppResult};
use crate::shipments::validate_shipment_date;
use calamine::{open_workbook_auto, Data, Reader};
use csv::{ReaderBuilder, Trim};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use rust_xlsxwriter::{Format, FormatAlign, Workbook, Worksheet, XlsxError};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportKind {
    Bom,
    Shipments,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewRowStatus {
    Valid,
    Conflict,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateDownloadResult {
    pub kind: ImportKind,
    pub output_path: String,
    pub sheet_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BomProductSummary {
    pub product_code: String,
    pub product_name: Option<String>,
    pub overseas_branch_name: String,
    pub row_numbers: Vec<usize>,
    pub component_count: usize,
    pub weight_pct_sum_scaled: i64,
    pub weight_pct_sum_display: String,
    pub exists_in_db: bool,
    pub status: PreviewRowStatus,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportPreviewRow {
    pub row_number: usize,
    pub status: PreviewRowStatus,
    pub is_conflict: bool,
    pub conflict_reason: Option<String>,
    pub errors: Vec<String>,
    pub overseas_branch_name: String,
    pub product_code: String,
    pub product_name: Option<String>,
    pub cas_number: Option<String>,
    pub substance_name: Option<String>,
    pub weight_pct_text: Option<String>,
    pub weight_pct_scaled: Option<i64>,
    pub shipment_date: Option<String>,
    pub domestic_importer_name: Option<String>,
    pub import_weight_kg_text: Option<String>,
    pub import_weight_kg_scaled: Option<i64>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportPreviewResult {
    pub kind: ImportKind,
    pub file_path: String,
    pub file_name: String,
    pub total_rows: usize,
    pub total_products: usize,
    pub valid_count: usize,
    pub conflict_count: usize,
    pub error_count: usize,
    pub can_commit: bool,
    pub unknown_product_codes: Vec<String>,
    pub bom_products: Vec<BomProductSummary>,
    pub rows: Vec<ImportPreviewRow>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportOptions {
    pub overwrite_existing: bool,
    pub skip_duplicates: bool,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            overwrite_existing: true,
            skip_duplicates: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportCommitResult {
    pub kind: ImportKind,
    pub inserted_count: usize,
    pub updated_count: usize,
    pub skipped_count: usize,
    pub total_processed: usize,
    pub message: String,
}

#[derive(Debug, Clone)]
enum RawCell {
    Text(String),
    ExcelDateSerial(f64),
    NumberFloat(f64),
    NumberInt(i64),
    Error(String),
    Empty,
}

impl RawCell {
    fn to_trimmed_text(&self) -> String {
        match self {
            RawCell::Text(s) => s.trim().to_string(),
            RawCell::ExcelDateSerial(serial) => {
                excel_serial_to_ymd(*serial).unwrap_or_else(|| serial.to_string())
            }
            RawCell::NumberFloat(f) => format_float_cell(*f),
            RawCell::NumberInt(i) => i.to_string(),
            RawCell::Error(e) => e.clone(),
            RawCell::Empty => String::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct RawRow {
    row_number: usize,
    cells: Vec<RawCell>,
}

pub fn default_template_filename(kind: ImportKind) -> &'static str {
    match kind {
        ImportKind::Bom => "제품구성성분_BOM_양식.xlsx",
        ImportKind::Shipments => "수입실적_양식.xlsx",
    }
}

/// Generates a pre-formatted `.xlsx` import template with `@` text formatting
/// on `CAS번호` (and date/code columns) so Excel preserves values like `50-00-0`.
pub fn download_import_template(
    kind: ImportKind,
    output_path: impl AsRef<Path>,
) -> AppResult<TemplateDownloadResult> {
    let out_path = output_path.as_ref();
    if out_path.as_os_str().is_empty() || out_path.is_dir() {
        return Err(AppError::Validation(
            "양식 저장 파일 경로가 올바르지 않습니다.".to_string(),
        ));
    }
    if let Some(parent) = out_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let mut wb = Workbook::new();
    let header_fmt = Format::new().set_bold();
    let header_right_fmt = Format::new().set_bold().set_align(FormatAlign::Right);
    let text_at_fmt = Format::new().set_num_format("@");
    let num_pct_fmt = Format::new().set_num_format("0.00");
    let num_kg_fmt = Format::new().set_num_format("#,##0.00");

    let sheet_name = match kind {
        ImportKind::Bom => {
            let sheet = wb.add_worksheet();
            let name = "제품구성성분_BOM";
            sheet.set_name(name).map_err(xlsx_to_app_error)?;
            sheet
                .set_column_format(1, &text_at_fmt)
                .map_err(xlsx_to_app_error)?;
            sheet
                .set_column_format(3, &text_at_fmt)
                .map_err(xlsx_to_app_error)?;

            let headers = [
                ("해외지사", false),
                ("수입제품코드", false),
                ("제품명(선택)", false),
                ("CAS번호", false),
                ("화학물질명(선택)", false),
                ("함유율(%)", true),
            ];
            let mut widths = [0usize; 6];
            for (col_idx, (title, right)) in headers.iter().enumerate() {
                let fmt = if *right { &header_right_fmt } else { &header_fmt };
                sheet
                    .write_string_with_format(0, col_idx as u16, title, fmt)
                    .map_err(xlsx_to_app_error)?;
                widths[col_idx] = widths[col_idx].max(title.width());
            }

            let sample_rows = [
                (
                    "유럽지사 (독일)",
                    "PRD-SAMPLE-01",
                    "샘플 코팅제",
                    "50-00-0",
                    "포름알데히드",
                    35.25,
                    "35.25",
                ),
                (
                    "유럽지사 (독일)",
                    "PRD-SAMPLE-01",
                    "샘플 코팅제",
                    "7732-18-5",
                    "정제수",
                    64.75,
                    "64.75",
                ),
            ];
            for (idx, (branch, code, pname, cas, sname, pct, pct_disp)) in
                sample_rows.iter().enumerate()
            {
                let r = (idx + 1) as u32;
                sheet.write_string(r, 0, branch).map_err(xlsx_to_app_error)?;
                sheet
                    .write_string_with_format(r, 1, code, &text_at_fmt)
                    .map_err(xlsx_to_app_error)?;
                sheet.write_string(r, 2, pname).map_err(xlsx_to_app_error)?;
                sheet
                    .write_string_with_format(r, 3, cas, &text_at_fmt)
                    .map_err(xlsx_to_app_error)?;
                sheet.write_string(r, 4, sname).map_err(xlsx_to_app_error)?;
                sheet
                    .write_number_with_format(r, 5, *pct, &num_pct_fmt)
                    .map_err(xlsx_to_app_error)?;

                widths[0] = widths[0].max(branch.width());
                widths[1] = widths[1].max(code.width());
                widths[2] = widths[2].max(pname.width());
                widths[3] = widths[3].max(cas.width());
                widths[4] = widths[4].max(sname.width());
                widths[5] = widths[5].max(pct_disp.width());
            }
            apply_column_widths(sheet, &widths).map_err(xlsx_to_app_error)?;
            name.to_string()
        }
        ImportKind::Shipments => {
            let sheet = wb.add_worksheet();
            let name = "수입실적";
            sheet.set_name(name).map_err(xlsx_to_app_error)?;
            sheet
                .set_column_format(0, &text_at_fmt)
                .map_err(xlsx_to_app_error)?;
            sheet
                .set_column_format(3, &text_at_fmt)
                .map_err(xlsx_to_app_error)?;

            let headers = [
                ("수입일자(YYYY-MM-DD)", false),
                ("국내 수입사", false),
                ("해외지사", false),
                ("수입제품코드", false),
                ("제품 수입량(kg)", true),
                ("비고(선택)", false),
            ];
            let mut widths = [0usize; 6];
            for (col_idx, (title, right)) in headers.iter().enumerate() {
                let fmt = if *right { &header_right_fmt } else { &header_fmt };
                sheet
                    .write_string_with_format(0, col_idx as u16, title, fmt)
                    .map_err(xlsx_to_app_error)?;
                widths[col_idx] = widths[col_idx].max(title.width());
            }

            let sample = (
                "2026-03-15",
                "(주)대한케미칼",
                "유럽지사 (독일)",
                "PRD-EU-101",
                1250.5,
                "1,250.50",
                "예시 데이터 (수정 후 사용)",
            );
            sheet
                .write_string_with_format(1, 0, sample.0, &text_at_fmt)
                .map_err(xlsx_to_app_error)?;
            sheet
                .write_string(1, 1, sample.1)
                .map_err(xlsx_to_app_error)?;
            sheet
                .write_string(1, 2, sample.2)
                .map_err(xlsx_to_app_error)?;
            sheet
                .write_string_with_format(1, 3, sample.3, &text_at_fmt)
                .map_err(xlsx_to_app_error)?;
            sheet
                .write_number_with_format(1, 4, sample.4, &num_kg_fmt)
                .map_err(xlsx_to_app_error)?;
            sheet
                .write_string(1, 5, sample.6)
                .map_err(xlsx_to_app_error)?;

            widths[0] = widths[0].max(sample.0.width());
            widths[1] = widths[1].max(sample.1.width());
            widths[2] = widths[2].max(sample.2.width());
            widths[3] = widths[3].max(sample.3.width());
            widths[4] = widths[4].max(sample.5.width());
            widths[5] = widths[5].max(sample.6.width());
            apply_column_widths(sheet, &widths).map_err(xlsx_to_app_error)?;
            name.to_string()
        }
    };

    wb.save(out_path).map_err(xlsx_to_app_error)?;

    Ok(TemplateDownloadResult {
        kind,
        output_path: out_path.display().to_string(),
        sheet_name,
    })
}

/// Parses and validates an uploaded `.xlsx`, `.xls`, or `.csv` file without
/// modifying SQLite, returning row/product status, conflicts, and errors.
pub fn preview_excel_import(
    conn: &Connection,
    kind: ImportKind,
    file_path: impl AsRef<Path>,
) -> AppResult<ImportPreviewResult> {
    let path = file_path.as_ref();
    if !path.exists() || path.is_dir() {
        return Err(AppError::Validation(format!(
            "가져올 파일을 찾을 수 없습니다: {}",
            path.display()
        )));
    }

    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("import_file")
        .to_string();
    let file_path_str = path.display().to_string();

    let (headers, raw_rows) = read_tabular_file(path, kind)?;

    match kind {
        ImportKind::Bom => preview_bom(conn, file_path_str, file_name, &headers, &raw_rows),
        ImportKind::Shipments => {
            preview_shipments(conn, file_path_str, file_name, &headers, &raw_rows)
        }
    }
}

fn read_tabular_file(path: &Path, kind: ImportKind) -> AppResult<(Vec<String>, Vec<RawRow>)> {
    let is_csv = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("csv"))
        .unwrap_or(false);

    if is_csv {
        read_csv_file(path)
    } else {
        read_spreadsheet_file(path, kind)
    }
}

fn read_csv_file(path: &Path) -> AppResult<(Vec<String>, Vec<RawRow>)> {
    let mut rdr = ReaderBuilder::new()
        .has_headers(false)
        .trim(Trim::All)
        .flexible(true)
        .from_path(path)
        .map_err(|e| AppError::Validation(format!("CSV 파일을 열 수 없습니다: {e}")))?;

    let mut all_rows: Vec<(usize, Vec<RawCell>)> = Vec::new();
    for (idx, rec_res) in rdr.records().enumerate() {
        let rec = rec_res
            .map_err(|e| AppError::Validation(format!("CSV {}행 파싱 오류: {e}", idx + 1)))?;
        let cells: Vec<RawCell> = rec
            .iter()
            .map(|s| {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    RawCell::Empty
                } else {
                    RawCell::Text(trimmed.to_string())
                }
            })
            .collect();
        all_rows.push((idx + 1, cells));
    }

    extract_header_and_data_rows(all_rows)
}

fn read_spreadsheet_file(path: &Path, kind: ImportKind) -> AppResult<(Vec<String>, Vec<RawRow>)> {
    let mut wb = open_workbook_auto(path)
        .map_err(|e| AppError::Validation(format!("엑셀 파일을 읽을 수 없습니다: {e}")))?;

    let names = wb.sheet_names();
    if names.is_empty() {
        return Err(AppError::Validation(
            "엑셀 파일에 시트가 존재하지 않습니다.".to_string(),
        ));
    }

    let preferred = match kind {
        ImportKind::Bom => "제품구성성분_BOM",
        ImportKind::Shipments => "수입실적",
    };
    let target_sheet = names
        .iter()
        .find(|n| n.as_str() == preferred)
        .cloned()
        .unwrap_or_else(|| names[0].clone());

    let range = wb
        .worksheet_range(&target_sheet)
        .map_err(|e| AppError::Validation(format!("시트 '{target_sheet}'를 읽을 수 없습니다: {e}")))?;

    let mut all_rows: Vec<(usize, Vec<RawCell>)> = Vec::with_capacity(range.height());
    for (r_idx, row) in range.rows().enumerate() {
        let cells: Vec<RawCell> = row.iter().map(calamine_cell_to_raw).collect();
        all_rows.push((r_idx + 1, cells));
    }

    extract_header_and_data_rows(all_rows)
}

fn extract_header_and_data_rows(
    all_rows: Vec<(usize, Vec<RawCell>)>,
) -> AppResult<(Vec<String>, Vec<RawRow>)> {
    let Some(header_pos) = all_rows
        .iter()
        .position(|(_, cells)| cells.iter().any(|c| !c.to_trimmed_text().is_empty()))
    else {
        return Err(AppError::Validation(
            "업로드한 파일이 비어 있습니다.".to_string(),
        ));
    };

    let headers: Vec<String> = all_rows[header_pos]
        .1
        .iter()
        .map(RawCell::to_trimmed_text)
        .collect();

    let data_rows: Vec<RawRow> = all_rows
        .into_iter()
        .skip(header_pos + 1)
        .filter(|(_, cells)| cells.iter().any(|c| !c.to_trimmed_text().is_empty()))
        .map(|(row_number, cells)| RawRow { row_number, cells })
        .collect();

    if data_rows.is_empty() {
        return Err(AppError::Validation(
            "헤더 외에 가져올 데이터 행이 없습니다.".to_string(),
        ));
    }

    Ok((headers, data_rows))
}

fn calamine_cell_to_raw(cell: &Data) -> RawCell {
    match cell {
        Data::Empty => RawCell::Empty,
        Data::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                RawCell::Empty
            } else {
                RawCell::Text(trimmed.to_string())
            }
        }
        Data::Int(i) => RawCell::NumberInt(*i),
        Data::Float(f) => RawCell::NumberFloat(*f),
        Data::Bool(b) => RawCell::Text(if *b { "TRUE" } else { "FALSE" }.to_string()),
        Data::DateTime(dt) => RawCell::ExcelDateSerial(dt.as_f64()),
        Data::DateTimeIso(s) | Data::DurationIso(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                RawCell::Empty
            } else {
                RawCell::Text(trimmed.to_string())
            }
        }
        Data::Error(err) => RawCell::Error(format!("#오류({err:?})")),
    }
}

fn format_float_cell(f: f64) -> String {
    if !f.is_finite() {
        return "#유효하지않은숫자".to_string();
    }
    if f == 0.0 {
        return "0".to_string();
    }
    f.to_string()
}

fn normalize_header_key(header: &str) -> String {
    header
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_lowercase()
}

fn find_column_index(headers: &[String], aliases: &[&str]) -> Option<usize> {
    let normalized_aliases: Vec<String> = aliases.iter().map(|a| normalize_header_key(a)).collect();
    headers
        .iter()
        .position(|h| normalized_aliases.contains(&normalize_header_key(h)))
}

fn get_cell_text(row: &RawRow, col_opt: Option<usize>) -> String {
    col_opt
        .and_then(|idx| row.cells.get(idx))
        .map(RawCell::to_trimmed_text)
        .unwrap_or_default()
}

struct BomColumnMap {
    overseas_branch: usize,
    product_code: usize,
    product_name: Option<usize>,
    cas_number: usize,
    substance_name: Option<usize>,
    weight_pct: usize,
}

fn resolve_bom_columns(headers: &[String]) -> AppResult<BomColumnMap> {
    let overseas_branch = find_column_index(headers, &["해외지사"]);
    let product_code = find_column_index(headers, &["수입제품코드"]);
    let product_name = find_column_index(headers, &["제품명(선택)", "제품명"]);
    let cas_number = find_column_index(headers, &["cas번호", "CAS번호"]);
    let substance_name = find_column_index(headers, &["화학물질명(선택)", "화학물질명"]);
    let weight_pct = find_column_index(headers, &["함유율(%)", "함유율"]);

    let mut missing = Vec::new();
    if overseas_branch.is_none() {
        missing.push("해외지사");
    }
    if product_code.is_none() {
        missing.push("수입제품코드");
    }
    if cas_number.is_none() {
        missing.push("CAS번호");
    }
    if weight_pct.is_none() {
        missing.push("함유율(%)");
    }
    if !missing.is_empty() {
        return Err(AppError::Validation(format!(
            "제품 구성성분(BOM) 필수 열이 누락되었습니다: {}",
            missing.join(", ")
        )));
    }

    Ok(BomColumnMap {
        overseas_branch: overseas_branch.unwrap(),
        product_code: product_code.unwrap(),
        product_name,
        cas_number: cas_number.unwrap(),
        substance_name,
        weight_pct: weight_pct.unwrap(),
    })
}

fn preview_bom(
    conn: &Connection,
    file_path: String,
    file_name: String,
    headers: &[String],
    raw_rows: &[RawRow],
) -> AppResult<ImportPreviewResult> {
    let cols = resolve_bom_columns(headers)?;

    let mut existing_codes: HashSet<String> = HashSet::new();
    {
        let mut stmt = conn.prepare("SELECT product_code FROM imported_products;")?;
        let codes = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        for c in codes {
            existing_codes.insert(c.trim().to_lowercase());
        }
    }

    struct DraftRow {
        preview: ImportPreviewRow,
        group_key: Option<String>,
    }

    let mut draft_rows: Vec<DraftRow> = Vec::with_capacity(raw_rows.len());
    let mut group_order: Vec<String> = Vec::new();
    let mut group_indices: HashMap<String, Vec<usize>> = HashMap::new();

    for raw in raw_rows {
        let branch = get_cell_text(raw, Some(cols.overseas_branch));
        let product_code = get_cell_text(raw, Some(cols.product_code));
        let product_name_str = get_cell_text(raw, cols.product_name);
        let product_name = if product_name_str.is_empty() {
            None
        } else {
            Some(product_name_str)
        };
        let cas = normalize_cas_number(&get_cell_text(raw, Some(cols.cas_number)));
        let sub_name_str = get_cell_text(raw, cols.substance_name);
        let substance_name = if sub_name_str.is_empty() {
            None
        } else {
            Some(sub_name_str)
        };
        let pct_raw = get_cell_text(raw, Some(cols.weight_pct));

        let mut errors = Vec::new();
        if branch.is_empty() {
            errors.push("해외지사가 비어 있습니다.".to_string());
        }
        if product_code.is_empty() {
            errors.push("수입제품코드가 비어 있습니다.".to_string());
        }
        if cas.is_empty() {
            errors.push("CAS번호가 비어 있습니다.".to_string());
        }

        let mut weight_pct_text = None;
        let mut weight_pct_scaled = None;
        if pct_raw.is_empty() {
            errors.push("함유율(%)이 비어 있습니다.".to_string());
        } else {
            match parse_decimal_4dp(&pct_raw) {
                Ok(parsed) => {
                    if parsed.scaled_i64 <= 0 || parsed.scaled_i64 > HUNDRED_PERCENT_SCALED {
                        errors.push(format!(
                            "함유율(%)은 0보다 크고 100 이하이어야 합니다: '{pct_raw}'"
                        ));
                    } else {
                        weight_pct_text = Some(parsed.canonical_text);
                        weight_pct_scaled = Some(parsed.scaled_i64);
                    }
                }
                Err(e) => errors.push(format!("함유율(%) 오류: {e}")),
            }
        }

        let group_key = if product_code.is_empty() {
            None
        } else {
            let key = product_code.to_lowercase();
            let row_idx = draft_rows.len();
            group_indices
                .entry(key.clone())
                .or_insert_with(|| {
                    group_order.push(key.clone());
                    Vec::new()
                })
                .push(row_idx);
            Some(key)
        };

        let status = if errors.is_empty() {
            PreviewRowStatus::Valid
        } else {
            PreviewRowStatus::Error
        };

        draft_rows.push(DraftRow {
            preview: ImportPreviewRow {
                row_number: raw.row_number,
                status,
                is_conflict: false,
                conflict_reason: None,
                errors,
                overseas_branch_name: branch,
                product_code,
                product_name,
                cas_number: if cas.is_empty() { None } else { Some(cas) },
                substance_name,
                weight_pct_text,
                weight_pct_scaled,
                shipment_date: None,
                domestic_importer_name: None,
                import_weight_kg_text: None,
                import_weight_kg_scaled: None,
                note: None,
            },
            group_key,
        });
    }

    let mut bom_products: Vec<BomProductSummary> = Vec::with_capacity(group_order.len());
    let mut valid_product_count = 0usize;
    let mut conflict_product_count = 0usize;

    for key in &group_order {
        let indices = &group_indices[key];
        let first_idx = indices[0];
        let canonical_code = draft_rows[first_idx].preview.product_code.clone();
        let row_numbers: Vec<usize> = indices
            .iter()
            .map(|&i| draft_rows[i].preview.row_number)
            .collect();

        let mut group_errors: Vec<String> = Vec::new();
        let mut branch_canonical: Option<String> = None;
        let mut product_name_canonical: Option<String> = None;
        let mut cas_positions: HashMap<String, Vec<usize>> = HashMap::new();
        let mut all_pct_parsed = true;
        let mut sum_scaled: i64 = 0;

        for &idx in indices {
            let row = &draft_rows[idx].preview;
            if !row.overseas_branch_name.is_empty() {
                match &branch_canonical {
                    None => branch_canonical = Some(row.overseas_branch_name.clone()),
                    Some(first_branch)
                        if !first_branch.eq_ignore_ascii_case(&row.overseas_branch_name)
                            && first_branch.to_lowercase()
                                != row.overseas_branch_name.to_lowercase() =>
                    {
                        let msg = format!(
                            "수입제품코드 '{canonical_code}'에 서로 다른 해외지사('{first_branch}' vs '{}')가 지정되었습니다.",
                            row.overseas_branch_name
                        );
                        if !group_errors.contains(&msg) {
                            group_errors.push(msg);
                        }
                    }
                    _ => {}
                }
            }
            if let Some(ref pname) = row.product_name {
                match &product_name_canonical {
                    None => product_name_canonical = Some(pname.clone()),
                    Some(first_name) if first_name != pname => {
                        let msg = format!(
                            "수입제품코드 '{canonical_code}'에 서로 다른 제품명('{first_name}' vs '{pname}')이 입력되었습니다."
                        );
                        if !group_errors.contains(&msg) {
                            group_errors.push(msg);
                        }
                    }
                    _ => {}
                }
            }
            if let Some(ref cas) = row.cas_number {
                cas_positions.entry(cas.clone()).or_default().push(idx);
            }
            match row.weight_pct_scaled {
                Some(scaled) => {
                    sum_scaled = sum_scaled.saturating_add(scaled);
                }
                None => {
                    all_pct_parsed = false;
                }
            }
        }

        for (cas, pos_list) in &cas_positions {
            if pos_list.len() > 1 {
                let msg = format!(
                    "동일한 제품 '{canonical_code}' 내에 중복된 CAS번호가 있습니다: '{cas}'"
                );
                group_errors.push(msg.clone());
                for &idx in pos_list {
                    if !draft_rows[idx].preview.errors.contains(&msg) {
                        draft_rows[idx].preview.errors.push(msg.clone());
                    }
                }
            }
        }

        if all_pct_parsed && sum_scaled != HUNDRED_PERCENT_SCALED {
            let sum_str = format_percentage(from_scaled_4dp(sum_scaled));
            let diff_scaled = sum_scaled - HUNDRED_PERCENT_SCALED;
            let diff_str = if diff_scaled > 0 {
                format!("+{}", format_percentage(from_scaled_4dp(diff_scaled)))
            } else {
                format!("-{}", format_percentage(from_scaled_4dp(-diff_scaled)))
            };
            let msg = format!(
                "제품 '{canonical_code}'의 함유율 합계가 {sum_str}%입니다. 정확히 100%여야 합니다 (차이: {diff_str}%)."
            );
            group_errors.push(msg.clone());
            for &idx in indices {
                draft_rows[idx].preview.errors.push(msg.clone());
            }
        }

        let any_row_error = indices
            .iter()
            .any(|&idx| !draft_rows[idx].preview.errors.is_empty());

        for &idx in indices {
            if !group_errors.is_empty() {
                for g_err in &group_errors {
                    if !draft_rows[idx].preview.errors.contains(g_err) {
                        draft_rows[idx].preview.errors.push(g_err.clone());
                    }
                }
            }
            if any_row_error && draft_rows[idx].preview.errors.is_empty() {
                draft_rows[idx].preview.errors.push(format!(
                    "동일 제품 '{canonical_code}'의 다른 행에 오류가 있어 가져올 수 없습니다."
                ));
            }
        }

        let exists_in_db = existing_codes.contains(key);
        let prod_status = if any_row_error || !group_errors.is_empty() {
            PreviewRowStatus::Error
        } else if exists_in_db {
            conflict_product_count += 1;
            PreviewRowStatus::Conflict
        } else {
            valid_product_count += 1;
            PreviewRowStatus::Valid
        };

        for &idx in indices {
            draft_rows[idx].preview.status = prod_status;
            if prod_status == PreviewRowStatus::Conflict {
                draft_rows[idx].preview.is_conflict = true;
                draft_rows[idx].preview.conflict_reason = Some(
                    "기존 DB에 동일한 수입제품코드가 존재합니다 (덮어쓰기/건너뛰기 대상)."
                        .to_string(),
                );
            }
        }

        let mut summary_errors = group_errors;
        for &idx in indices {
            for e in &draft_rows[idx].preview.errors {
                if !summary_errors.contains(e) {
                    summary_errors.push(e.clone());
                }
            }
        }

        bom_products.push(BomProductSummary {
            product_code: canonical_code,
            product_name: product_name_canonical,
            overseas_branch_name: branch_canonical.unwrap_or_default(),
            row_numbers,
            component_count: indices.len(),
            weight_pct_sum_scaled: sum_scaled,
            weight_pct_sum_display: format_percentage(from_scaled_4dp(sum_scaled)),
            exists_in_db,
            status: prod_status,
            errors: summary_errors,
        });
    }

    let rows: Vec<ImportPreviewRow> = draft_rows
        .into_iter()
        .map(|d| {
            let _ = d.group_key;
            d.preview
        })
        .collect();
    let error_count = rows
        .iter()
        .filter(|r| r.status == PreviewRowStatus::Error)
        .count();

    Ok(ImportPreviewResult {
        kind: ImportKind::Bom,
        file_path,
        file_name,
        total_rows: rows.len(),
        total_products: bom_products.len(),
        valid_count: valid_product_count,
        conflict_count: conflict_product_count,
        error_count,
        can_commit: error_count == 0 && !rows.is_empty(),
        unknown_product_codes: Vec::new(),
        bom_products,
        rows,
    })
}

struct ShipmentColumnMap {
    shipment_date: usize,
    domestic_importer: usize,
    overseas_branch: usize,
    product_code: usize,
    import_weight_kg: usize,
    note: Option<usize>,
}

fn resolve_shipment_columns(headers: &[String]) -> AppResult<ShipmentColumnMap> {
    let shipment_date = find_column_index(
        headers,
        &["수입일자(YYYY-MM-DD)", "수입일자(yyyy-mm-dd)", "수입일자"],
    );
    let domestic_importer = find_column_index(headers, &["국내 수입사", "국내수입사"]);
    let overseas_branch = find_column_index(headers, &["해외지사"]);
    let product_code = find_column_index(headers, &["수입제품코드"]);
    let import_weight_kg = find_column_index(
        headers,
        &["제품 수입량(kg)", "제품수입량(kg)", "제품 수입량"],
    );
    let note = find_column_index(headers, &["비고(선택)", "비고"]);

    let mut missing = Vec::new();
    if shipment_date.is_none() {
        missing.push("수입일자(YYYY-MM-DD)");
    }
    if domestic_importer.is_none() {
        missing.push("국내 수입사");
    }
    if overseas_branch.is_none() {
        missing.push("해외지사");
    }
    if product_code.is_none() {
        missing.push("수입제품코드");
    }
    if import_weight_kg.is_none() {
        missing.push("제품 수입량(kg)");
    }
    if !missing.is_empty() {
        return Err(AppError::Validation(format!(
            "수입실적 필수 열이 누락되었습니다: {}",
            missing.join(", ")
        )));
    }

    Ok(ShipmentColumnMap {
        shipment_date: shipment_date.unwrap(),
        domestic_importer: domestic_importer.unwrap(),
        overseas_branch: overseas_branch.unwrap(),
        product_code: product_code.unwrap(),
        import_weight_kg: import_weight_kg.unwrap(),
        note,
    })
}

type ShipmentDupKey = (String, String, String, String, i64);

struct KnownProductInfo {
    product_code: String,
    product_name: Option<String>,
    branch_name: String,
}

fn preview_shipments(
    conn: &Connection,
    file_path: String,
    file_name: String,
    headers: &[String],
    raw_rows: &[RawRow],
) -> AppResult<ImportPreviewResult> {
    let cols = resolve_shipment_columns(headers)?;

    let mut known_products: HashMap<String, KnownProductInfo> = HashMap::new();
    {
        let mut stmt = conn.prepare(
            "SELECT ip.product_code, ip.product_name, ob.name
             FROM imported_products ip
             JOIN overseas_branches ob ON ob.id = ip.overseas_branch_id;",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for (code, pname, bname) in rows {
            known_products.insert(
                code.trim().to_lowercase(),
                KnownProductInfo {
                    product_code: code,
                    product_name: pname,
                    branch_name: bname,
                },
            );
        }
    }

    let mut db_dup_keys: HashSet<ShipmentDupKey> = HashSet::new();
    {
        let mut stmt = conn.prepare(
            "SELECT
                s.shipment_date,
                di.name,
                ob.name,
                ip.product_code,
                s.import_weight_kg_scaled
             FROM import_shipments s
             JOIN domestic_importers di ON di.id = s.domestic_importer_id
             JOIN overseas_branches ob ON ob.id = s.overseas_branch_id
             JOIN imported_products ip ON ip.id = s.product_id;",
        )?;
        let existing = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i64>(4)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for (date, imp, br, code, weight_scaled) in existing {
            db_dup_keys.insert((
                date.trim().to_string(),
                imp.trim().to_lowercase(),
                br.trim().to_lowercase(),
                code.trim().to_lowercase(),
                weight_scaled,
            ));
        }
    }

    let mut preview_rows: Vec<ImportPreviewRow> = Vec::with_capacity(raw_rows.len());
    let mut batch_seen: HashMap<ShipmentDupKey, usize> = HashMap::new();
    let mut unknown_codes: Vec<String> = Vec::new();
    let mut unknown_seen: HashSet<String> = HashSet::new();
    let mut valid_count = 0usize;
    let mut conflict_count = 0usize;

    for raw in raw_rows {
        let date_cell = raw
            .cells
            .get(cols.shipment_date)
            .unwrap_or(&RawCell::Empty);
        let importer = get_cell_text(raw, Some(cols.domestic_importer));
        let branch = get_cell_text(raw, Some(cols.overseas_branch));
        let product_code = get_cell_text(raw, Some(cols.product_code));
        let weight_raw = get_cell_text(raw, Some(cols.import_weight_kg));
        let note_str = get_cell_text(raw, cols.note);
        let note = if note_str.is_empty() {
            None
        } else {
            Some(note_str)
        };

        let mut errors = Vec::new();

        let normalized_date = match normalize_import_date(date_cell) {
            Ok(d) => Some(d),
            Err(e) => {
                errors.push(e.to_string());
                None
            }
        };

        if importer.is_empty() {
            errors.push("국내 수입사가 비어 있습니다.".to_string());
        }
        if branch.is_empty() {
            errors.push("해외지사가 비어 있습니다.".to_string());
        }

        let mut resolved_product_name: Option<String> = None;
        let mut effective_branch_name = branch.clone();
        let mut effective_product_code = product_code.clone();

        if product_code.is_empty() {
            errors.push("수입제품코드가 비어 있습니다.".to_string());
        } else {
            let code_key = product_code.to_lowercase();
            match known_products.get(&code_key) {
                Some(info) => {
                    resolved_product_name = info.product_name.clone();
                    effective_product_code = info.product_code.clone();
                    if !branch.is_empty()
                        && branch.to_lowercase() != info.branch_name.to_lowercase()
                    {
                        errors.push(format!(
                            "수입제품코드 '{}'의 등록된 해외지사('{}')와 일치하지 않습니다 (입력값: '{}').",
                            info.product_code, info.branch_name, branch
                        ));
                    } else if !branch.is_empty() {
                        effective_branch_name = info.branch_name.clone();
                    }
                }
                None => {
                    if unknown_seen.insert(code_key) {
                        unknown_codes.push(product_code.clone());
                    }
                    errors.push(format!(
                        "등록되지 않은 수입제품코드입니다: '{product_code}'. 먼저 '제품 구성성분(BOM)'을 등록하거나 가져와 주세요."
                    ));
                }
            }
        }

        let mut weight_text = None;
        let mut weight_scaled = None;
        if weight_raw.is_empty() {
            errors.push("제품 수입량(kg)이 비어 있습니다.".to_string());
        } else {
            match parse_decimal_4dp(&weight_raw) {
                Ok(parsed) => {
                    if parsed.scaled_i64 <= 0 {
                        errors.push(format!(
                            "제품 수입량(kg)은 0보다 커야 합니다: '{weight_raw}'"
                        ));
                    } else {
                        weight_text = Some(parsed.canonical_text);
                        weight_scaled = Some(parsed.scaled_i64);
                    }
                }
                Err(e) => errors.push(format!("제품 수입량(kg) 오류: {e}")),
            }
        }

        let mut is_conflict = false;
        let mut conflict_reason = None;
        let status = if !errors.is_empty() {
            PreviewRowStatus::Error
        } else {
            let key: ShipmentDupKey = (
                normalized_date.clone().unwrap_or_default(),
                importer.to_lowercase(),
                effective_branch_name.to_lowercase(),
                effective_product_code.to_lowercase(),
                weight_scaled.unwrap_or(0),
            );
            if db_dup_keys.contains(&key) {
                is_conflict = true;
                conflict_count += 1;
                conflict_reason = Some(
                    "기존 DB에 5개 항목(수입일자·국내수입사·해외지사·수입제품코드·수입량)이 동일한 수입실적이 이미 존재합니다."
                        .to_string(),
                );
                PreviewRowStatus::Conflict
            } else if let Some(&first_row) = batch_seen.get(&key) {
                is_conflict = true;
                conflict_count += 1;
                conflict_reason = Some(format!(
                    "업로드 파일 내 중복 의심 건입니다 ({first_row}행과 5개 항목 동일)."
                ));
                PreviewRowStatus::Conflict
            } else {
                batch_seen.insert(key, raw.row_number);
                valid_count += 1;
                PreviewRowStatus::Valid
            }
        };

        preview_rows.push(ImportPreviewRow {
            row_number: raw.row_number,
            status,
            is_conflict,
            conflict_reason,
            errors,
            overseas_branch_name: effective_branch_name,
            product_code: effective_product_code,
            product_name: resolved_product_name,
            cas_number: None,
            substance_name: None,
            weight_pct_text: None,
            weight_pct_scaled: None,
            shipment_date: normalized_date.or_else(|| {
                let raw_text = date_cell.to_trimmed_text();
                if raw_text.is_empty() {
                    None
                } else {
                    Some(raw_text)
                }
            }),
            domestic_importer_name: if importer.is_empty() {
                None
            } else {
                Some(importer)
            },
            import_weight_kg_text: weight_text.or_else(|| {
                if weight_raw.is_empty() {
                    None
                } else {
                    Some(weight_raw)
                }
            }),
            import_weight_kg_scaled: weight_scaled,
            note,
        });
    }

    let error_count = preview_rows
        .iter()
        .filter(|r| r.status == PreviewRowStatus::Error)
        .count();

    Ok(ImportPreviewResult {
        kind: ImportKind::Shipments,
        file_path,
        file_name,
        total_rows: preview_rows.len(),
        total_products: 0,
        valid_count,
        conflict_count,
        error_count,
        can_commit: error_count == 0 && !preview_rows.is_empty(),
        unknown_product_codes: unknown_codes,
        bom_products: Vec::new(),
        rows: preview_rows,
    })
}

fn normalize_import_date(cell: &RawCell) -> AppResult<String> {
    match cell {
        RawCell::Empty => Err(AppError::Validation("수입일자가 비어 있습니다.".to_string())),
        RawCell::Error(e) => Err(AppError::Validation(format!("수입일자 셀 오류: {e}"))),
        RawCell::ExcelDateSerial(serial) => {
            let ymd = excel_serial_to_ymd(*serial).ok_or_else(|| {
                AppError::Validation(format!("올바른 엑셀 날짜 시리얼이 아닙니다: '{serial}'"))
            })?;
            validate_shipment_date(&ymd)
        }
        RawCell::NumberInt(n) => normalize_numeric_date(*n as f64),
        RawCell::NumberFloat(f) => normalize_numeric_date(*f),
        RawCell::Text(s) => normalize_text_date(s),
    }
}

fn normalize_numeric_date(val: f64) -> AppResult<String> {
    if !val.is_finite() {
        return Err(AppError::Validation("올바른 수입일자 숫자가 아닙니다.".to_string()));
    }
    if val.fract().abs() < 1e-9 && (19_000_101.0..=21_001_231.0).contains(&val) {
        let digits = format!("{:.0}", val);
        return normalize_text_date(&digits);
    }
    let ymd = excel_serial_to_ymd(val).ok_or_else(|| {
        AppError::Validation(format!("올바른 수입일자(YYYY-MM-DD 또는 엑셀 날짜)가 아닙니다: '{val}'"))
    })?;
    validate_shipment_date(&ymd)
}

fn normalize_text_date(raw: &str) -> AppResult<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation("수입일자가 비어 있습니다.".to_string()));
    }

    // Strip optional ISO time portion ('2026-03-15T09:30:00Z' or '2026-03-15 09:30:00')
    let date_part = trimmed
        .split_once('T')
        .or_else(|| trimmed.split_once(' '))
        .map(|(d, _)| d.trim())
        .unwrap_or(trimmed);

    // Compact 8-digit YYYYMMDD
    if date_part.len() == 8 && date_part.bytes().all(|b| b.is_ascii_digit()) {
        let formatted = format!(
            "{}-{}-{}",
            &date_part[0..4],
            &date_part[4..6],
            &date_part[6..8]
        );
        return validate_shipment_date(&formatted);
    }

    // Accept '-', '/', '.' separators with 1- or 2-digit month/day
    let normalized_sep = date_part.replace(['/', '.'], "-");
    let parts: Vec<&str> = normalized_sep.split('-').collect();
    if parts.len() == 3
        && parts[0].len() == 4
        && parts[0].bytes().all(|b| b.is_ascii_digit())
        && (1..=2).contains(&parts[1].len())
        && parts[1].bytes().all(|b| b.is_ascii_digit())
        && (1..=2).contains(&parts[2].len())
        && parts[2].bytes().all(|b| b.is_ascii_digit())
    {
        let y: i32 = parts[0].parse().unwrap_or(0);
        let m: u32 = parts[1].parse().unwrap_or(0);
        let d: u32 = parts[2].parse().unwrap_or(0);
        let formatted = format!("{y:04}-{m:02}-{d:02}");
        return validate_shipment_date(&formatted);
    }

    validate_shipment_date(trimmed)
}

/// Re-validates the file and atomically commits all valid rows inside a single
/// `BEGIN IMMEDIATE ... COMMIT` SQLite transaction, rolling back completely on any error.
pub fn commit_excel_import(
    conn: &mut Connection,
    kind: ImportKind,
    file_path: impl AsRef<Path>,
    options: ImportOptions,
) -> AppResult<ImportCommitResult> {
    let preview = preview_excel_import(conn, kind, file_path)?;
    if !preview.can_commit || preview.error_count > 0 {
        let first_errors: Vec<String> = preview
            .rows
            .iter()
            .filter(|r| !r.errors.is_empty())
            .take(3)
            .map(|r| format!("{}행: {}", r.row_number, r.errors.join("; ")))
            .collect();
        return Err(AppError::Validation(format!(
            "검증 오류가 {}건 있어 일괄 가져오기를 실행할 수 없습니다. ({})",
            preview.error_count,
            first_errors.join(" | ")
        )));
    }

    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let result = match kind {
        ImportKind::Bom => commit_bom_import(&tx, &preview, options.overwrite_existing)?,
        ImportKind::Shipments => commit_shipments_import(&tx, &preview, options.skip_duplicates)?,
    };
    tx.commit()?;
    Ok(result)
}

fn commit_bom_import(
    tx: &Transaction<'_>,
    preview: &ImportPreviewResult,
    overwrite_existing: bool,
) -> AppResult<ImportCommitResult> {
    let mut rows_by_product: HashMap<String, Vec<&ImportPreviewRow>> = HashMap::new();
    for row in &preview.rows {
        rows_by_product
            .entry(row.product_code.to_lowercase())
            .or_default()
            .push(row);
    }

    let mut inserted_count = 0usize;
    let mut updated_count = 0usize;
    let mut skipped_count = 0usize;

    for product in &preview.bom_products {
        let existing_id: Option<i64> = tx
            .query_row(
                "SELECT id FROM imported_products WHERE product_code = ?1 COLLATE NOCASE;",
                [&product.product_code],
                |r| r.get(0),
            )
            .optional()?;

        if existing_id.is_some() && !overwrite_existing {
            skipped_count += 1;
            continue;
        }

        let branch_id = upsert_overseas_branch(tx, &product.overseas_branch_name)?;
        let product_id = match existing_id {
            Some(pid) => {
                tx.execute(
                    "UPDATE imported_products
                     SET product_code = ?1,
                         product_name = ?2,
                         overseas_branch_id = ?3,
                         updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                     WHERE id = ?4;",
                    params![
                        product.product_code,
                        product.product_name,
                        branch_id,
                        pid
                    ],
                )?;
                tx.execute(
                    "DELETE FROM product_components WHERE product_id = ?1;",
                    [pid],
                )?;
                updated_count += 1;
                pid
            }
            None => {
                tx.execute(
                    "INSERT INTO imported_products (product_code, product_name, overseas_branch_id)
                     VALUES (?1, ?2, ?3);",
                    params![product.product_code, product.product_name, branch_id],
                )?;
                inserted_count += 1;
                tx.last_insert_rowid()
            }
        };

        let comp_rows = rows_by_product
            .get(&product.product_code.to_lowercase())
            .ok_or_else(|| {
                AppError::Validation(format!(
                    "제품 '{}'의 구성성분 행을 찾을 수 없습니다.",
                    product.product_code
                ))
            })?;

        for row in comp_rows {
            let cas = row.cas_number.as_deref().ok_or_else(|| {
                AppError::Validation(format!("{}행의 CAS번호가 비어 있습니다.", row.row_number))
            })?;
            let pct_text = row.weight_pct_text.as_deref().ok_or_else(|| {
                AppError::Validation(format!("{}행의 함유율(%)이 비어 있습니다.", row.row_number))
            })?;
            let pct_scaled = row.weight_pct_scaled.ok_or_else(|| {
                AppError::Validation(format!("{}행의 함유율(%) 스케일 오류.", row.row_number))
            })?;

            let substance_id =
                upsert_chemical_substance(tx, cas, row.substance_name.as_deref())?;
            tx.execute(
                "INSERT INTO product_components (product_id, substance_id, weight_pct_text, weight_pct_scaled)
                 VALUES (?1, ?2, ?3, ?4);",
                params![product_id, substance_id, pct_text, pct_scaled],
            )?;
        }
    }

    let total_processed = inserted_count + updated_count + skipped_count;
    let message = format!(
        "제품 구성성분(BOM) 가져오기 완료: 신규 등록 {inserted_count}개, 덮어쓰기 {updated_count}개, 건너뛰기 {skipped_count}개"
    );

    Ok(ImportCommitResult {
        kind: ImportKind::Bom,
        inserted_count,
        updated_count,
        skipped_count,
        total_processed,
        message,
    })
}

fn commit_shipments_import(
    tx: &Transaction<'_>,
    preview: &ImportPreviewResult,
    skip_duplicates: bool,
) -> AppResult<ImportCommitResult> {
    let mut inserted_count = 0usize;
    let mut skipped_count = 0usize;

    for row in &preview.rows {
        if row.is_conflict && skip_duplicates {
            skipped_count += 1;
            continue;
        }

        let shipment_date = row.shipment_date.as_deref().ok_or_else(|| {
            AppError::Validation(format!("{}행의 수입일자가 비어 있습니다.", row.row_number))
        })?;
        let importer_name = row.domestic_importer_name.as_deref().ok_or_else(|| {
            AppError::Validation(format!("{}행의 국내 수입사가 비어 있습니다.", row.row_number))
        })?;
        let weight_text = row.import_weight_kg_text.as_deref().ok_or_else(|| {
            AppError::Validation(format!("{}행의 제품 수입량(kg)이 비어 있습니다.", row.row_number))
        })?;
        let weight_scaled = row.import_weight_kg_scaled.ok_or_else(|| {
            AppError::Validation(format!("{}행의 제품 수입량(kg) 스케일 오류.", row.row_number))
        })?;

        let (product_id, branch_id): (i64, i64) = tx
            .query_row(
                "SELECT id, overseas_branch_id
                 FROM imported_products
                 WHERE product_code = ?1 COLLATE NOCASE;",
                [&row.product_code],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?
            .ok_or_else(|| {
                AppError::Validation(format!(
                    "등록되지 않은 수입제품코드입니다: '{}'",
                    row.product_code
                ))
            })?;

        let importer_id = upsert_domestic_importer(tx, importer_name)?;

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
                importer_id,
                branch_id,
                product_id,
                weight_text,
                weight_scaled,
                row.note
            ],
        )?;
        inserted_count += 1;
    }

    let total_processed = inserted_count + skipped_count;
    let message = format!(
        "수입실적 일괄 가져오기 완료: 신규 추가 {inserted_count}건, 중복 건너뛰기 {skipped_count}건"
    );

    Ok(ImportCommitResult {
        kind: ImportKind::Shipments,
        inserted_count,
        updated_count: 0,
        skipped_count,
        total_processed,
        message,
    })
}

fn excel_serial_to_ymd(serial: f64) -> Option<String> {
    if !serial.is_finite() || serial < 1.0 || serial > 109_574.0 {
        return None;
    }
    let total_ms = (serial * 86_400_000.0).round() as i64;
    let serial_days = total_ms.div_euclid(86_400_000);
    if serial_days == 60 {
        // Non-existent Excel 1900-02-29 bug day
        return None;
    }
    let unix_days = if serial_days < 60 {
        serial_days - 25_568
    } else {
        serial_days - 25_569
    };
    let (year, month, day) = civil_from_days(unix_days);
    if !(1900..=2100).contains(&year) {
        return None;
    }
    Some(format!("{year:04}-{month:02}-{day:02}"))
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year as i32, m as u32, d as u32)
}

fn apply_column_widths(sheet: &mut Worksheet, widths: &[usize]) -> Result<(), XlsxError> {
    for (col_idx, w) in widths.iter().enumerate() {
        let fitted = ((*w).max(8) as f64) + 3.0;
        sheet.set_column_width(col_idx as u16, fitted)?;
    }
    Ok(())
}

fn xlsx_to_app_error(err: XlsxError) -> AppError {
    AppError::Validation(format!("엑셀 파일 생성 오류: {err}"))
}
