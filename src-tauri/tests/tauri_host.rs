use chem_tracker::{build_app, resolve_db_path, AppState};
use serde_json::{json, Value};
use std::path::PathBuf;
use tauri::Manager;

fn unique_temp_dir(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("chem_tracker_{name}_{}_{}", std::process::id(), nanos));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn ipc_call(
    webview: &tauri::WebviewWindow<tauri::test::MockRuntime>,
    cmd: &str,
    body: Value,
) -> Result<Value, String> {
    let req = tauri::webview::InvokeRequest {
        cmd: cmd.into(),
        callback: tauri::ipc::CallbackFn(0),
        error: tauri::ipc::CallbackFn(1),
        url: webview.url().expect("webview url"),
        body: tauri::ipc::InvokeBody::Json(body),
        headers: Default::default(),
        invoke_key: tauri::test::INVOKE_KEY.to_string(),
    };
    match tauri::test::get_ipc_response(webview, req) {
        Ok(resp) => Ok(resp.deserialize::<Value>().expect("deserialize json")),
        Err(err) => Err(err.to_string()),
    }
}

fn test_context() -> tauri::Context<tauri::test::MockRuntime> {
    tauri::generate_context!()
}

struct MockHost {
    temp_dir: PathBuf,
    app: tauri::App<tauri::test::MockRuntime>,
}

impl MockHost {
    fn launch(name: &str) -> Self {
        let temp_dir = unique_temp_dir(name);
        let db_path = temp_dir.join("data.db");
        #[allow(unused_mut)]
        let mut app = build_app(tauri::test::mock_builder(), Some(db_path))
            .build(test_context())
            .expect("build tauri mock app");
        #[allow(deprecated)]
        app.run_iteration(|_, _| {});
        Self { temp_dir, app }
    }

    fn main_window(&self) -> tauri::WebviewWindow<tauri::test::MockRuntime> {
        self.app
            .get_webview_window("main")
            .expect("main window from tauri.conf.json")
    }
}

impl Drop for MockHost {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.temp_dir);
    }
}

#[test]
fn resolve_db_path_and_build_app_initializes_wal_state() {
    let custom_dir = PathBuf::from("/tmp/koreabranch-appdata");
    assert_eq!(
        resolve_db_path(Some(custom_dir.clone())),
        custom_dir.join("data.db")
    );
    assert_eq!(resolve_db_path(None), chem_tracker::db::default_db_path());

    let temp_dir = unique_temp_dir("host_init");
    let db_path = temp_dir.join("data.db");

    let context = tauri::test::mock_context(tauri::test::noop_assets());
    #[allow(unused_mut)]
    let mut app = build_app(tauri::test::mock_builder(), Some(db_path.clone()))
        .build(context)
        .expect("build tauri mock app");
    #[allow(deprecated)]
    app.run_iteration(|_, _| {});

    let state = app.state::<AppState>();
    let status = state.get_db_status().expect("get_db_status via AppState");
    assert_eq!(status.db_path, db_path.display().to_string());
    assert!(status.journal_mode.eq_ignore_ascii_case("wal"));
    assert!(status.integrity_ok);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn db_masters_and_products_ipc_round_trip() {
    let host = MockHost::launch("ipc_products");
    let webview = host.main_window();

    // 1. db commands
    let status = ipc_call(&webview, "get_db_status", json!({})).expect("get_db_status");
    assert_eq!(status["imported_products_count"], 0);

    let seed = ipc_call(&webview, "seed_sample_data", json!({})).expect("seed_sample_data");
    assert_eq!(seed["already_seeded"], false);
    assert_eq!(seed["inserted_products"], 3);

    // 2. masters commands (snake_case enum + camelCase args)
    let branches = ipc_call(
        &webview,
        "list_master_items",
        json!({ "kind": "overseas_branch" }),
    )
    .expect("list_master_items");
    let branch_arr = branches.as_array().expect("branches array");
    assert_eq!(branch_arr.len(), 3);
    let first_branch_id = branch_arr[0]["id"].as_i64().unwrap();

    let updated_branch = ipc_call(
        &webview,
        "update_master_item",
        json!({
            "input": {
                "id": first_branch_id,
                "kind": "overseas_branch",
                "name": branch_arr[0]["name"],
                "cas_number": null,
                "substance_name": null,
                "note": "updated via tauri ipc"
            }
        }),
    )
    .expect("update_master_item");
    assert_eq!(updated_branch["note"], "updated via tauri ipc");

    // Deleting referenced branch should serialize AppError cleanly
    let delete_blocked = ipc_call(
        &webview,
        "delete_master_item",
        json!({ "kind": "overseas_branch", "id": first_branch_id }),
    )
    .expect_err("referenced branch delete must fail");
    assert!(delete_blocked.contains("삭제할 수 없습니다"));

    // 3. products commands (camelCase search/overseasBranchId/productId)
    let branch_list =
        ipc_call(&webview, "list_overseas_branches", json!({})).expect("list_overseas_branches");
    assert_eq!(branch_list.as_array().unwrap().len(), 3);

    let substances = ipc_call(
        &webview,
        "search_chemical_substances",
        json!({ "query": "67-64-1" }),
    )
    .expect("search_chemical_substances");
    assert!(!substances.as_array().unwrap().is_empty());

    let saved = ipc_call(
        &webview,
        "save_product_with_bom",
        json!({
            "input": {
                "id": null,
                "product_code": "IPC-TEST-01",
                "product_name": "IPC 테스트 제품",
                "overseas_branch_name": branch_arr[0]["name"],
                "components": [
                    { "cas_number": "50-00-0", "substance_name": "Formaldehyde", "weight_pct_text": "100" }
                ]
            }
        }),
    )
    .expect("save_product_with_bom");
    assert_eq!(saved["created"], true);
    let new_product_id = saved["product"]["id"].as_i64().unwrap();

    let listed = ipc_call(
        &webview,
        "list_products",
        json!({ "search": "IPC-TEST", "overseasBranchId": first_branch_id }),
    )
    .expect("list_products");
    assert_eq!(listed.as_array().unwrap().len(), 1);

    let detail = ipc_call(
        &webview,
        "get_product_detail",
        json!({ "productId": new_product_id }),
    )
    .expect("get_product_detail");
    assert_eq!(detail["product_code"], "IPC-TEST-01");

    let deleted = ipc_call(
        &webview,
        "delete_product",
        json!({ "productId": new_product_id }),
    )
    .expect("delete_product");
    assert_eq!(deleted["deleted_product_id"], new_product_id);
}

#[test]
fn shipments_excel_reports_and_backup_ipc_round_trip() {
    let host = MockHost::launch("ipc_reports_backup");
    let temp_dir = &host.temp_dir;
    let webview = host.main_window();

    ipc_call(&webview, "seed_sample_data", json!({})).expect("seed_sample_data");

    // 1. shipments commands
    let importers =
        ipc_call(&webview, "list_domestic_importers", json!({})).expect("list_domestic_importers");
    let importer_arr = importers.as_array().unwrap();
    assert_eq!(importer_arr.len(), 3);

    let prod_opts =
        ipc_call(&webview, "list_product_options", json!({})).expect("list_product_options");
    let first_prod = &prod_opts.as_array().unwrap()[0];
    let first_prod_id = first_prod["product_id"].as_i64().unwrap();
    let first_prod_code = first_prod["product_code"].as_str().unwrap();

    let saved_shipment = ipc_call(
        &webview,
        "save_shipment",
        json!({
            "input": {
                "id": null,
                "shipment_date": "2028-11-14",
                "domestic_importer_name": importer_arr[0]["name"],
                "product_id": first_prod_id,
                "import_weight_kg": "250.5",
                "note": "IPC shipment"
            }
        }),
    )
    .expect("save_shipment");
    let shipment_id = saved_shipment["id"].as_i64().unwrap();

    let queried = ipc_call(
        &webview,
        "query_shipments",
        json!({
            "filter": {
                "year": 2028,
                "domestic_importer_id": null,
                "overseas_branch_id": null,
                "search_text": first_prod_code
            }
        }),
    )
    .expect("query_shipments");
    assert_eq!(queried["summary"]["total_count"], 1);

    ipc_call(&webview, "delete_shipment", json!({ "shipmentId": shipment_id }))
        .expect("delete_shipment");

    // 2. excel_import commands
    let template_path = temp_dir.join("bom_template.xlsx");
    let tpl_res = ipc_call(
        &webview,
        "download_import_template",
        json!({
            "kind": "bom",
            "outputPath": template_path.to_string_lossy()
        }),
    )
    .expect("download_import_template");
    assert_eq!(tpl_res["kind"], "bom");
    assert!(template_path.exists());

    let csv_path = temp_dir.join("bom_import.csv");
    std::fs::write(
        &csv_path,
        "해외지사,수입제품코드,제품명,CAS번호,화학물질명,함유율(%)\n유럽지사 (독일),PRD-CSV-01,CSV 제품,67-64-1,아세톤,100\n",
    )
    .expect("write bom csv");

    let preview = ipc_call(
        &webview,
        "preview_excel_import",
        json!({
            "kind": "bom",
            "filePath": csv_path.to_string_lossy()
        }),
    )
    .expect("preview_excel_import");
    assert_eq!(preview["can_commit"], true);

    let commit = ipc_call(
        &webview,
        "commit_excel_import",
        json!({
            "kind": "bom",
            "filePath": csv_path.to_string_lossy(),
            "options": { "overwrite_existing": true, "skip_duplicates": true }
        }),
    )
    .expect("commit_excel_import");
    assert_eq!(commit["inserted_count"], 1);

    // 3. reports commands
    let filter_opts =
        ipc_call(&webview, "get_report_filter_options", json!({})).expect("get_report_filter_options");
    assert!(!filter_opts["years"].as_array().unwrap().is_empty());

    let report_filter = json!({
        "period_mode": "year",
        "target_year": filter_opts["years"][0],
        "start_date": null,
        "end_date": null,
        "domestic_importer_id": null,
        "overseas_branch_id": null,
        "product_id": null
    });
    let report = ipc_call(
        &webview,
        "query_annual_substance_totals",
        json!({ "filter": report_filter }),
    )
    .expect("query_annual_substance_totals");
    assert!(report["unique_substance_count"].as_u64().unwrap() > 0);

    let report_xlsx_path = temp_dir.join("annual_report.xlsx");
    let export_res = ipc_call(
        &webview,
        "export_annual_report_xlsx",
        json!({
            "filter": report_filter,
            "mode": "external_summary",
            "outputPath": report_xlsx_path.to_string_lossy()
        }),
    )
    .expect("export_annual_report_xlsx");
    assert_eq!(export_res["sheet_count"], 1);
    assert!(report_xlsx_path.exists());

    // 4. backup & restore commands (verifying AppState connection swap)
    let before_status = ipc_call(&webview, "get_db_status", json!({})).expect("status before backup");
    let original_shipments = before_status["import_shipments_count"].as_i64().unwrap();

    let backup_path = temp_dir.join("snapshot.db");
    let backup_res = ipc_call(
        &webview,
        "backup_database",
        json!({ "targetPath": backup_path.to_string_lossy() }),
    )
    .expect("backup_database");
    assert!(backup_res["file_size_bytes"].as_u64().unwrap() > 0);

    // Mutate after backup
    ipc_call(
        &webview,
        "save_shipment",
        json!({
            "input": {
                "id": null,
                "shipment_date": "2025-12-31",
                "domestic_importer_name": importer_arr[0]["name"],
                "product_id": first_prod_id,
                "import_weight_kg": "10",
                "note": "post-backup"
            }
        }),
    )
    .expect("mutate after backup");

    let mutated_status = ipc_call(&webview, "get_db_status", json!({})).expect("mutated status");
    assert_eq!(
        mutated_status["import_shipments_count"].as_i64().unwrap(),
        original_shipments + 1
    );

    let restore_res = ipc_call(
        &webview,
        "restore_database",
        json!({ "candidatePath": backup_path.to_string_lossy() }),
    )
    .expect("restore_database");
    assert_eq!(
        restore_res["status"]["import_shipments_count"].as_i64().unwrap(),
        original_shipments
    );

    // Subsequent IPC query uses the re-opened WAL connection cleanly
    let after_status = ipc_call(&webview, "get_db_status", json!({})).expect("status after restore");
    assert_eq!(
        after_status["import_shipments_count"].as_i64().unwrap(),
        original_shipments
    );
    assert!(after_status["journal_mode"]
        .as_str()
        .unwrap()
        .eq_ignore_ascii_case("wal"));
}
