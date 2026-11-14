import { describe, expect, test } from "bun:test";
import { readFileSync } from "fs";
import { join } from "path";
import {
  formatBigIntFixed,
  normalizeCasNumber,
  roundHalfAway,
  validateIsoCalendarDate,
} from "../src/lib/decimal";
import { DB_RESTORED_EVENT, detectTauriInvoke } from "../src/lib/ipc";
import {
  computeAnnualReportFromDataset,
  createReportClient,
  SEED_REPORT_DATASET,
  toSerializedReportFilter,
  type AnnualReportFilter,
} from "../src/workspaces/annualReportLogic";
import { parseCsvImportPreview } from "../src/workspaces/excelImportLogic";
import { evaluateBomGauge } from "../src/workspaces/productsLogic";

const ROOT = join(import.meta.dir, "..");

describe("Code review fixes verification", () => {
  test("(A) App.tsx & all workspaces wire dbRevision / DB_RESTORED_EVENT refresh and transpile cleanly", () => {
    const filesToTranspile = [
      "src/App.tsx",
      "src/components/DatabaseFooterPanel.tsx",
      "src/workspaces/ProductsWorkspace.tsx",
      "src/workspaces/ShipmentsWorkspace.tsx",
      "src/workspaces/ExcelImportWorkspace.tsx",
      "src/workspaces/AnnualReportWorkspace.tsx",
    ];
    const transpiler = new Bun.Transpiler({ loader: "tsx" });

    for (const relPath of filesToTranspile) {
      const code = readFileSync(join(ROOT, relPath), "utf8");
      expect(transpiler.transformSync(code).length).toBeGreaterThan(0);
    }

    const appSrc = readFileSync(join(ROOT, "src/App.tsx"), "utf8");
    expect(appSrc).toContain("onDatabaseRestored={handleDatabaseRestored}");
    expect(appSrc).toContain("setDbRevision((prev) => prev + 1)");
    expect(appSrc).toContain("<ProductsWorkspace refreshKey={dbRevision}");
    expect(appSrc).toContain("<ShipmentsWorkspace refreshKey={dbRevision}");
    expect(appSrc).toContain("<ExcelImportWorkspace refreshKey={dbRevision}");
    expect(appSrc).toContain("<AnnualReportWorkspace refreshKey={dbRevision}");

    for (const wsPath of [
      "src/workspaces/ProductsWorkspace.tsx",
      "src/workspaces/ShipmentsWorkspace.tsx",
      "src/workspaces/ExcelImportWorkspace.tsx",
      "src/workspaces/AnnualReportWorkspace.tsx",
    ]) {
      const wsSrc = readFileSync(join(ROOT, wsPath), "utf8");
      expect(wsSrc).toContain("refreshKey");
      expect(wsSrc).toContain("subscribeDbRestoredEvent");
    }
    expect(DB_RESTORED_EVENT).toBe("chem-tracker:db-restored");
  });

  test("(B) annualReportLogic aligns IPC serialization, custom_range semantics, and export_annual_report_xlsx", async () => {
    const staleYearFilter: AnnualReportFilter = {
      periodMode: "custom_range",
      targetYear: 2025,
      startDate: "",
      endDate: "",
      domesticImporterId: null,
      overseasBranchId: null,
      productId: null,
    };

    // Offline aggregation ignores targetYear in custom_range
    const allPeriodReport = computeAnnualReportFromDataset(
      SEED_REPORT_DATASET,
      staleYearFilter,
    );
    expect(allPeriodReport.period_label).toBe("전체 기간");
    expect(allPeriodReport.matching_shipment_count).toBe(6);

    // Serialized payload nulls out target_year in custom_range & nulls dates in year mode
    expect(toSerializedReportFilter(staleYearFilter)).toEqual({
      period_mode: "custom_range",
      target_year: null,
      start_date: null,
      end_date: null,
      domestic_importer_id: null,
      overseas_branch_id: null,
      product_id: null,
    });
    expect(
      toSerializedReportFilter({
        periodMode: "year",
        targetYear: 2026,
        startDate: "2025-01-01",
        endDate: "2025-12-31",
        domesticImporterId: 1,
        overseasBranchId: 2,
        productId: 3,
      }),
    ).toEqual({
      period_mode: "year",
      target_year: 2026,
      start_date: null,
      end_date: null,
      domestic_importer_id: 1,
      overseas_branch_id: 2,
      product_id: 3,
    });

    // Cancellation on plugin:dialog|save skips export_annual_report_xlsx invoke
    const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
    const cancelledClient = createReportClient(async <T>(cmd: string, args?: Record<string, unknown>) => {
      calls.push({ cmd, args });
      return null as T;
    });
    const cancelledRes = await cancelledClient.exportXlsx(staleYearFilter, "external_summary");
    expect(cancelledRes).toBeNull();
    expect(calls.map((c) => c.cmd)).toEqual(["plugin:dialog|save"]);

    // Successful save picks path & forwards { filter, mode, outputPath }
    const savedCalls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
    const tauriClient = createReportClient(async <T>(cmd: string, args?: Record<string, unknown>) => {
      savedCalls.push({ cmd, args });
      if (cmd === "plugin:dialog|save") {
        return "C:\\reports\\out.xlsx" as T;
      }
      if (cmd === "export_annual_report_xlsx") {
        return {
          saved_path: "C:\\reports\\out.xlsx",
          sheet_count: 2,
          message: "완료",
        } as T;
      }
      throw new Error(`Unexpected command: ${cmd}`);
    });
    const exportRes = await tauriClient.exportXlsx(staleYearFilter, "internal_detail");
    expect(exportRes).toEqual({
      saved_path: "C:\\reports\\out.xlsx",
      sheet_count: 2,
      message: "완료",
    });
    expect(savedCalls.map((c) => c.cmd)).toEqual([
      "plugin:dialog|save",
      "export_annual_report_xlsx",
    ]);
    expect(savedCalls[1].args).toEqual({
      filter: toSerializedReportFilter(staleYearFilter),
      mode: "internal_detail",
      outputPath: "C:\\reports\\out.xlsx",
    });
  });

  test("(D) Shared ipc/decimal helpers, CAS normalization in BOM/CSV, and ShipmentsWorkspace GLOSSARY labels", () => {
    expect(detectTauriInvoke()).toBeNull();
    expect(validateIsoCalendarDate("2024-02-29")).toBe("2024-02-29");
    expect(validateIsoCalendarDate("2026-02-29")).toBeNull();
    expect(roundHalfAway(15n, 10n)).toBe(2n);
    expect(formatBigIntFixed(123456n, 4, 2, 4, true)).toBe("12.3456");
    expect(normalizeCasNumber(" 67 \u{2013} 64 - 1 ")).toBe("67-64-1");

    // Duplicate detection in evaluateBomGauge catches Unicode dash vs ASCII hyphen
    const gauge = evaluateBomGauge([
      { casNumber: "67-64-1", substanceName: "아세톤", weightPctText: "60" },
      { casNumber: "67\u{2013}64-1", substanceName: "아세톤", weightPctText: "40" },
    ]);
    expect(gauge.canSave).toBe(false);
    expect(gauge.duplicateCasNumbers).toContain("67-64-1");

    // CSV preview catches Unicode-dash duplicate CAS rows within the same product
    const csvPreview = parseCsvImportPreview(
      "bom",
      "bom.csv",
      [
        "해외지사,수입제품코드,제품명,CAS번호,화학물질명,함유율(%)",
        "유럽지사 (독일),PRD-CAS-1,시험제,67-64-1,아세톤,50",
        "유럽지사 (독일),PRD-CAS-1,시험제,67 \u{2013} 64 - 1,아세톤,50",
      ].join("\n"),
      { existingProductCodes: [], existingProducts: [], existingShipmentSignatures: new Set() },
    );
    expect(csvPreview.can_commit).toBe(false);
    expect(csvPreview.error_count).toBe(2);

    // ShipmentsWorkspace uses '제품 환산 중량 (ton)' and never bare '환산 중량 (ton)'
    const shipSrc = readFileSync(
      join(ROOT, "src/workspaces/ShipmentsWorkspace.tsx"),
      "utf8",
    );
    const bareMatches = shipSrc.match(/(?<!제품 )환산 중량 \(ton\)/g);
    expect(bareMatches).toBeNull();
    expect(shipSrc).toContain("제품 환산 중량 (ton)");
  });
});
