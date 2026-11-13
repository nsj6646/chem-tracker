import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import {
  autoFillSubstanceNameOnCasChange,
  createProductsIpcClient,
  evaluateBomGauge,
  filterProductList,
  findSubstanceSuggestions,
} from "../src/workspaces/productsLogic";

describe("Ticket 02 Frontend Products & BOM Logic", () => {
  test("slice 1: evaluateBomGauge computes exact 100.00% vs blocker banners without float drift", () => {
    // Valid 100.0000% (including 4-decimal precision that sums cleanly)
    const valid = evaluateBomGauge([
      { casNumber: "67-64-1", substanceName: "아세톤", weightPctText: "50.1234" },
      { casNumber: "108-88-3", substanceName: "톨루엔", weightPctText: "29.8766" },
      { casNumber: "141-78-6", substanceName: "", weightPctText: "20" },
    ]);
    expect(valid.isExact100).toBe(true);
    expect(valid.canSave).toBe(true);
    expect(valid.sumScaled).toBe(1_000_000);
    expect(valid.diffScaled).toBe(0);
    expect(valid.bannerText).toBe("합계: 100.00% — 저장 가능");

    // 99.9999% must NOT round to 100.00% and must block saving
    const under = evaluateBomGauge([
      { casNumber: "67-64-1", substanceName: "", weightPctText: "50" },
      { casNumber: "108-88-3", substanceName: "", weightPctText: "49.9999" },
    ]);
    expect(under.isExact100).toBe(false);
    expect(under.canSave).toBe(false);
    expect(under.sumScaled).toBe(999_999);
    expect(under.diffScaled).toBe(-1);
    expect(under.bannerText).toBe(
      "합계: 99.9999% — 합계가 정확히 100%여야 저장할 수 있습니다 (차이: -0.0001%)",
    );

    // 100.0001% must block saving with +0.0001%
    const over = evaluateBomGauge([
      { casNumber: "67-64-1", substanceName: "", weightPctText: "50" },
      { casNumber: "108-88-3", substanceName: "", weightPctText: "50.0001" },
    ]);
    expect(over.isExact100).toBe(false);
    expect(over.canSave).toBe(false);
    expect(over.bannerText).toBe(
      "합계: 100.0001% — 합계가 정확히 100%여야 저장할 수 있습니다 (차이: +0.0001%)",
    );

    // 98.50% example from spec/prototype
    const short = evaluateBomGauge([
      { casNumber: "67-64-1", substanceName: "", weightPctText: "45.25" },
      { casNumber: "108-88-3", substanceName: "", weightPctText: "34.75" },
      { casNumber: "141-78-6", substanceName: "", weightPctText: "18.5" },
    ]);
    expect(short.canSave).toBe(false);
    expect(short.bannerText).toBe(
      "합계: 98.50% — 합계가 정확히 100%여야 저장할 수 있습니다 (차이: -1.50%)",
    );

    // Duplicate CAS summing to 100% still blocks saving
    const dupCas = evaluateBomGauge([
      { casNumber: "67-64-1", substanceName: "", weightPctText: "50" },
      { casNumber: " 67-64-1 ", substanceName: "", weightPctText: "50" },
    ]);
    expect(dupCas.isExact100).toBe(true);
    expect(dupCas.canSave).toBe(false);
    expect(dupCas.duplicateCasNumbers).toEqual(["67-64-1"]);

    // Empty rows block saving
    const empty = evaluateBomGauge([]);
    expect(empty.canSave).toBe(false);
    expect(empty.sumScaled).toBe(0);
  });

  test("slice 2: product filtering, CAS autocomplete/auto-fill, and IPC client CRUD + delete guard", async () => {
    const ipc = createProductsIpcClient();

    const initialProducts = await ipc.listProducts();
    expect(initialProducts.length).toBe(3);
    expect(initialProducts.every((p) => p.is_valid_100_pct)).toBe(true);

    // Filter by search text across code, name, and overseas branch
    expect(filterProductList(initialProducts, "PRD-US", null).length).toBe(1);
    expect(filterProductList(initialProducts, "세정제", null).length).toBe(1);
    expect(filterProductList(initialProducts, "유럽지사", null).length).toBe(2);

    const branches = await ipc.listOverseasBranches();
    expect(branches.length).toBe(3);
    const euBranch = branches.find((b) => b.name.includes("유럽지사"))!;
    expect(filterProductList(initialProducts, "", euBranch.id).length).toBe(2);

    // Chemical substance autocomplete & auto-fill
    const substances = await ipc.searchChemicalSubstances("");
    expect(substances.length).toBe(7);
    expect(findSubstanceSuggestions(substances, "아세톤").length).toBe(1);
    expect(findSubstanceSuggestions(substances, "67-64").length).toBe(1);

    const autoFilled = autoFillSubstanceNameOnCasChange(
      { casNumber: "", substanceName: "", weightPctText: "25" },
      " 67-64-1 ",
      substances,
    );
    expect(autoFilled.casNumber).toBe(" 67-64-1 ");
    expect(autoFilled.substanceName).toBe("아세톤 (Acetone)");

    // Creating a new product with inline new branch and new substance works
    const created = await ipc.saveProductWithBom({
      id: null,
      product_code: "PRD-JP-501",
      product_name: "일본 신규 제품",
      overseas_branch_name: "일본신규지사 (오사카)",
      components: [
        {
          cas_number: "50-00-0",
          substance_name: "포름알데히드",
          weight_pct_text: "100",
        },
      ],
    });
    expect(created.created).toBe(true);
    expect(created.product.product_code).toBe("PRD-JP-501");

    // Newly created branch and substance appear in lookups
    const branchesAfter = await ipc.listOverseasBranches();
    expect(branchesAfter.some((b) => b.name === "일본신규지사 (오사카)")).toBe(true);
    const subsAfter = await ipc.searchChemicalSubstances("50-00-0");
    expect(subsAfter.length).toBe(1);

    // Deleting a seeded product referenced by shipments fails with exact Korean message
    const prdEu101 = initialProducts.find((p) => p.product_code === "PRD-EU-101")!;
    await expect(ipc.deleteProduct(prdEu101.id)).rejects.toThrow(
      "해당 수입제품으로 등록된 수입실적이",
    );

    // Deleting an unreferenced product succeeds
    const del = await ipc.deleteProduct(created.product.id);
    expect(del.deleted_product_code).toBe("PRD-JP-501");
    expect((await ipc.listProducts()).length).toBe(3);

    // Tauri invoke override delegates with expected command names and snake_case args
    const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
    const tauriIpc = createProductsIpcClient(async <T>(cmd: string, args?: Record<string, unknown>) => {
      calls.push({ cmd, args });
      return [] as unknown as T;
    });
    await tauriIpc.listProducts("PRD", 2);
    expect(calls[0]).toEqual({
      cmd: "list_products",
      args: { search: "PRD", overseasBranchId: 2 },
    });
  });

  test("slice 3: ProductsWorkspace.tsx compiles and exposes required controls & trigger hooks", () => {
    const root = join(import.meta.dir, "..");
    const tsx = readFileSync(
      join(root, "src/workspaces/ProductsWorkspace.tsx"),
      "utf8",
    );

    const transpiler = new Bun.Transpiler({ loader: "tsx" });
    const output = transpiler.transformSync(tsx);
    expect(output.length).toBeGreaterThan(0);

    // Left pane elements
    expect(tsx).toContain("+ 신규 수입제품 등록");
    expect(tsx).toContain("수입제품코드");
    expect(tsx).toContain("제품명(선택)");
    expect(tsx).toContain("해외지사");
    expect(tsx).toContain("구성 물질 수");
    expect(tsx).toContain("100.00% 정상");

    // Right pane elements & master modal trigger hooks
    expect(tsx).toContain("+ 새 해외지사 추가");
    expect(tsx).toContain("[관리/수정]");
    expect(tsx).toContain("[화학물질 관리/수정]");
    expect(tsx).toContain("CAS번호");
    expect(tsx).toContain("화학물질명");
    expect(tsx).toContain("함유율 (%)");
    expect(tsx).toContain("evaluateBomGauge");
    expect(tsx).toContain("autoFillSubstanceNameOnCasChange");
  });
});
