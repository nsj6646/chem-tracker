import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import {
  SEED_REPORT_DATASET,
  buildReportDisplayModel,
  computeAnnualReportFromDataset,
  createDefaultReportFilter,
  createReportClient,
  renderPrintableReportHtml,
  validateReportFilter,
} from "../src/workspaces/annualReportLogic";

describe("Ticket 06 Annual Substance Totals & Mode-Aware Report Logic", () => {
  test("Slice 3: exact fixed-point aggregation, filtering, and confidentiality guard", () => {
    const defaultFilter = createDefaultReportFilter([2026, 2025]);
    expect(defaultFilter.periodMode).toBe("year");
    expect(defaultFilter.targetYear).toBe(2026);
    expect(defaultFilter.domesticImporterId).toBeNull();
    expect(defaultFilter.overseasBranchId).toBeNull();
    expect(defaultFilter.productId).toBeNull();

    // Filter 2026 + (주)대한케미칼 (id: 1)
    const report = computeAnnualReportFromDataset(SEED_REPORT_DATASET, {
      ...defaultFilter,
      targetYear: 2026,
      domesticImporterId: 1,
    });

    expect(report.matching_shipment_count).toBe(4);
    expect(report.unique_substance_count).toBe(7);
    expect(report.total_product_weight_kg_formatted).toBe("8,200.75");
    expect(report.total_substance_kg_formatted).toBe("8,200.75");
    expect(report.total_substance_ton_formatted).toBe("8.2008");

    // 67-64-1 (아세톤): 2400*45.25% + 1500.5*60% + 1100.25*45.25% = 2484.163125 kg
    const acetone = report.rows.find((r) => r.cas_number === "67-64-1");
    expect(acetone).toBeDefined();
    expect(acetone?.total_substance_kg_formatted).toBe("2,484.1631");
    expect(acetone?.total_substance_ton_formatted).toBe("2.4842");
    expect(acetone?.breakdown.length).toBe(3);

    // Custom date range validation
    expect(
      validateReportFilter({
        ...defaultFilter,
        periodMode: "custom_range",
        startDate: "2026-12-31",
        endDate: "2026-01-01",
      }),
    ).not.toBeNull();

    const rangeReport = computeAnnualReportFromDataset(SEED_REPORT_DATASET, {
      ...defaultFilter,
      periodMode: "custom_range",
      startDate: "2026-01-01",
      endDate: "2026-03-31",
      domesticImporterId: 1,
      overseasBranchId: 1,
      productId: 1,
    });
    expect(rangeReport.matching_shipment_count).toBe(1);
    expect(rangeReport.total_product_weight_kg_formatted).toBe("2,400.00");

    // External Summary Mode (default) strictly omits 함유율(%) & breakdown from display model & HTML
    const externalModel = buildReportDisplayModel(
      report,
      "external_summary",
      "2026-11-14",
    );
    expect(externalModel.modeLabel).toBe("대외 제출용 요약본");
    expect(externalModel.includeBreakdown).toBe(false);
    expect(externalModel.rows.every((r) => r.breakdown.length === 0)).toBe(true);

    const externalHtml = renderPrintableReportHtml(
      report,
      "external_summary",
      "2026-11-14",
    );
    expect(externalHtml).toContain("연간 화학물질 수입량 집계 보고서");
    expect(externalHtml).toContain("67-64-1");
    expect(externalHtml).toContain("2,484.1631");
    expect(externalHtml).not.toContain("함유율");
    expect(externalHtml).not.toContain("PRD-EU-101");
    expect(externalHtml).not.toContain("45.25%");

    // Internal Detail Mode includes breakdown & 함유율(%)
    const internalModel = buildReportDisplayModel(
      report,
      "internal_detail",
      "2026-11-14",
    );
    expect(internalModel.modeLabel).toBe("내부 검증용 상세본");
    expect(internalModel.includeBreakdown).toBe(true);

    const internalHtml = renderPrintableReportHtml(
      report,
      "internal_detail",
      "2026-11-14",
    );
    expect(internalHtml).toContain("함유율 (%)");
    expect(internalHtml).toContain("PRD-EU-101");
    expect(internalHtml).toContain("45.25");
  });

  test("Slice 3b: unrounded intermediate BigInt sum & small ton 5-decimal formatting", () => {
    const customDataset = {
      ...SEED_REPORT_DATASET,
      substances: [{ id: 99, cas_number: "50-00-0", substance_name: "포름알데히드" }, ...SEED_REPORT_DATASET.substances],
      products: [
        {
          id: 99,
          product_code: "PRD-TINY",
          product_name: "미량 시험제",
          overseas_branch_id: 1,
          components: [
            { substance_id: 99, weight_pct_text: "0.04" },
            { substance_id: 5, weight_pct_text: "99.96" },
          ],
        },
      ],
      shipments: [
        {
          id: 99,
          shipment_date: "2026-09-01",
          domestic_importer_id: 1,
          overseas_branch_id: 1,
          product_id: 99,
          import_weight_kg_text: "100",
        },
      ],
    };

    const report = computeAnnualReportFromDataset(customDataset, {
      periodMode: "year",
      targetYear: 2026,
      startDate: "",
      endDate: "",
      domesticImporterId: 1,
      overseasBranchId: null,
      productId: 99,
    });
    const row = report.rows.find((r) => r.cas_number === "50-00-0");
    expect(row?.total_substance_kg_formatted).toBe("0.04");
    expect(row?.total_substance_ton_formatted).toBe("0.00004");
  });

  test("Slice 3c: standalone report client returns options, report, and export feedback", async () => {
    const client = createReportClient();
    const options = await client.getFilterOptions();
    expect(options.years).toEqual([2026, 2025]);
    expect(options.domestic_importers.length).toBe(3);

    const report = await client.queryAnnualTotals({
      periodMode: "year",
      targetYear: 2026,
      startDate: "",
      endDate: "",
      domesticImporterId: 1,
      overseasBranchId: null,
      productId: null,
    });
    expect(report.matching_shipment_count).toBe(4);
  });

  test("Slice 4: AnnualReportWorkspace TSX transpiles, enforces controls/modes, and index.css has A4 print rules", () => {
    const root = join(import.meta.dir, "..");
    const workspaceSrc = readFileSync(
      join(root, "src/workspaces/AnnualReportWorkspace.tsx"),
      "utf8",
    );
    const transpiler = new Bun.Transpiler({ loader: "tsx" });
    expect(transpiler.transformSync(workspaceSrc).length).toBeGreaterThan(0);

    // Filter controls, mode toggle, Print/PDF, Excel export, and confidentiality guard
    expect(workspaceSrc).toContain("buildReportDisplayModel");
    expect(workspaceSrc).toContain("includeBreakdown && row.breakdown.length > 0");
    expect(workspaceSrc).toContain("window.print()");
    expect(workspaceSrc).toContain("대외 제출용 요약본");
    expect(workspaceSrc).toContain("내부 검증용 상세본");
    expect(workspaceSrc).toContain("전체 수입사");
    expect(workspaceSrc).toContain("대상 연도");
    expect(workspaceSrc).toContain("해외지사");
    expect(workspaceSrc).toContain("수입제품코드");
    expect(workspaceSrc).toContain("연간 화학물질 수입량 집계 보고서");

    const css = readFileSync(join(root, "src/index.css"), "utf8");
    expect(css).toContain("@page");
    expect(css).toContain("size: A4 portrait");
    expect(css).toContain("margin: 14mm 12mm");
    expect(css).toContain("Malgun Gothic");
    expect(css).toContain("print-color-adjust: exact");
    expect(css).toContain("display: table-header-group");
    expect(css).toContain("display: table-footer-group");
    expect(css).toContain("break-inside: avoid");
    expect(css).toContain(".a4-sheet");
  });
});
