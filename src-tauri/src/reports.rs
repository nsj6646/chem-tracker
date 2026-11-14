//! Workspace 4: Annual Substance Total aggregation & mode-aware `.xlsx` export.
//! Implemented in Ticket 06.

use crate::decimal::{compute_substance_weight_kg, format_kg, format_percentage, format_ton};
use crate::domain::validate_calendar_date;
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection, OptionalExtension};
use rust_decimal::{Decimal, RoundingStrategy};
use rust_xlsxwriter::{Format, FormatAlign, Workbook, Worksheet, XlsxError};
use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::str::FromStr;
use unicode_width::UnicodeWidthStr;

fn serialize_decimal<S: serde::Serializer>(val: &Decimal, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&val.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportMode {
    /// Default external mode: omits confidential per-product recipe percentages.
    #[default]
    ExternalSummary,
    /// Internal verification mode: includes per-shipment recipe breakdown.
    InternalDetail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportPeriodMode {
    /// Filter shipments by `target_year` (`YYYY` calendar year, or all years if `None`).
    #[default]
    Year,
    /// Filter shipments by optional `start_date`..=`end_date` (never falls back to `target_year`).
    CustomRange,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Deserialize)]
pub struct AnnualReportFilter {
    pub period_mode: ReportPeriodMode,
    pub target_year: Option<i32>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub domestic_importer_id: Option<i64>,
    pub overseas_branch_id: Option<i64>,
    pub product_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ReportExportResult {
    pub saved_path: String,
    pub sheet_count: usize,
    pub message: String,
    pub report: AnnualSubstanceReport,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct FilterOption {
    pub id: i64,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProductFilterOption {
    pub id: i64,
    pub product_code: String,
    pub product_name: Option<String>,
    pub overseas_branch_id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ReportFilterOptions {
    pub years: Vec<i32>,
    pub domestic_importers: Vec<FilterOption>,
    pub overseas_branches: Vec<FilterOption>,
    pub imported_products: Vec<ProductFilterOption>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SubstanceShipmentBreakdown {
    pub shipment_id: i64,
    pub shipment_date: String,
    pub domestic_importer_name: String,
    pub overseas_branch_name: String,
    pub product_code: String,
    pub product_name: Option<String>,
    #[serde(serialize_with = "serialize_decimal")]
    pub import_weight_kg: Decimal,
    pub import_weight_kg_formatted: String,
    #[serde(serialize_with = "serialize_decimal")]
    pub weight_pct: Decimal,
    pub weight_pct_formatted: String,
    #[serde(serialize_with = "serialize_decimal")]
    pub substance_weight_kg: Decimal,
    pub substance_weight_kg_formatted: String,
    #[serde(serialize_with = "serialize_decimal")]
    pub substance_weight_ton: Decimal,
    pub substance_weight_ton_formatted: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SubstanceSummaryRow {
    pub no: usize,
    pub substance_id: i64,
    pub cas_number: String,
    pub substance_name: Option<String>,
    #[serde(serialize_with = "serialize_decimal")]
    pub total_substance_kg: Decimal,
    pub total_substance_kg_formatted: String,
    #[serde(serialize_with = "serialize_decimal")]
    pub total_substance_ton: Decimal,
    pub total_substance_ton_formatted: String,
    pub shipment_count: usize,
    pub breakdown: Vec<SubstanceShipmentBreakdown>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AnnualSubstanceReport {
    pub period_label: String,
    pub domestic_importer_label: String,
    pub overseas_branch_label: String,
    pub product_label: String,
    pub matching_shipment_count: usize,
    pub unique_substance_count: usize,
    #[serde(serialize_with = "serialize_decimal")]
    pub total_product_weight_kg: Decimal,
    pub total_product_weight_kg_formatted: String,
    #[serde(serialize_with = "serialize_decimal")]
    pub total_product_weight_ton: Decimal,
    pub total_product_weight_ton_formatted: String,
    #[serde(serialize_with = "serialize_decimal")]
    pub total_substance_kg: Decimal,
    pub total_substance_kg_formatted: String,
    #[serde(serialize_with = "serialize_decimal")]
    pub total_substance_ton: Decimal,
    pub total_substance_ton_formatted: String,
    pub rows: Vec<SubstanceSummaryRow>,
}

/// Loads available years and master entities for Workspace 4 filter controls.
pub fn get_report_filter_options(conn: &Connection) -> AppResult<ReportFilterOptions> {
    let mut year_stmt = conn.prepare(
        "SELECT DISTINCT shipment_year FROM import_shipments ORDER BY shipment_year DESC;",
    )?;
    let years = year_stmt
        .query_map([], |row| row.get::<_, i32>(0))?
        .collect::<Result<Vec<_>, _>>()?;

    let mut imp_stmt =
        conn.prepare("SELECT id, name FROM domestic_importers ORDER BY name COLLATE NOCASE ASC;")?;
    let domestic_importers = imp_stmt
        .query_map([], |row| {
            Ok(FilterOption {
                id: row.get(0)?,
                label: row.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut branch_stmt =
        conn.prepare("SELECT id, name FROM overseas_branches ORDER BY name COLLATE NOCASE ASC;")?;
    let overseas_branches = branch_stmt
        .query_map([], |row| {
            Ok(FilterOption {
                id: row.get(0)?,
                label: row.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut prod_stmt = conn.prepare(
        "SELECT id, product_code, product_name, overseas_branch_id
         FROM imported_products
         ORDER BY product_code COLLATE NOCASE ASC;",
    )?;
    let imported_products = prod_stmt
        .query_map([], |row| {
            Ok(ProductFilterOption {
                id: row.get(0)?,
                product_code: row.get(1)?,
                product_name: row.get(2)?,
                overseas_branch_id: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ReportFilterOptions {
        years,
        domestic_importers,
        overseas_branches,
        imported_products,
    })
}

/// Queries matching import shipments joined with product BOM recipes and
/// accumulates exact unrounded `rust_decimal::Decimal` annual totals per `CAS번호`.
pub fn query_annual_substance_totals(
    conn: &Connection,
    filter: &AnnualReportFilter,
) -> AppResult<AnnualSubstanceReport> {
    let normalized = normalize_filter(filter)?;
    let (start_date, end_date, effective_year) = match normalized.period_mode {
        ReportPeriodMode::Year => (None, None, normalized.target_year),
        ReportPeriodMode::CustomRange => (
            normalized.start_date.as_deref(),
            normalized.end_date.as_deref(),
            None,
        ),
    };

    let period_label = build_period_label(effective_year, start_date, end_date);
    let domestic_importer_label = resolve_optional_name(
        conn,
        "SELECT name FROM domestic_importers WHERE id = ?1;",
        normalized.domestic_importer_id,
        "전체 국내 수입사",
        "국내 수입사",
    )?;
    let overseas_branch_label = resolve_optional_name(
        conn,
        "SELECT name FROM overseas_branches WHERE id = ?1;",
        normalized.overseas_branch_id,
        "전체 해외지사",
        "해외지사",
    )?;
    let product_label = resolve_product_label(conn, normalized.product_id)?;

    let mut stmt = conn.prepare(
        "SELECT
            s.id AS shipment_id,
            s.shipment_date,
            di.name AS domestic_importer_name,
            ob.name AS overseas_branch_name,
            ip.product_code,
            ip.product_name,
            s.import_weight_kg_text,
            cs.id AS substance_id,
            cs.cas_number,
            cs.substance_name,
            pc.weight_pct_text
        FROM import_shipments s
        JOIN domestic_importers di ON di.id = s.domestic_importer_id
        JOIN overseas_branches ob  ON ob.id = s.overseas_branch_id
        JOIN imported_products ip  ON ip.id = s.product_id
        JOIN product_components pc ON pc.product_id = ip.id
        JOIN chemical_substances cs ON cs.id = pc.substance_id
        WHERE (?1 IS NULL OR s.shipment_year = ?1)
          AND (?2 IS NULL OR s.shipment_date >= ?2)
          AND (?3 IS NULL OR s.shipment_date <= ?3)
          AND (?4 IS NULL OR s.domestic_importer_id = ?4)
          AND (?5 IS NULL OR s.overseas_branch_id = ?5)
          AND (?6 IS NULL OR s.product_id = ?6)
        ORDER BY cs.cas_number ASC, s.shipment_date ASC, s.id ASC;",
    )?;

    struct SubstanceAcc {
        substance_id: i64,
        cas_number: String,
        substance_name: Option<String>,
        total_substance_kg: Decimal,
        breakdown: Vec<SubstanceShipmentBreakdown>,
    }

    let mut by_cas: BTreeMap<String, SubstanceAcc> = BTreeMap::new();
    let mut seen_shipments: HashSet<i64> = HashSet::new();
    let mut total_product_weight_kg = Decimal::ZERO;

    let rows_iter = stmt.query_map(
        params![
            effective_year,
            start_date,
            end_date,
            normalized.domestic_importer_id,
            normalized.overseas_branch_id,
            normalized.product_id,
        ],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, String>(10)?,
            ))
        },
    )?;

    for row_res in rows_iter {
        let (
            shipment_id,
            shipment_date,
            domestic_importer_name,
            overseas_branch_name,
            product_code,
            product_name,
            import_weight_kg_text,
            substance_id,
            cas_number,
            substance_name,
            weight_pct_text,
        ) = row_res?;

        let import_weight_kg = Decimal::from_str(&import_weight_kg_text).map_err(|_| {
            AppError::Validation(format!(
                "유효하지 않은 제품 수입량 데이터: '{import_weight_kg_text}'"
            ))
        })?;
        let weight_pct = Decimal::from_str(&weight_pct_text).map_err(|_| {
            AppError::Validation(format!("유효하지 않은 함유율 데이터: '{weight_pct_text}'"))
        })?;

        if seen_shipments.insert(shipment_id) {
            total_product_weight_kg += import_weight_kg;
        }

        let substance_weight_kg = compute_substance_weight_kg(import_weight_kg, weight_pct);
        let substance_weight_ton = substance_weight_kg / Decimal::ONE_THOUSAND;

        let entry = by_cas
            .entry(cas_number.clone())
            .or_insert_with(|| SubstanceAcc {
                substance_id,
                cas_number,
                substance_name,
                total_substance_kg: Decimal::ZERO,
                breakdown: Vec::new(),
            });

        entry.total_substance_kg += substance_weight_kg;
        entry.breakdown.push(SubstanceShipmentBreakdown {
            shipment_id,
            shipment_date,
            domestic_importer_name,
            overseas_branch_name,
            product_code,
            product_name,
            import_weight_kg,
            import_weight_kg_formatted: format_kg(import_weight_kg),
            weight_pct,
            weight_pct_formatted: format_percentage(weight_pct),
            substance_weight_kg,
            substance_weight_kg_formatted: format_kg(substance_weight_kg),
            substance_weight_ton,
            substance_weight_ton_formatted: format_ton(substance_weight_ton),
        });
    }

    let mut total_substance_kg = Decimal::ZERO;
    let mut rows = Vec::with_capacity(by_cas.len());
    for (idx, (_, acc)) in by_cas.into_iter().enumerate() {
        let row_ton = acc.total_substance_kg / Decimal::ONE_THOUSAND;
        total_substance_kg += acc.total_substance_kg;
        let shipment_count = acc.breakdown.len();
        rows.push(SubstanceSummaryRow {
            no: idx + 1,
            substance_id: acc.substance_id,
            cas_number: acc.cas_number,
            substance_name: acc.substance_name,
            total_substance_kg: acc.total_substance_kg,
            total_substance_kg_formatted: format_kg(acc.total_substance_kg),
            total_substance_ton: row_ton,
            total_substance_ton_formatted: format_ton(row_ton),
            shipment_count,
            breakdown: acc.breakdown,
        });
    }

    let total_product_weight_ton = total_product_weight_kg / Decimal::ONE_THOUSAND;
    let total_substance_ton = total_substance_kg / Decimal::ONE_THOUSAND;

    Ok(AnnualSubstanceReport {
        period_label,
        domestic_importer_label,
        overseas_branch_label,
        product_label,
        matching_shipment_count: seen_shipments.len(),
        unique_substance_count: rows.len(),
        total_product_weight_kg,
        total_product_weight_kg_formatted: format_kg(total_product_weight_kg),
        total_product_weight_ton,
        total_product_weight_ton_formatted: format_ton(total_product_weight_ton),
        total_substance_kg,
        total_substance_kg_formatted: format_kg(total_substance_kg),
        total_substance_ton,
        total_substance_ton_formatted: format_ton(total_substance_ton),
        rows,
    })
}

fn normalize_filter(filter: &AnnualReportFilter) -> AppResult<AnnualReportFilter> {
    match filter.period_mode {
        ReportPeriodMode::Year => Ok(AnnualReportFilter {
            period_mode: ReportPeriodMode::Year,
            target_year: filter.target_year,
            start_date: None,
            end_date: None,
            domestic_importer_id: filter.domestic_importer_id,
            overseas_branch_id: filter.overseas_branch_id,
            product_id: filter.product_id,
        }),
        ReportPeriodMode::CustomRange => {
            let start_date = clean_optional_date(filter.start_date.as_deref(), "시작일")?;
            let end_date = clean_optional_date(filter.end_date.as_deref(), "종료일")?;

            if let (Some(s), Some(e)) = (&start_date, &end_date) {
                if s > e {
                    return Err(AppError::Validation(
                        "조회 시작일은 종료일보다 늦을 수 없습니다.".to_string(),
                    ));
                }
            }

            Ok(AnnualReportFilter {
                period_mode: ReportPeriodMode::CustomRange,
                target_year: None,
                start_date,
                end_date,
                domestic_importer_id: filter.domestic_importer_id,
                overseas_branch_id: filter.overseas_branch_id,
                product_id: filter.product_id,
            })
        }
    }
}

fn clean_optional_date(raw: Option<&str>, field_label: &str) -> AppResult<Option<String>> {
    let Some(val) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let (year, month, day) = validate_calendar_date(val).map_err(|_| {
        AppError::Validation(format!(
            "{field_label}은 YYYY-MM-DD 형식이어야 합니다: '{val}'"
        ))
    })?;
    if !(1900..=2999).contains(&year) {
        return Err(AppError::Validation(format!(
            "{field_label}은 YYYY-MM-DD 형식이어야 합니다: '{val}'"
        )));
    }
    Ok(Some(format!("{year:04}-{month:02}-{day:02}")))
}

fn build_period_label(
    year: Option<i32>,
    start_date: Option<&str>,
    end_date: Option<&str>,
) -> String {
    match (year, start_date, end_date) {
        (_, Some(s), Some(e)) => format!("{s} ~ {e}"),
        (_, Some(s), None) => format!("{s} ~"),
        (_, None, Some(e)) => format!("~ {e}"),
        (Some(y), None, None) => format!("{y}년 (01.01 ~ 12.31)"),
        (None, None, None) => "전체 기간".to_string(),
    }
}

fn resolve_optional_name(
    conn: &Connection,
    sql: &str,
    id_opt: Option<i64>,
    all_label: &str,
    entity_label: &str,
) -> AppResult<String> {
    let Some(id) = id_opt else {
        return Ok(all_label.to_string());
    };
    let found: Option<String> = conn.query_row(sql, [id], |r| r.get(0)).optional()?;
    found.ok_or_else(|| {
        AppError::Validation(format!("선택한 {entity_label}(ID: {id})를 찾을 수 없습니다."))
    })
}

fn resolve_product_label(conn: &Connection, product_id: Option<i64>) -> AppResult<String> {
    let Some(id) = product_id else {
        return Ok("전체 수입제품".to_string());
    };
    let found: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT product_code, product_name FROM imported_products WHERE id = ?1;",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    match found {
        Some((code, Some(name))) if !name.trim().is_empty() => Ok(format!("{code} ({name})")),
        Some((code, _)) => Ok(code),
        None => Err(AppError::Validation(format!(
            "선택한 수입제품(ID: {id})을 찾을 수 없습니다."
        ))),
    }
}

/// Exports the annual substance total report as a mode-aware `.xlsx` workbook.
/// - `ExternalSummary`: single sheet `연간물질집계_요약`, zero confidential recipe data.
/// - `InternalDetail`: two sheets (`1_연간물질집계_요약` + `2_수입실적별_상세내역`).
pub fn export_annual_report_xlsx(
    conn: &Connection,
    filter: &AnnualReportFilter,
    mode: ReportMode,
    output_path: impl AsRef<Path>,
) -> AppResult<ReportExportResult> {
    let out_path = output_path.as_ref();
    if out_path.as_os_str().is_empty() || out_path.to_string_lossy().trim().is_empty() {
        return Err(AppError::Validation(
            "내보낼 엑셀 파일 경로를 지정해 주세요.".to_string(),
        ));
    }
    if out_path.is_dir() {
        return Err(AppError::Validation(format!(
            "유효한 파일 경로가 아닙니다 (디렉토리): {}",
            out_path.display()
        )));
    }

    let report = query_annual_substance_totals(conn, filter)?;
    if let Some(parent) = out_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let mut wb = Workbook::new();
    let styles = ExportFormats::new();

    let (sheet_count, message) = match mode {
        ReportMode::ExternalSummary => {
            let sheet = wb.add_worksheet();
            write_summary_sheet(sheet, "연간물질집계_요약", &report, &styles)
                .map_err(xlsx_to_app_error)?;
            (
                1usize,
                "대외 제출용 요약본(.xlsx, 1개 시트 · 함유율% 제외) 내보내기 완료 — CAS번호 텍스트(@) 포맷 적용".to_string(),
            )
        }
        ReportMode::InternalDetail => {
            let summary_sheet = wb.add_worksheet();
            write_summary_sheet(summary_sheet, "1_연간물질집계_요약", &report, &styles)
                .map_err(xlsx_to_app_error)?;

            let detail_sheet = wb.add_worksheet();
            write_detail_sheet(detail_sheet, "2_수입실적별_상세내역", &report, &styles)
                .map_err(xlsx_to_app_error)?;
            (
                2usize,
                "내부 검증용 상세본(.xlsx, 2개 시트) 내보내기 완료 — CAS번호·수입제품코드 텍스트(@) 포맷 적용".to_string(),
            )
        }
    };

    wb.save(out_path).map_err(xlsx_to_app_error)?;
    Ok(ReportExportResult {
        saved_path: out_path.display().to_string(),
        sheet_count,
        message,
        report,
    })
}

struct ExportFormats {
    header: Format,
    header_right: Format,
    text_at: Format,
    center: Format,
    kg_2: Format,
    kg_4: Format,
    ton_4: Format,
    ton_5: Format,
    pct_2: Format,
    pct_4: Format,
    bold_label: Format,
    bold_kg_2: Format,
    bold_kg_4: Format,
    bold_ton_4: Format,
    bold_ton_5: Format,
}

impl ExportFormats {
    fn new() -> Self {
        Self {
            header: Format::new().set_bold(),
            header_right: Format::new().set_bold().set_align(FormatAlign::Right),
            text_at: Format::new().set_num_format("@"),
            center: Format::new().set_align(FormatAlign::Center),
            kg_2: Format::new().set_num_format("#,##0.00"),
            kg_4: Format::new().set_num_format("#,##0.0000"),
            ton_4: Format::new().set_num_format("#,##0.0000"),
            ton_5: Format::new().set_num_format("#,##0.00000"),
            pct_2: Format::new().set_num_format("0.00"),
            pct_4: Format::new().set_num_format("0.0000"),
            bold_label: Format::new().set_bold().set_align(FormatAlign::Right),
            bold_kg_2: Format::new().set_bold().set_num_format("#,##0.00"),
            bold_kg_4: Format::new().set_bold().set_num_format("#,##0.0000"),
            bold_ton_4: Format::new().set_bold().set_num_format("#,##0.0000"),
            bold_ton_5: Format::new().set_bold().set_num_format("#,##0.00000"),
        }
    }
}

fn write_summary_sheet(
    sheet: &mut Worksheet,
    sheet_name: &str,
    report: &AnnualSubstanceReport,
    fmt: &ExportFormats,
) -> Result<(), XlsxError> {
    sheet.set_name(sheet_name)?;
    sheet.set_column_format(1, &fmt.text_at)?;

    let headers = [
        ("NO", false),
        ("CAS번호", false),
        ("화학물질명", false),
        ("연간 총 수입량 (kg)", true),
        ("연간 총 수입량 (ton)", true),
    ];
    let mut widths = [0usize; 5];

    for (col_idx, (title, is_right)) in headers.iter().enumerate() {
        let col = col_idx as u16;
        let header_fmt = if *is_right { &fmt.header_right } else { &fmt.header };
        sheet.write_string_with_format(0, col, title, header_fmt)?;
        widths[col_idx] = widths[col_idx].max(title.width());
    }

    for (idx, row) in report.rows.iter().enumerate() {
        let r = (idx + 1) as u32;
        let no_text = row.no.to_string();
        let name_text = row.substance_name.as_deref().unwrap_or("");

        sheet.write_number_with_format(r, 0, row.no as f64, &fmt.center)?;
        sheet.write_string_with_format(r, 1, &row.cas_number, &fmt.text_at)?;
        sheet.write_string(r, 2, name_text)?;

        let (kg_val, kg_needs_4) = kg_export_number(row.total_substance_kg);
        sheet.write_number_with_format(
            r,
            3,
            kg_val,
            if kg_needs_4 { &fmt.kg_4 } else { &fmt.kg_2 },
        )?;

        let (ton_val, ton_needs_5) = ton_export_number(row.total_substance_ton);
        sheet.write_number_with_format(
            r,
            4,
            ton_val,
            if ton_needs_5 { &fmt.ton_5 } else { &fmt.ton_4 },
        )?;

        widths[0] = widths[0].max(no_text.width());
        widths[1] = widths[1].max(row.cas_number.width());
        widths[2] = widths[2].max(name_text.width());
        widths[3] = widths[3].max(row.total_substance_kg_formatted.width());
        widths[4] = widths[4].max(row.total_substance_ton_formatted.width());
    }

    let footer_row = (report.rows.len() + 1) as u32;
    let footer_label = "합계";
    sheet.write_string_with_format(footer_row, 2, footer_label, &fmt.bold_label)?;
    let (total_kg_val, total_kg_4) = kg_export_number(report.total_substance_kg);
    sheet.write_number_with_format(
        footer_row,
        3,
        total_kg_val,
        if total_kg_4 { &fmt.bold_kg_4 } else { &fmt.bold_kg_2 },
    )?;
    let (total_ton_val, total_ton_5) = ton_export_number(report.total_substance_ton);
    sheet.write_number_with_format(
        footer_row,
        4,
        total_ton_val,
        if total_ton_5 { &fmt.bold_ton_5 } else { &fmt.bold_ton_4 },
    )?;

    widths[2] = widths[2].max(footer_label.width());
    widths[3] = widths[3].max(report.total_substance_kg_formatted.width());
    widths[4] = widths[4].max(report.total_substance_ton_formatted.width());

    apply_column_widths(sheet, &widths)?;
    Ok(())
}

fn write_detail_sheet(
    sheet: &mut Worksheet,
    sheet_name: &str,
    report: &AnnualSubstanceReport,
    fmt: &ExportFormats,
) -> Result<(), XlsxError> {
    sheet.set_name(sheet_name)?;
    sheet.set_column_format(0, &fmt.text_at)?;
    sheet.set_column_format(2, &fmt.text_at)?;
    sheet.set_column_format(5, &fmt.text_at)?;

    let headers = [
        ("CAS번호", false),
        ("화학물질명", false),
        ("수입일자", false),
        ("국내 수입사", false),
        ("해외지사", false),
        ("수입제품코드", false),
        ("제품명", false),
        ("제품 수입량 (kg)", true),
        ("함유율 (%)", true),
        ("화학물질 환산 중량 (kg)", true),
        ("화학물질 환산 중량 (ton)", true),
    ];
    let mut widths = [0usize; 11];

    for (col_idx, (title, is_right)) in headers.iter().enumerate() {
        let col = col_idx as u16;
        let header_fmt = if *is_right { &fmt.header_right } else { &fmt.header };
        sheet.write_string_with_format(0, col, title, header_fmt)?;
        widths[col_idx] = widths[col_idx].max(title.width());
    }

    let mut row_idx = 1u32;
    for summary_row in &report.rows {
        let substance_name = summary_row.substance_name.as_deref().unwrap_or("");
        for item in &summary_row.breakdown {
            let prod_name = item.product_name.as_deref().unwrap_or("");

            sheet.write_string_with_format(row_idx, 0, &summary_row.cas_number, &fmt.text_at)?;
            sheet.write_string(row_idx, 1, substance_name)?;
            sheet.write_string_with_format(row_idx, 2, &item.shipment_date, &fmt.text_at)?;
            sheet.write_string(row_idx, 3, &item.domestic_importer_name)?;
            sheet.write_string(row_idx, 4, &item.overseas_branch_name)?;
            sheet.write_string_with_format(row_idx, 5, &item.product_code, &fmt.text_at)?;
            sheet.write_string(row_idx, 6, prod_name)?;

            let (imp_kg, imp_needs_4) = kg_export_number(item.import_weight_kg);
            sheet.write_number_with_format(
                row_idx,
                7,
                imp_kg,
                if imp_needs_4 { &fmt.kg_4 } else { &fmt.kg_2 },
            )?;

            let (pct_val, pct_needs_4) = pct_export_number(item.weight_pct);
            sheet.write_number_with_format(
                row_idx,
                8,
                pct_val,
                if pct_needs_4 { &fmt.pct_4 } else { &fmt.pct_2 },
            )?;

            let (sub_kg, sub_needs_4) = kg_export_number(item.substance_weight_kg);
            sheet.write_number_with_format(
                row_idx,
                9,
                sub_kg,
                if sub_needs_4 { &fmt.kg_4 } else { &fmt.kg_2 },
            )?;

            let (sub_ton, sub_ton_5) = ton_export_number(item.substance_weight_ton);
            sheet.write_number_with_format(
                row_idx,
                10,
                sub_ton,
                if sub_ton_5 { &fmt.ton_5 } else { &fmt.ton_4 },
            )?;

            widths[0] = widths[0].max(summary_row.cas_number.width());
            widths[1] = widths[1].max(substance_name.width());
            widths[2] = widths[2].max(item.shipment_date.width());
            widths[3] = widths[3].max(item.domestic_importer_name.width());
            widths[4] = widths[4].max(item.overseas_branch_name.width());
            widths[5] = widths[5].max(item.product_code.width());
            widths[6] = widths[6].max(prod_name.width());
            widths[7] = widths[7].max(item.import_weight_kg_formatted.width());
            widths[8] = widths[8].max(item.weight_pct_formatted.width());
            widths[9] = widths[9].max(item.substance_weight_kg_formatted.width());
            widths[10] = widths[10].max(item.substance_weight_ton_formatted.width());

            row_idx += 1;
        }
    }

    apply_column_widths(sheet, &widths)?;
    Ok(())
}

fn apply_column_widths(sheet: &mut Worksheet, widths: &[usize]) -> Result<(), XlsxError> {
    for (col_idx, &w) in widths.iter().enumerate() {
        sheet.set_column_width(col_idx as u16, (w as f64) + 4.0)?;
    }
    Ok(())
}

fn kg_export_number(val: Decimal) -> (f64, bool) {
    let rounded = val
        .round_dp_with_strategy(4, RoundingStrategy::MidpointAwayFromZero)
        .normalize();
    (rounded.to_f64().unwrap_or(0.0), rounded.scale() > 2)
}

fn ton_export_number(val: Decimal) -> (f64, bool) {
    let rounded_4 = val
        .round_dp_with_strategy(4, RoundingStrategy::MidpointAwayFromZero)
        .normalize();
    if !val.is_zero() && rounded_4.is_zero() {
        let rounded_5 = val
            .round_dp_with_strategy(5, RoundingStrategy::MidpointAwayFromZero)
            .normalize();
        (rounded_5.to_f64().unwrap_or(0.0), true)
    } else {
        (rounded_4.to_f64().unwrap_or(0.0), false)
    }
}

fn pct_export_number(val: Decimal) -> (f64, bool) {
    let rounded = val
        .round_dp_with_strategy(4, RoundingStrategy::MidpointAwayFromZero)
        .normalize();
    (rounded.to_f64().unwrap_or(0.0), rounded.scale() > 2)
}

fn xlsx_to_app_error(err: XlsxError) -> AppError {
    AppError::Io(std::io::Error::other(err.to_string()))
}
