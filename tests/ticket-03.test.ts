import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import {
  computeLiveWeightPreview,
  computeVirtualWindow,
  createInitialShipmentForm,
  createShipmentsClient,
  filterShipments,
  loadShipmentIntoForm,
  resolveSelectedProduct,
  summarizeShipments,
  validateShipmentForm,
  type ProductOption,
  type ShipmentRecord,
} from "../src/workspaces/shipmentsLogic";

const SAMPLE_PRODUCTS: ProductOption[] = [
  {
    product_id: 1,
    product_code: "PRD-EU-101",
    product_name: "코팅솔벤트 A-100",
    overseas_branch_id: 1,
    overseas_branch_name: "유럽지사 (독일)",
  },
  {
    product_id: 3,
    product_code: "PRD-US-305",
    product_name: "표면처리제 C-300",
    overseas_branch_id: 2,
    overseas_branch_name: "북미지사 (미국)",
  },
];

describe("Ticket 03 Shipments Logic & Virtual Grid", () => {
  test("slice4: form state, product->branch auto-lock, live ton preview, and validation", () => {
    const form = createInitialShipmentForm("2026-10-05");
    expect(form.shipmentDate).toBe("2026-10-05");
    expect(form.editingId).toBeNull();

    // Selecting product automatically resolves and locks its overseas branch
    const selected = resolveSelectedProduct(SAMPLE_PRODUCTS, 3);
    expect(selected?.overseas_branch_name).toBe("북미지사 (미국)");
    expect(resolveSelectedProduct(SAMPLE_PRODUCTS, 999)).toBeNull();

    // Live fixed-point kg -> ton preview
    const preview1 = computeLiveWeightPreview("1,250.5");
    expect(preview1).toEqual({
      valid: true,
      kgText: "1,250.50",
      tonText: "1.2505",
      error: null,
    });

    // Small non-zero weight (0.04 kg = 0.00004 ton -> 5 decimals)
    const previewSmall = computeLiveWeightPreview("0.04");
    expect(previewSmall).toEqual({
      valid: true,
      kgText: "0.04",
      tonText: "0.00004",
      error: null,
    });

    // Rejects <= 0 or > 4 decimals
    expect(computeLiveWeightPreview("0").valid).toBe(false);
    expect(computeLiveWeightPreview("-5").valid).toBe(false);
    expect(computeLiveWeightPreview("1.23456").valid).toBe(false);

    // Form validation blocks bad calendar date, missing importer, missing product
    const badDate = validateShipmentForm(
      {
        ...form,
        shipmentDate: "2026-02-29",
        domesticImporterName: "(주)대한케미칼",
        productId: 1,
        importWeightKgInput: "100",
      },
      SAMPLE_PRODUCTS,
    );
    expect(badDate.ok).toBe(false);

    const valid = validateShipmentForm(
      {
        ...form,
        shipmentDate: "2024-02-29",
        domesticImporterName: "  (주)신규수입사  ",
        productId: 1,
        importWeightKgInput: "2,400.1250",
        note: "  정기 입고  ",
      },
      SAMPLE_PRODUCTS,
    );
    expect(valid).toEqual({
      ok: true,
      payload: {
        id: null,
        shipment_date: "2024-02-29",
        domestic_importer_name: "(주)신규수입사",
        product_id: 1,
        import_weight_kg: "2400.125",
        note: "정기 입고",
      },
    });

    // Loading an existing row into form for inline edit
    const existingRow: ShipmentRecord = {
      id: 42,
      shipment_date: "2026-08-04",
      shipment_year: 2026,
      domestic_importer_id: 1,
      domestic_importer_name: "(주)대한케미칼",
      overseas_branch_id: 1,
      overseas_branch_name: "유럽지사 (독일)",
      product_id: 1,
      product_code: "PRD-EU-101",
      product_name: "코팅솔벤트 A-100",
      import_weight_kg_text: "1100.25",
      import_weight_kg_scaled: 11_002_500,
      import_weight_kg_display: "1,100.25",
      import_weight_ton_display: "1.1003",
      note: "비고 테스트",
    };
    const editForm = loadShipmentIntoForm(existingRow);
    expect(editForm.editingId).toBe(42);
    expect(editForm.domesticImporterName).toBe("(주)대한케미칼");
    expect(editForm.importWeightKgInput).toBe("1100.25");
    expect(editForm.note).toBe("비고 테스트");
  });

  test("slice5: filtering, exact fixed-point summary, and virtual windowing for 10,000+ rows", () => {
    const rows: ShipmentRecord[] = [
      {
        id: 1,
        shipment_date: "2026-08-04",
        shipment_year: 2026,
        domestic_importer_id: 1,
        domestic_importer_name: "(주)대한케미칼",
        overseas_branch_id: 1,
        overseas_branch_name: "유럽지사 (독일)",
        product_id: 1,
        product_code: "PRD-EU-101",
        product_name: "코팅솔벤트 A-100",
        import_weight_kg_text: "1100.25",
        import_weight_kg_scaled: 11_002_500,
        import_weight_kg_display: "1,100.25",
        import_weight_ton_display: "1.1003",
        note: null,
      },
      {
        id: 2,
        shipment_date: "2026-05-22",
        shipment_year: 2026,
        domestic_importer_id: 1,
        domestic_importer_name: "(주)대한케미칼",
        overseas_branch_id: 2,
        overseas_branch_name: "북미지사 (미국)",
        product_id: 3,
        product_code: "PRD-US-305",
        product_name: "표면처리제 C-300",
        import_weight_kg_text: "3200",
        import_weight_kg_scaled: 32_000_000,
        import_weight_kg_display: "3,200.00",
        import_weight_ton_display: "3.2000",
        note: "상반기 추가 물량",
      },
      {
        id: 3,
        shipment_date: "2025-11-12",
        shipment_year: 2025,
        domestic_importer_id: 2,
        domestic_importer_name: "한성정밀화학(주)",
        overseas_branch_id: 1,
        overseas_branch_name: "유럽지사 (독일)",
        product_id: 1,
        product_code: "PRD-EU-101",
        product_name: "코팅솔벤트 A-100",
        import_weight_kg_text: "1800",
        import_weight_kg_scaled: 18_000_000,
        import_weight_kg_display: "1,800.00",
        import_weight_ton_display: "1.8000",
        note: null,
      },
    ];

    const filtered = filterShipments(rows, {
      year: 2026,
      domestic_importer_id: 1,
      overseas_branch_id: null,
      search_text: "표면처리제",
    });
    expect(filtered.map((r) => r.id)).toEqual([2]);

    const summary = summarizeShipments(rows);
    expect(summary.total_count).toBe(3);
    expect(summary.total_weight_kg_text).toBe("6,100.25");
    expect(summary.total_weight_ton_text).toBe("6.1003");

    // Empty summary
    expect(summarizeShipments([])).toEqual({
      total_count: 0,
      total_weight_kg_text: "0.00",
      total_weight_ton_text: "0.0000",
    });

    // Virtual window for 10,000 rows: renders ~30-50 DOM rows at a time
    const winTop = computeVirtualWindow({
      count: 10_000,
      estimateSize: 40,
      overscan: 8,
      scrollOffset: 0,
      viewportHeight: 520, // 13 visible rows + 8 overscan = 21..30
    });
    expect(winTop.totalSize).toBe(400_000);
    expect(winTop.startIndex).toBe(0);
    expect(winTop.virtualItems.length).toBeGreaterThanOrEqual(20);
    expect(winTop.virtualItems.length).toBeLessThanOrEqual(50);

    const winMid = computeVirtualWindow({
      count: 10_000,
      estimateSize: 40,
      overscan: 10,
      scrollOffset: 200_000, // row 5000
      viewportHeight: 560, // 14 visible + 10 top + 10 bottom = ~34 rows
    });
    expect(winMid.totalSize).toBe(400_000);
    expect(winMid.startIndex).toBe(4990);
    expect(winMid.virtualItems.length).toBeGreaterThanOrEqual(30);
    expect(winMid.virtualItems.length).toBeLessThanOrEqual(50);
    expect(winMid.virtualItems[0]).toEqual({
      index: 4990,
      key: 4990,
      start: 4990 * 40,
      size: 40,
      end: 4991 * 40,
    });

    const winEmpty = computeVirtualWindow({
      count: 0,
      estimateSize: 40,
      overscan: 8,
      scrollOffset: 0,
      viewportHeight: 520,
    });
    expect(winEmpty).toEqual({
      totalSize: 0,
      startIndex: 0,
      endIndex: -1,
      virtualItems: [],
    });
  });

  test("slice6: shipments client CRUD fallback and ShipmentsWorkspace TSX contract", async () => {
    const client = createShipmentsClient();
    const initialImporters = await client.listDomesticImporters();
    const initialProducts = await client.listProductOptions();
    expect(initialImporters.length).toBe(3);
    expect(initialProducts.length).toBe(3);

    const before = await client.queryShipments({
      year: null,
      domestic_importer_id: null,
      overseas_branch_id: null,
      search_text: "",
    });
    expect(before.summary.total_count).toBe(6);
    expect(before.summary.total_weight_kg_text).toBe("14,000.75");
    expect(before.summary.total_weight_ton_text).toBe("14.0008");

    // Create with inline importer creation & product->branch lock
    const created = await client.saveShipment({
      id: null,
      shipment_date: "2026-10-05",
      domestic_importer_name: "(주)신규고객사",
      product_id: 3, // PRD-US-305 -> 북미지사 (미국)
      import_weight_kg: "500.25",
      note: "긴급 물량",
    });
    expect(created.overseas_branch_name).toBe("북미지사 (미국)");
    expect(created.import_weight_kg_display).toBe("500.25");
    expect(created.import_weight_ton_display).toBe("0.5003");
    expect((await client.listDomesticImporters()).length).toBe(4);

    // Update
    const updated = await client.saveShipment({
      id: created.id,
      shipment_date: "2026-10-06",
      domestic_importer_name: "(주)신규고객사",
      product_id: 1, // PRD-EU-101 -> 유럽지사 (독일)
      import_weight_kg: "1000",
      note: null,
    });
    expect(updated.overseas_branch_name).toBe("유럽지사 (독일)");
    expect(updated.import_weight_kg_display).toBe("1,000.00");

    // Delete
    await client.deleteShipment(created.id);
    const after = await client.queryShipments({
      year: null,
      domestic_importer_id: null,
      overseas_branch_id: null,
      search_text: "",
    });
    expect(after.summary.total_count).toBe(6);

    // Verify ShipmentsWorkspace.tsx compiles via Bun.Transpiler and contains required UI hooks
    const root = join(import.meta.dir, "..");
    const tsxCode = readFileSync(
      join(root, "src/workspaces/ShipmentsWorkspace.tsx"),
      "utf8",
    );
    const transpiler = new Bun.Transpiler({ loader: "tsx" });
    const transformed = transpiler.transformSync(tsxCode);
    expect(transformed.length).toBeGreaterThan(0);

    for (const requiredText of [
      "2. 수입실적 관리",
      "수입일자",
      "국내 수입사",
      "[수입사 관리/수정]",
      "수입제품코드",
      "해외지사",
      "제품 수입량 (kg)",
      "제품 환산 중량 (ton)",
      "비고",
      "수정",
      "삭제",
      "computeVirtualWindow",
    ]) {
      expect(tsxCode).toContain(requiredText);
    }
  });
});
