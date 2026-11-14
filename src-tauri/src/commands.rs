use crate::backup::{self, BackupResult, RestoreResult};
use crate::db::{self, DbStatus, SeedResult};
use crate::error::AppResult;
use crate::excel_import::{
    self, ImportCommitResult, ImportKind, ImportOptions, ImportPreviewResult,
    TemplateDownloadResult,
};
use crate::masters::{
    self, DeleteMasterResult, MasterEntityKind, MasterItem, UpdateMasterItemInput,
};
use crate::products::{
    self, ChemicalSubstanceOption, DeleteProductResult, OverseasBranchOption, ProductDetail,
    ProductListItem, SaveProductInput, SaveProductResult,
};
use crate::reports::{
    self, AnnualReportFilter, AnnualSubstanceReport, ReportExportResult, ReportFilterOptions,
    ReportMode,
};
use crate::shipments::{
    self, DomesticImporterOption, ProductOption, SaveShipmentInput, ShipmentFilter,
    ShipmentQueryResult, ShipmentRecord,
};
use crate::AppState;
use tauri::State;

#[tauri::command]
pub fn get_db_status(state: State<'_, AppState>) -> AppResult<DbStatus> {
    state.get_db_status()
}

#[tauri::command]
pub fn seed_sample_data(state: State<'_, AppState>) -> AppResult<SeedResult> {
    state.with_conn_mut(|conn, db_path| db::seed_sample_data(conn, db_path))
}

#[tauri::command]
pub fn list_master_items(
    state: State<'_, AppState>,
    kind: MasterEntityKind,
) -> AppResult<Vec<MasterItem>> {
    state.with_conn(|conn, _| masters::list_master_items(conn, kind))
}

#[tauri::command]
pub fn update_master_item(
    state: State<'_, AppState>,
    input: UpdateMasterItemInput,
) -> AppResult<MasterItem> {
    state.with_conn_mut(|conn, _| masters::update_master_item(conn, &input))
}

#[tauri::command]
pub fn delete_master_item(
    state: State<'_, AppState>,
    kind: MasterEntityKind,
    id: i64,
) -> AppResult<DeleteMasterResult> {
    state.with_conn_mut(|conn, _| masters::delete_master_item(conn, kind, id))
}

#[tauri::command]
pub fn list_products(
    state: State<'_, AppState>,
    search: Option<String>,
    overseas_branch_id: Option<i64>,
) -> AppResult<Vec<ProductListItem>> {
    state.with_conn(|conn, _| products::list_products(conn, search.as_deref(), overseas_branch_id))
}

#[tauri::command]
pub fn get_product_detail(
    state: State<'_, AppState>,
    product_id: i64,
) -> AppResult<ProductDetail> {
    state.with_conn(|conn, _| products::get_product_detail(conn, product_id))
}

#[tauri::command]
pub fn list_overseas_branches(state: State<'_, AppState>) -> AppResult<Vec<OverseasBranchOption>> {
    state.with_conn(|conn, _| products::list_overseas_branches(conn))
}

#[tauri::command]
pub fn search_chemical_substances(
    state: State<'_, AppState>,
    query: Option<String>,
) -> AppResult<Vec<ChemicalSubstanceOption>> {
    state.with_conn(|conn, _| products::search_chemical_substances(conn, query.as_deref()))
}

#[tauri::command]
pub fn save_product_with_bom(
    state: State<'_, AppState>,
    input: SaveProductInput,
) -> AppResult<SaveProductResult> {
    state.with_conn_mut(|conn, _| products::save_product_with_bom(conn, &input))
}

#[tauri::command]
pub fn delete_product(
    state: State<'_, AppState>,
    product_id: i64,
) -> AppResult<DeleteProductResult> {
    state.with_conn_mut(|conn, _| products::delete_product(conn, product_id))
}

#[tauri::command]
pub fn list_domestic_importers(
    state: State<'_, AppState>,
) -> AppResult<Vec<DomesticImporterOption>> {
    state.with_conn(|conn, _| shipments::list_domestic_importers(conn))
}

#[tauri::command]
pub fn list_product_options(state: State<'_, AppState>) -> AppResult<Vec<ProductOption>> {
    state.with_conn(|conn, _| shipments::list_product_options(conn))
}

#[tauri::command]
pub fn query_shipments(
    state: State<'_, AppState>,
    filter: ShipmentFilter,
) -> AppResult<ShipmentQueryResult> {
    state.with_conn(|conn, _| shipments::query_shipments(conn, filter))
}

#[tauri::command]
pub fn save_shipment(
    state: State<'_, AppState>,
    input: SaveShipmentInput,
) -> AppResult<ShipmentRecord> {
    state.with_conn(|conn, _| shipments::save_shipment(conn, input))
}

#[tauri::command]
pub fn delete_shipment(state: State<'_, AppState>, shipment_id: i64) -> AppResult<()> {
    state.with_conn(|conn, _| shipments::delete_shipment(conn, shipment_id))
}

#[tauri::command]
pub fn download_import_template(
    kind: ImportKind,
    output_path: String,
) -> AppResult<TemplateDownloadResult> {
    excel_import::download_import_template(kind, output_path)
}

#[tauri::command]
pub fn preview_excel_import(
    state: State<'_, AppState>,
    kind: ImportKind,
    file_path: String,
) -> AppResult<ImportPreviewResult> {
    state.with_conn(|conn, _| excel_import::preview_excel_import(conn, kind, file_path))
}

#[tauri::command]
pub fn commit_excel_import(
    state: State<'_, AppState>,
    kind: ImportKind,
    file_path: String,
    options: ImportOptions,
) -> AppResult<ImportCommitResult> {
    state.with_conn_mut(|conn, _| excel_import::commit_excel_import(conn, kind, file_path, options))
}

#[tauri::command]
pub fn get_report_filter_options(state: State<'_, AppState>) -> AppResult<ReportFilterOptions> {
    state.with_conn(|conn, _| reports::get_report_filter_options(conn))
}

#[tauri::command]
pub fn query_annual_substance_totals(
    state: State<'_, AppState>,
    filter: AnnualReportFilter,
) -> AppResult<AnnualSubstanceReport> {
    state.with_conn(|conn, _| reports::query_annual_substance_totals(conn, &filter))
}

#[tauri::command]
pub fn export_annual_report_xlsx(
    state: State<'_, AppState>,
    filter: AnnualReportFilter,
    mode: ReportMode,
    output_path: String,
) -> AppResult<ReportExportResult> {
    state.with_conn(|conn, _| reports::export_annual_report_xlsx(conn, &filter, mode, output_path))
}

#[tauri::command]
pub fn backup_database(state: State<'_, AppState>, target_path: String) -> AppResult<BackupResult> {
    state.with_conn(|conn, _| backup::backup_database(conn, target_path))
}

#[tauri::command]
pub fn restore_database(
    state: State<'_, AppState>,
    candidate_path: String,
) -> AppResult<RestoreResult> {
    state.restore_database(&candidate_path)
}

