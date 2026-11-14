use chem_tracker::db::open_connection;
use chem_tracker::decimal::HUNDRED_PERCENT_SCALED;
use chem_tracker::products::{
    delete_product, get_product_detail, list_overseas_branches, list_products,
    save_product_with_bom, search_chemical_substances, SaveBomComponentInput, SaveProductInput,
};
use std::path::PathBuf;

fn temp_db_path(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "chem_tracker_t02_{name}_{}_{}.db",
        std::process::id(),
        nanos
    ))
}

struct TempDb {
    path: PathBuf,
}

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(self.path.with_extension("db-wal"));
        let _ = std::fs::remove_file(self.path.with_extension("db-shm"));
    }
}

#[test]
fn slice1_save_new_product_with_bom_list_detail_and_lookups() {
    let db_path = temp_db_path("slice1");
    let _guard = TempDb {
        path: db_path.clone(),
    };
    let mut conn = open_connection(&db_path).expect("open_connection");

    let saved = save_product_with_bom(
        &mut conn,
        &SaveProductInput {
            id: None,
            product_code: " PRD-EU-901 ".to_string(),
            product_name: Some(" 신제품 코팅액 ".to_string()),
            overseas_branch_name: " 유럽지사 (독일) ".to_string(),
            components: vec![
                SaveBomComponentInput {
                    cas_number: " 67-64-1 ".to_string(),
                    substance_name: Some(" 아세톤 (Acetone) ".to_string()),
                    weight_pct_text: "45.2500".to_string(),
                },
                SaveBomComponentInput {
                    cas_number: "108-88-3".to_string(),
                    substance_name: None,
                    weight_pct_text: "54.75".to_string(),
                },
            ],
        },
    )
    .expect("valid 100% BOM should save");

    assert!(saved.created);
    assert_eq!(saved.product.product_code, "PRD-EU-901");
    assert_eq!(
        saved.product.product_name.as_deref(),
        Some("신제품 코팅액")
    );
    assert_eq!(saved.product.overseas_branch_name, "유럽지사 (독일)");
    assert_eq!(saved.product.weight_pct_sum_scaled, HUNDRED_PERCENT_SCALED);
    assert!(saved.product.is_valid_100_pct);
    assert_eq!(saved.product.components.len(), 2);
    assert_eq!(saved.product.components[0].cas_number, "67-64-1");
    assert_eq!(
        saved.product.components[0].substance_name.as_deref(),
        Some("아세톤 (Acetone)")
    );
    assert_eq!(saved.product.components[0].weight_pct_text, "45.25");
    assert_eq!(saved.product.components[0].weight_pct_scaled, 452_500);
    assert_eq!(saved.product.components[1].cas_number, "108-88-3");
    assert_eq!(saved.product.components[1].substance_name, None);
    assert_eq!(saved.product.components[1].weight_pct_scaled, 547_500);

    // list_products returns the saved item
    let list = list_products(&conn, None, None).expect("list_products");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, saved.product.id);
    assert_eq!(list[0].product_code, "PRD-EU-901");
    assert_eq!(list[0].component_count, 2);
    assert_eq!(list[0].weight_pct_sum_scaled, HUNDRED_PERCENT_SCALED);
    assert!(list[0].is_valid_100_pct);

    // get_product_detail returns identical detail
    let detail = get_product_detail(&conn, saved.product.id).expect("get_product_detail");
    assert_eq!(detail, saved.product);

    // list_overseas_branches includes created branch
    let branches = list_overseas_branches(&conn).expect("list_overseas_branches");
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].name, "유럽지사 (독일)");

    // search_chemical_substances finds created substances by CAS or Korean name
    let all_subs = search_chemical_substances(&conn, None).expect("all substances");
    assert_eq!(all_subs.len(), 2);
    let by_cas = search_chemical_substances(&conn, Some("67-64")).expect("by cas");
    assert_eq!(by_cas.len(), 1);
    assert_eq!(by_cas[0].cas_number, "67-64-1");
    let by_name = search_chemical_substances(&conn, Some("아세톤")).expect("by name");
    assert_eq!(by_name.len(), 1);
    assert_eq!(by_name[0].cas_number, "67-64-1");
}

#[test]
fn slice2_bom_validation_rejects_invalid_inputs_and_99_9999_or_100_0001_atomically() {
    let db_path = temp_db_path("slice2");
    let _guard = TempDb {
        path: db_path.clone(),
    };
    let mut conn = open_connection(&db_path).expect("open_connection");

    // Helper for baseline valid components
    let valid_comps = vec![SaveBomComponentInput {
        cas_number: "67-64-1".to_string(),
        substance_name: Some("아세톤".to_string()),
        weight_pct_text: "100".to_string(),
    }];

    // 1. Empty product_code
    let err = save_product_with_bom(
        &mut conn,
        &SaveProductInput {
            id: None,
            product_code: "   ".to_string(),
            product_name: None,
            overseas_branch_name: "유럽지사".to_string(),
            components: valid_comps.clone(),
        },
    )
    .expect_err("empty product_code must fail");
    assert!(err.to_string().contains("수입제품코드"));

    // 2. Empty overseas_branch_name
    let err = save_product_with_bom(
        &mut conn,
        &SaveProductInput {
            id: None,
            product_code: "PRD-1".to_string(),
            product_name: None,
            overseas_branch_name: "  ".to_string(),
            components: valid_comps.clone(),
        },
    )
    .expect_err("empty overseas_branch_name must fail");
    assert!(err.to_string().contains("해외지사"));

    // 3. Zero components
    let err = save_product_with_bom(
        &mut conn,
        &SaveProductInput {
            id: None,
            product_code: "PRD-1".to_string(),
            product_name: None,
            overseas_branch_name: "유럽지사".to_string(),
            components: vec![],
        },
    )
    .expect_err("zero components must fail");
    assert!(err.to_string().contains("최소 1개"));

    // 4. Component weight <= 0 or > 100 or > 4 decimals
    for bad_pct in ["0", "-5", "100.0001", "50.12345"] {
        let err = save_product_with_bom(
            &mut conn,
            &SaveProductInput {
                id: None,
                product_code: "PRD-1".to_string(),
                product_name: None,
                overseas_branch_name: "유럽지사".to_string(),
                components: vec![SaveBomComponentInput {
                    cas_number: "67-64-1".to_string(),
                    substance_name: None,
                    weight_pct_text: bad_pct.to_string(),
                }],
            },
        )
        .expect_err(&format!("bad weight {bad_pct} must fail"));
        assert!(!err.to_string().is_empty());
    }

    // 5. Duplicate CAS번호 within same product
    let err = save_product_with_bom(
        &mut conn,
        &SaveProductInput {
            id: None,
            product_code: "PRD-1".to_string(),
            product_name: None,
            overseas_branch_name: "유럽지사".to_string(),
            components: vec![
                SaveBomComponentInput {
                    cas_number: "67-64-1".to_string(),
                    substance_name: None,
                    weight_pct_text: "50".to_string(),
                },
                SaveBomComponentInput {
                    cas_number: " 67-64-1 ".to_string(),
                    substance_name: None,
                    weight_pct_text: "50".to_string(),
                },
            ],
        },
    )
    .expect_err("duplicate CAS must fail");
    assert!(err.to_string().contains("중복된 CAS번호"));

    // 6. Sum = 99.9999% must fail with clear message showing 99.9999%
    let err_low = save_product_with_bom(
        &mut conn,
        &SaveProductInput {
            id: None,
            product_code: "PRD-1".to_string(),
            product_name: None,
            overseas_branch_name: "유럽지사".to_string(),
            components: vec![
                SaveBomComponentInput {
                    cas_number: "67-64-1".to_string(),
                    substance_name: None,
                    weight_pct_text: "50".to_string(),
                },
                SaveBomComponentInput {
                    cas_number: "108-88-3".to_string(),
                    substance_name: None,
                    weight_pct_text: "49.9999".to_string(),
                },
            ],
        },
    )
    .expect_err("99.9999% must fail");
    let msg_low = err_low.to_string();
    assert!(msg_low.contains("100%"), "unexpected msg: {msg_low}");
    assert!(msg_low.contains("99.9999"), "unexpected msg: {msg_low}");

    // 7. Sum = 100.0001% must fail with clear message showing 100.0001%
    let err_high = save_product_with_bom(
        &mut conn,
        &SaveProductInput {
            id: None,
            product_code: "PRD-1".to_string(),
            product_name: None,
            overseas_branch_name: "유럽지사".to_string(),
            components: vec![
                SaveBomComponentInput {
                    cas_number: "67-64-1".to_string(),
                    substance_name: None,
                    weight_pct_text: "50".to_string(),
                },
                SaveBomComponentInput {
                    cas_number: "108-88-3".to_string(),
                    substance_name: None,
                    weight_pct_text: "50.0001".to_string(),
                },
            ],
        },
    )
    .expect_err("100.0001% must fail");
    let msg_high = err_high.to_string();
    assert!(msg_high.contains("100%"), "unexpected msg: {msg_high}");
    assert!(msg_high.contains("100.0001"), "unexpected msg: {msg_high}");

    // Verify zero partial inserts (branches, substances, products all remain 0)
    assert!(list_products(&conn, None, None).unwrap().is_empty());
    assert!(list_overseas_branches(&conn).unwrap().is_empty());
    assert!(search_chemical_substances(&conn, None).unwrap().is_empty());
}

#[test]
fn slice3_update_upsert_substance_names_and_product_filtering() {
    let db_path = temp_db_path("slice3");
    let _guard = TempDb {
        path: db_path.clone(),
    };
    let mut conn = open_connection(&db_path).expect("open_connection");

    // Seed two products in different branches
    let p1 = save_product_with_bom(
        &mut conn,
        &SaveProductInput {
            id: None,
            product_code: "PRD-EU-101".to_string(),
            product_name: Some("코팅솔벤트 A-100".to_string()),
            overseas_branch_name: "Europe Branch".to_string(),
            components: vec![
                SaveBomComponentInput {
                    cas_number: "67-64-1".to_string(),
                    substance_name: Some("아세톤".to_string()),
                    weight_pct_text: "60".to_string(),
                },
                SaveBomComponentInput {
                    cas_number: "108-88-3".to_string(),
                    substance_name: None,
                    weight_pct_text: "40".to_string(),
                },
            ],
        },
    )
    .expect("p1");

    let p2 = save_product_with_bom(
        &mut conn,
        &SaveProductInput {
            id: None,
            product_code: "PRD-US-305".to_string(),
            product_name: Some("표면처리제 C-300".to_string()),
            overseas_branch_name: "북미지사 (미국)".to_string(),
            components: vec![SaveBomComponentInput {
                cas_number: "1330-20-7".to_string(),
                substance_name: Some("크실렌".to_string()),
                weight_pct_text: "100".to_string(),
            }],
        },
    )
    .expect("p2");

    // Update p1 by id: match branch case-insensitively ("europe branch"),
    // omit substance_name for 67-64-1 (must preserve "아세톤"),
    // provide substance_name for 108-88-3 (must set "톨루엔"),
    // and replace component set with 3 items summing to 100.0000%.
    let updated = save_product_with_bom(
        &mut conn,
        &SaveProductInput {
            id: Some(p1.product.id),
            product_code: "PRD-EU-101-R2".to_string(),
            product_name: Some("코팅솔벤트 A-100 개정".to_string()),
            overseas_branch_name: "europe branch".to_string(),
            components: vec![
                SaveBomComponentInput {
                    cas_number: "67-64-1".to_string(),
                    substance_name: None,
                    weight_pct_text: "50.1234".to_string(),
                },
                SaveBomComponentInput {
                    cas_number: "108-88-3".to_string(),
                    substance_name: Some("톨루엔".to_string()),
                    weight_pct_text: "29.8766".to_string(),
                },
                SaveBomComponentInput {
                    cas_number: "141-78-6".to_string(),
                    substance_name: Some("초산에틸".to_string()),
                    weight_pct_text: "20".to_string(),
                },
            ],
        },
    )
    .expect("update p1");

    assert!(!updated.created);
    assert_eq!(updated.product.id, p1.product.id);
    assert_eq!(updated.product.overseas_branch_id, p1.product.overseas_branch_id);
    assert_eq!(updated.product.components.len(), 3);
    assert_eq!(
        updated.product.components[0].substance_name.as_deref(),
        Some("아세톤")
    );
    assert_eq!(
        updated.product.components[1].substance_name.as_deref(),
        Some("톨루엔")
    );
    assert_eq!(list_overseas_branches(&conn).unwrap().len(), 2);

    // Creating with id: None must reject duplicate product_code (case-insensitive) instead of overwriting
    let dup_create = save_product_with_bom(
        &mut conn,
        &SaveProductInput {
            id: None,
            product_code: "prd-us-305".to_string(),
            product_name: None,
            overseas_branch_name: "북미지사 (미국)".to_string(),
            components: vec![SaveBomComponentInput {
                cas_number: "1330-20-7".to_string(),
                substance_name: None,
                weight_pct_text: "100".to_string(),
            }],
        },
    )
    .expect_err("duplicate product_code on create must fail");
    assert!(dup_create.to_string().contains("이미 사용 중인 수입제품코드입니다"));

    // Updating p1 to collide with p2's code must fail
    let conflict = save_product_with_bom(
        &mut conn,
        &SaveProductInput {
            id: Some(p1.product.id),
            product_code: "prd-us-305".to_string(),
            product_name: None,
            overseas_branch_name: "Europe Branch".to_string(),
            components: vec![SaveBomComponentInput {
                cas_number: "67-64-1".to_string(),
                substance_name: None,
                weight_pct_text: "100".to_string(),
            }],
        },
    );
    assert!(conflict.is_err());

    // Filtering tests: by code, by product_name, by branch name, by branch id
    assert_eq!(list_products(&conn, Some("R2"), None).unwrap().len(), 1);
    assert_eq!(list_products(&conn, Some("개정"), None).unwrap().len(), 1);
    assert_eq!(list_products(&conn, Some("북미지사"), None).unwrap().len(), 1);
    assert_eq!(
        list_products(&conn, None, Some(p1.product.overseas_branch_id))
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        list_products(&conn, Some("PRD"), Some(p2.product.overseas_branch_id))
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn slice4_delete_product_blocks_when_shipments_exist_and_cascades_when_unreferenced() {
    let db_path = temp_db_path("slice4");
    let _guard = TempDb {
        path: db_path.clone(),
    };
    let mut conn = open_connection(&db_path).expect("open_connection");

    let p_referenced = save_product_with_bom(
        &mut conn,
        &SaveProductInput {
            id: None,
            product_code: "PRD-REF-1".to_string(),
            product_name: Some("실적 연결 제품".to_string()),
            overseas_branch_name: "유럽지사".to_string(),
            components: vec![
                SaveBomComponentInput {
                    cas_number: "67-64-1".to_string(),
                    substance_name: Some("아세톤".to_string()),
                    weight_pct_text: "70".to_string(),
                },
                SaveBomComponentInput {
                    cas_number: "108-88-3".to_string(),
                    substance_name: Some("톨루엔".to_string()),
                    weight_pct_text: "30".to_string(),
                },
            ],
        },
    )
    .expect("p_referenced");

    let p_free = save_product_with_bom(
        &mut conn,
        &SaveProductInput {
            id: None,
            product_code: "PRD-FREE-2".to_string(),
            product_name: Some("미사용 제품".to_string()),
            overseas_branch_name: "유럽지사".to_string(),
            components: vec![SaveBomComponentInput {
                cas_number: "7732-18-5".to_string(),
                substance_name: Some("정제수".to_string()),
                weight_pct_text: "100".to_string(),
            }],
        },
    )
    .expect("p_free");

    // Insert 2 import_shipments referencing p_referenced
    conn.execute(
        "INSERT INTO domestic_importers (name) VALUES ('(주)대한케미칼');",
        [],
    )
    .unwrap();
    let importer_id: i64 = conn
        .query_row("SELECT id FROM domestic_importers LIMIT 1;", [], |r| r.get(0))
        .unwrap();
    for date in ["2026-01-10", "2026-02-15"] {
        conn.execute(
            "INSERT INTO import_shipments (shipment_date, domestic_importer_id, overseas_branch_id, product_id, import_weight_kg_text, import_weight_kg_scaled)
             VALUES (?1, ?2, ?3, ?4, '1000', 10000000);",
            rusqlite::params![
                date,
                importer_id,
                p_referenced.product.overseas_branch_id,
                p_referenced.product.id
            ],
        )
        .unwrap();
    }

    // Attempting to delete p_referenced must be blocked with exact Korean message
    let err = delete_product(&mut conn, p_referenced.product.id)
        .expect_err("referenced product must not be deleted");
    assert_eq!(
        err.to_string(),
        "입력값 검증 오류: 해당 수입제품으로 등록된 수입실적이 2건 존재하여 삭제할 수 없습니다. 수입실적을 먼저 삭제하거나 수정해 주세요."
    );
    // Product and its components still exist
    assert!(get_product_detail(&conn, p_referenced.product.id).is_ok());

    // Deleting p_free (0 shipments) succeeds and cascades to its product_components
    let del_res = delete_product(&mut conn, p_free.product.id).expect("unreferenced product deletes");
    assert_eq!(del_res.deleted_product_id, p_free.product.id);
    assert_eq!(del_res.deleted_product_code, "PRD-FREE-2");
    assert!(get_product_detail(&conn, p_free.product.id).is_err());

    let free_comp_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM product_components WHERE product_id = ?1;",
            [p_free.product.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(free_comp_count, 0);
}
