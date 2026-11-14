import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import {
  canCommitImport,
  computeEffectiveCommitPlan,
  createExcelImportClient,
  createInitialImportOptions,
  createInitialImportState,
  defaultTemplateFilename,
  getConflictToggleCopy,
  parseCsvImportPreview,
  reduceImportState,
} from "../src/workspaces/excelImportLogic";

describe("Ticket 05 Self-Service Excel Import Logic & UI", () => {
  test("slice 5: template names, default conflict options, state transitions, and commit guard", async () => {
    expect(defaultTemplateFilename("bom")).toBe("제품구성성분_BOM_양식.xlsx");
    expect(defaultTemplateFilename("shipments")).toBe("수입실적_양식.xlsx");

    const defaultOpts = createInitialImportOptions();
    expect(defaultOpts).toEqual({
      overwrite_existing: true,
      skip_duplicates: true,
    });

    const bomCopy = getConflictToggleCopy("bom");
    expect(bomCopy.activeLabel).toBe("기존 수입제품 레시피 덮어쓰기 (기본값: 켜짐)");
    expect(bomCopy.inactiveLabel).toBe("기존 제품 건너뛰기");

    const shipCopy = getConflictToggleCopy("shipments");
    expect(shipCopy.activeLabel).toBe("중복 수입실적 건너뛰기 (기본값: 켜짐)");
    expect(shipCopy.inactiveLabel).toBe("중복 포함 강제 추가");

    // 1. State machine & commit button guard
    let state = createInitialImportState();
    expect(state.kind).toBe("bom");
    expect(canCommitImport(state)).toBe(false);

    // Invalid BOM CSV (99.9999% sum + duplicate CAS) disables commit
    const invalidBomCsv = [
      "해외지사,수입제품코드,제품명(선택),CAS번호,화학물질명(선택),함유율(%)",
      "유럽지사 (독일),PRD-BAD,실패제품,50-00-0,포름알데히드,50",
      "유럽지사 (독일),PRD-BAD,실패제품,7732-18-5,정제수,49.9999",
      "유럽지사 (독일),PRD-DUP,중복제품,67-64-1,아세톤,40",
      "유럽지사 (독일),PRD-DUP,중복제품,67-64-1,아세톤,60",
    ].join("\n");
    const badPreview = parseCsvImportPreview("bom", "bad_bom.csv", invalidBomCsv, {
      existingProductCodes: ["PRD-EU-101"],
      existingShipments: [],
    });
    expect(badPreview.can_commit).toBe(false);
    expect(badPreview.error_count).toBe(4);
    expect(badPreview.bom_products[0].errors.join(" ")).toContain("99.9999");
    expect(badPreview.bom_products[1].errors.join(" ")).toContain("중복");

    state = reduceImportState(state, {
      type: "PREVIEW_SUCCEEDED",
      filePath: "bad_bom.csv",
      preview: badPreview,
    });
    expect(canCommitImport(state)).toBe(false);

    // Valid BOM CSV with 1 new product + 1 existing conflict product enables commit
    const validBomCsv = [
      "해외지사,수입제품코드,제품명(선택),CAS번호,화학물질명(선택),함유율(%)",
      "유럽지사 (독일),PRD-EU-101,개정제품,50-00-0,포름알데히드,25",
      "유럽지사 (독일),PRD-EU-101,개정제품,7732-18-5,정제수,75",
      "일본지사 (도쿄),PRD-JP-900,신규제품,67-64-1,아세톤,100",
    ].join("\n");
    const okPreview = parseCsvImportPreview("bom", "ok_bom.csv", validBomCsv, {
      existingProductCodes: ["PRD-EU-101"],
      existingShipments: [],
    });
    expect(okPreview.can_commit).toBe(true);
    expect(okPreview.total_products).toBe(2);
    expect(okPreview.valid_count).toBe(1);
    expect(okPreview.conflict_count).toBe(1);
    expect(okPreview.error_count).toBe(0);

    state = reduceImportState(state, {
      type: "PREVIEW_SUCCEEDED",
      filePath: "ok_bom.csv",
      preview: okPreview,
    });
    expect(canCommitImport(state)).toBe(true);

    const overwritePlan = computeEffectiveCommitPlan(okPreview, state.options);
    expect(overwritePlan).toContain("신규 1");
    expect(overwritePlan).toContain("기존 1개 덮어쓰기");

    state = reduceImportState(state, {
      type: "SET_OVERWRITE_EXISTING",
      value: false,
    });
    const skipPlan = computeEffectiveCommitPlan(okPreview, state.options);
    expect(skipPlan).toContain("기존 1개 건너뛰기");

    // Switching import kind clears stale preview so wrong kind cannot be committed
    state = reduceImportState(state, { type: "SET_KIND", kind: "shipments" });
    expect(state.kind).toBe("shipments");
    expect(state.preview).toBeNull();
    expect(canCommitImport(state)).toBe(false);

    // Shipment CSV with unknown product code disables commit; valid + 5-tuple dup enables commit
    const badShipCsv = [
      "수입일자(YYYY-MM-DD),국내 수입사,해외지사,수입제품코드,제품 수입량(kg),비고(선택)",
      "2026-02-29,(주)대한케미칼,유럽지사 (독일),PRD-UNKNOWN,100,",
    ].join("\n");
    const badShipPreview = parseCsvImportPreview("shipments", "bad_ship.csv", badShipCsv, {
      existingProductCodes: ["PRD-EU-101"],
      productBranches: { "PRD-EU-101": "유럽지사 (독일)" },
      existingShipments: [],
    });
    expect(badShipPreview.can_commit).toBe(false);
    expect(badShipPreview.unknown_product_codes).toEqual(["PRD-UNKNOWN"]);

    const dupShipCsv = [
      "수입일자(YYYY-MM-DD),국내 수입사,해외지사,수입제품코드,제품 수입량(kg),비고(선택)",
      "2026-01-15,(주)대한케미칼,유럽지사 (독일),PRD-EU-101,2400,DB중복",
      "2026/3/15,(주)신규수입사,유럽지사 (독일),PRD-EU-101,550.25,신규",
      "2026-03-15,(주)신규수입사,유럽지사 (독일),PRD-EU-101,550.2500,배치중복",
    ].join("\n");
    const dupShipPreview = parseCsvImportPreview("shipments", "dup_ship.csv", dupShipCsv, {
      existingProductCodes: ["PRD-EU-101"],
      productBranches: { "PRD-EU-101": "유럽지사 (독일)" },
      existingShipments: [
        {
          shipment_date: "2026-01-15",
          domestic_importer_name: "(주)대한케미칼",
          overseas_branch_name: "유럽지사 (독일)",
          product_code: "PRD-EU-101",
          import_weight_kg_scaled: 24_000_000,
        },
      ],
    });
    expect(dupShipPreview.can_commit).toBe(true);
    expect(dupShipPreview.valid_count).toBe(1);
    expect(dupShipPreview.conflict_count).toBe(2);
    expect(dupShipPreview.rows[1].shipment_date).toBe("2026-03-15");

    // Tauri invoke delegation verification
    const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
    const tauriClient = createExcelImportClient(
      async <T>(cmd: string, args?: Record<string, unknown>) => {
        calls.push({ cmd, args });
        if (cmd === "plugin:dialog|save") return "C:\\tmp\\제품구성성분_BOM_양식.xlsx" as unknown as T;
        if (cmd === "plugin:dialog|open") return "C:\\tmp\\bom.xlsx" as unknown as T;
        return {} as unknown as T;
      },
    );
    const savePath = await tauriClient.pickTemplateSavePath("bom");
    expect(savePath).toBe("C:\\tmp\\제품구성성분_BOM_양식.xlsx");
    await tauriClient.downloadImportTemplate("bom", savePath!);
    const openPath = await tauriClient.pickImportFile();
    expect(openPath).toBe("C:\\tmp\\bom.xlsx");
    await tauriClient.previewExcelImport("bom", openPath!);
    await tauriClient.commitExcelImport("bom", openPath!, defaultOpts);

    expect(calls.map((c) => c.cmd)).toEqual([
      "plugin:dialog|save",
      "download_import_template",
      "plugin:dialog|open",
      "preview_excel_import",
      "commit_excel_import",
    ]);
    expect(calls[1].args).toEqual({
      kind: "bom",
      outputPath: "C:\\tmp\\제품구성성분_BOM_양식.xlsx",
    });
    expect(calls[4].args).toEqual({
      kind: "bom",
      filePath: "C:\\tmp\\bom.xlsx",
      options: defaultOpts,
    });
  });

  test("slice 6: standalone import flow & ExcelImportWorkspace.tsx contract", async () => {
    // 1. Standalone client BOM + Shipments round-trip
    const client = createExcelImportClient();
    const bomPath = client.registerInlineCsvFile!(
      "custom_bom.csv",
      "해외지사,수입제품코드,제품명(선택),CAS번호,화학물질명(선택),함유율(%)\n" +
        "일본지사 (도쿄),PRD-JP-NEW,신규품,50-00-0,포름알데히드,100\n",
    );
    const bomPreview = await client.previewExcelImport("bom", bomPath);
    expect(bomPreview.can_commit).toBe(true);
    const bomCommit = await client.commitExcelImport(
      "bom",
      bomPath,
      createInitialImportOptions(),
    );
    expect(bomCommit.inserted_count).toBe(1);

    // Shipment referencing newly committed PRD-JP-NEW succeeds
    const shipPath = client.registerInlineCsvFile!(
      "custom_ship.csv",
      "수입일자(YYYY-MM-DD),국내 수입사,해외지사,수입제품코드,제품 수입량(kg),비고(선택)\n" +
        "2026-08-10,(주)대한케미칼,일본지사 (도쿄),PRD-JP-NEW,320.5,신규실적\n",
    );
    const shipPreview = await client.previewExcelImport("shipments", shipPath);
    expect(shipPreview.can_commit).toBe(true);
    const shipCommit = await client.commitExcelImport(
      "shipments",
      shipPath,
      createInitialImportOptions(),
    );
    expect(shipCommit.inserted_count).toBe(1);

    // 2. Verify ExcelImportWorkspace.tsx compiles via Bun.Transpiler and exposes required controls
    const root = join(import.meta.dir, "..");
    const tsxCode = readFileSync(
      join(root, "src/workspaces/ExcelImportWorkspace.tsx"),
      "utf8",
    );
    const transpiler = new Bun.Transpiler({ loader: "tsx" });
    const compiled = transpiler.transformSync(tsxCode);
    expect(compiled.length).toBeGreaterThan(0);

    for (const requiredText of [
      "3. 엑셀 일괄 가져오기",
      "제품 구성성분(BOM) 일괄 가져오기",
      "수입실적 일괄 가져오기",
      "제품구성성분_BOM_양식.xlsx",
      "수입실적_양식.xlsx",
      "기존 수입제품 레시피 덮어쓰기 (기본값: 켜짐)",
      "기존 제품 건너뛰기",
      "중복 수입실적 건너뛰기 (기본값: 켜짐)",
      "중복 포함 강제 추가",
      "canCommitImport",
      "computeEffectiveCommitPlan",
      "--danger-soft",
    ]) {
      expect(tsxCode).toContain(requiredText);
    }
  });
});
