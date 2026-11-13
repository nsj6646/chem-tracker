import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import {
  HUNDRED_PERCENT_SCALED,
  SCALE_FACTOR,
  computeSubstanceKgScaled,
  formatKg,
  formatPct,
  formatTonFromKgScaled,
  parseScaled4dp,
} from "../src/lib/decimal";
import {
  APP_TITLE,
  WORKSPACE_TABS,
  createInitialShellState,
  formatDbStatusSummary,
  reduceShellState,
} from "../src/lib/appState";
import { createIpcClient } from "../src/lib/ipc";

describe("Ticket 01 Frontend Core & Variant A Shell", () => {
  test("fixed-point 4dp parsing, exact BigInt multiplication, and formatting", () => {
    expect(SCALE_FACTOR).toBe(10_000);
    expect(HUNDRED_PERCENT_SCALED).toBe(1_000_000);

    const p100 = parseScaled4dp("100.0000");
    expect(p100).toEqual({ ok: true, scaled: 1_000_000, canonical: "100" });

    const pWeight = parseScaled4dp("1,250.5");
    expect(pWeight).toEqual({ ok: true, scaled: 12_505_000, canonical: "1250.5" });

    const pTooManyDp = parseScaled4dp("12.34567");
    expect(pTooManyDp.ok).toBe(false);

    // 1250.5 kg * 35.25% = 440.80125 kg -> 4 decimals scaled = 4,408,013 (MidpointAwayFromZero)
    const kgScaled = computeSubstanceKgScaled(12_505_000, 352_500);
    expect(kgScaled).toBe(4_408_013);
    expect(formatPct(350_000)).toBe("35.00");
    expect(formatPct(123_450)).toBe("12.345");
    expect(formatKg(12_505_000)).toBe("1,250.50");
    expect(formatTonFromKgScaled(4_408_013)).toBe("0.4408");
    // Very small non-zero kg (0.04 kg = 400 scaled -> 0.00004 ton -> 5 decimals)
    expect(formatTonFromKgScaled(400)).toBe("0.00004");
  });

  test("Variant A navigation tabs and shell state transitions", () => {
    expect(APP_TITLE).toBe("화학물질 수입량 집계 시스템 — 코리아 지사");
    expect(WORKSPACE_TABS.map((t) => t.label)).toEqual([
      "1. 수입제품 및 레시피(BOM)",
      "2. 수입실적 관리",
      "3. 엑셀 일괄 가져오기",
      "4. 연간 물질 집계 & A4 보고서",
    ]);

    let state = createInitialShellState();
    expect(state.activeTab).toBe("bom");

    state = reduceShellState(state, { type: "SET_TAB", tab: "report" });
    expect(state.activeTab).toBe("report");

    state = reduceShellState(state, { type: "SEED_STARTED" });
    expect(state.seeding).toBe(true);

    const status = {
      db_path: "data.db",
      journal_mode: "wal",
      integrity_ok: true,
      domestic_importers_count: 3,
      overseas_branches_count: 3,
      chemical_substances_count: 7,
      imported_products_count: 3,
      product_components_count: 9,
      import_shipments_count: 6,
    };

    state = reduceShellState(state, {
      type: "SEED_SUCCEEDED",
      result: {
        inserted_importers: 3,
        inserted_branches: 3,
        inserted_substances: 7,
        inserted_products: 3,
        inserted_shipments: 6,
        already_seeded: false,
        status,
      },
    });
    expect(state.seeding).toBe(false);
    expect(state.dbStatus?.imported_products_count).toBe(3);
    expect(formatDbStatusSummary(status)).toContain("WAL");
    expect(formatDbStatusSummary(status)).toContain("제품 3");
    expect(formatDbStatusSummary(status)).toContain("실적 6");
  });

  test("IPC client fallback seeds sample data and updates status", async () => {
    const ipc = createIpcClient();
    const initial = await ipc.getDbStatus();
    expect(initial.imported_products_count).toBe(0);
    expect(initial.journal_mode.toLowerCase()).toBe("wal");

    const seeded = await ipc.seedSampleData();
    expect(seeded.already_seeded).toBe(false);
    expect(seeded.status.imported_products_count).toBe(3);
    expect(seeded.status.import_shipments_count).toBe(6);

    const after = await ipc.getDbStatus();
    expect(after.imported_products_count).toBe(3);
  });

  test("Variant A CSS & App shell enforce 240px sidebar and @media print rules", () => {
    const root = join(import.meta.dir, "..");
    const css = readFileSync(join(root, "src/index.css"), "utf8");
    const indexHtml = readFileSync(join(root, "index.html"), "utf8");
    const appTsx = readFileSync(join(root, "src/App.tsx"), "utf8");
    const footerTsx = readFileSync(join(root, "src/components/DatabaseFooterPanel.tsx"), "utf8");

    expect(css).toContain("240px");
    expect(css).toContain("@media print");
    expect(css).toContain("A4 portrait");
    expect(css).toContain("Malgun Gothic");

    expect(indexHtml).toContain("화학물질 수입량 집계 시스템 — 코리아 지사");
    expect(appTsx).toContain("APP_TITLE");
    expect(appTsx).toContain("DatabaseFooterPanel");
    expect(footerTsx).toContain("DB 백업 저장 (.db)");
    expect(footerTsx).toContain("DB 백업 불러오기 (복원)");
    expect(footerTsx).toContain("샘플 데이터 적재");

    const transpiler = new Bun.Transpiler({ loader: "tsx" });
    const tsxFiles = [
      "src/main.tsx",
      "src/App.tsx",
      "src/components/DatabaseFooterPanel.tsx",
      "src/workspaces/ProductsWorkspace.tsx",
      "src/workspaces/ShipmentsWorkspace.tsx",
      "src/workspaces/MasterModal.tsx",
      "src/workspaces/ExcelImportWorkspace.tsx",
      "src/workspaces/AnnualReportWorkspace.tsx",
    ];
    for (const rel of tsxFiles) {
      const code = readFileSync(join(root, rel), "utf8");
      const output = transpiler.transformSync(code);
      expect(output.length).toBeGreaterThan(0);
    }
  });
});
