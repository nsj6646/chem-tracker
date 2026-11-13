import { formatKg, formatPct, parseScaled4dp } from "../lib/decimal";

export type ReportMode = "external_summary" | "internal_detail";
export type PeriodMode = "year" | "custom_range";

export interface AnnualReportFilter {
  periodMode: PeriodMode;
  targetYear: number | null;
  startDate: string;
  endDate: string;
  domesticImporterId: number | null;
  overseasBranchId: number | null;
  productId: number | null;
}

export interface FilterOption {
  id: number;
  label: string;
}

export interface ProductFilterOption {
  id: number;
  product_code: string;
  product_name: string | null;
  overseas_branch_id: number;
}

export interface ReportFilterOptions {
  years: number[];
  domestic_importers: FilterOption[];
  overseas_branches: FilterOption[];
  imported_products: ProductFilterOption[];
}

export interface SubstanceShipmentBreakdown {
  shipment_id: number;
  shipment_date: string;
  domestic_importer_name: string;
  overseas_branch_name: string;
  product_code: string;
  product_name: string | null;
  import_weight_kg_formatted: string;
  weight_pct_formatted: string;
  substance_weight_kg_formatted: string;
  substance_weight_ton_formatted: string;
}

export interface SubstanceSummaryRow {
  no: number;
  substance_id: number;
  cas_number: string;
  substance_name: string | null;
  total_substance_kg_formatted: string;
  total_substance_ton_formatted: string;
  shipment_count: number;
  breakdown: SubstanceShipmentBreakdown[];
}

export interface AnnualSubstanceReport {
  period_label: string;
  domestic_importer_label: string;
  overseas_branch_label: string;
  product_label: string;
  matching_shipment_count: number;
  unique_substance_count: number;
  total_product_weight_kg_formatted: string;
  total_product_weight_ton_formatted: string;
  total_substance_kg_formatted: string;
  total_substance_ton_formatted: string;
  rows: SubstanceSummaryRow[];
}

export interface ReportDisplayModel {
  mode: ReportMode;
  modeLabel: string;
  issueDate: string;
  includeBreakdown: boolean;
  report: AnnualSubstanceReport;
  rows: SubstanceSummaryRow[];
}

export interface ReportExportResult {
  savedPath: string;
  sheetCount: number;
  message: string;
}

export interface ReportSampleDataset {
  domestic_importers: FilterOption[];
  overseas_branches: FilterOption[];
  substances: Array<{
    id: number;
    cas_number: string;
    substance_name: string | null;
  }>;
  products: Array<{
    id: number;
    product_code: string;
    product_name: string | null;
    overseas_branch_id: number;
    components: Array<{
      substance_id: number;
      weight_pct_text: string;
    }>;
  }>;
  shipments: Array<{
    id: number;
    shipment_date: string;
    domestic_importer_id: number;
    overseas_branch_id: number;
    product_id: number;
    import_weight_kg_text: string;
  }>;
}

export const SEED_REPORT_DATASET: ReportSampleDataset = {
  domestic_importers: [
    { id: 1, label: "(주)대한케미칼" },
    { id: 3, label: "세진소재산업(주)" },
    { id: 2, label: "한성정밀화학(주)" },
  ],
  overseas_branches: [
    { id: 2, label: "북미지사 (미국)" },
    { id: 1, label: "유럽지사 (독일)" },
    { id: 3, label: "일본지사 (도쿄)" },
  ],
  substances: [
    { id: 1, cas_number: "67-64-1", substance_name: "아세톤 (Acetone)" },
    { id: 2, cas_number: "108-88-3", substance_name: "톨루엔 (Toluene)" },
    { id: 3, cas_number: "141-78-6", substance_name: "초산에틸 (Ethyl acetate)" },
    { id: 4, cas_number: "71-36-3", substance_name: "1-부탄올 (1-Butanol)" },
    { id: 5, cas_number: "7732-18-5", substance_name: "정제수 (Water)" },
    { id: 6, cas_number: "1330-20-7", substance_name: "크실렌 (Xylene)" },
    { id: 7, cas_number: "9002-88-4", substance_name: null },
  ],
  products: [
    {
      id: 1,
      product_code: "PRD-EU-101",
      product_name: "코팅솔벤트 A-100",
      overseas_branch_id: 1,
      components: [
        { substance_id: 1, weight_pct_text: "45.25" },
        { substance_id: 2, weight_pct_text: "34.75" },
        { substance_id: 3, weight_pct_text: "20" },
      ],
    },
    {
      id: 2,
      product_code: "PRD-EU-204",
      product_name: "고기능 세정제 B-200",
      overseas_branch_id: 1,
      components: [
        { substance_id: 1, weight_pct_text: "60" },
        { substance_id: 4, weight_pct_text: "25.5" },
        { substance_id: 5, weight_pct_text: "14.5" },
      ],
    },
    {
      id: 3,
      product_code: "PRD-US-305",
      product_name: "표면처리제 C-300",
      overseas_branch_id: 2,
      components: [
        { substance_id: 6, weight_pct_text: "72.125" },
        { substance_id: 2, weight_pct_text: "17.875" },
        { substance_id: 7, weight_pct_text: "10" },
      ],
    },
  ],
  shipments: [
    {
      id: 1,
      shipment_date: "2026-01-15",
      domestic_importer_id: 1,
      overseas_branch_id: 1,
      product_id: 1,
      import_weight_kg_text: "2400",
    },
    {
      id: 2,
      shipment_date: "2026-03-10",
      domestic_importer_id: 1,
      overseas_branch_id: 1,
      product_id: 2,
      import_weight_kg_text: "1500.5",
    },
    {
      id: 3,
      shipment_date: "2026-05-22",
      domestic_importer_id: 1,
      overseas_branch_id: 2,
      product_id: 3,
      import_weight_kg_text: "3200",
    },
    {
      id: 4,
      shipment_date: "2026-08-04",
      domestic_importer_id: 1,
      overseas_branch_id: 1,
      product_id: 1,
      import_weight_kg_text: "1100.25",
    },
    {
      id: 5,
      shipment_date: "2026-04-18",
      domestic_importer_id: 2,
      overseas_branch_id: 1,
      product_id: 2,
      import_weight_kg_text: "4000",
    },
    {
      id: 6,
      shipment_date: "2025-11-12",
      domestic_importer_id: 1,
      overseas_branch_id: 1,
      product_id: 1,
      import_weight_kg_text: "1800",
    },
  ],
};

export function createDefaultReportFilter(
  availableYears: number[] = [2026],
): AnnualReportFilter {
  return {
    periodMode: "year",
    targetYear: availableYears[0] ?? 2026,
    startDate: "",
    endDate: "",
    domesticImporterId: null,
    overseasBranchId: null,
    productId: null,
  };
}

export function isValidIsoDate(value: string): boolean {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value.trim());
  if (!m) return false;
  const year = Number(m[1]);
  const month = Number(m[2]);
  const day = Number(m[3]);
  if (year < 1900 || year > 2999 || month < 1 || month > 12) return false;
  const isLeap = (year % 4 === 0 && year % 100 !== 0) || year % 400 === 0;
  const maxDays = [31, isLeap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
  return day >= 1 && day <= maxDays[month - 1];
}

export function validateReportFilter(filter: AnnualReportFilter): string | null {
  if (filter.periodMode !== "custom_range") {
    return null;
  }
  const start = filter.startDate.trim();
  const end = filter.endDate.trim();
  if (start && !isValidIsoDate(start)) {
    return `시작일은 YYYY-MM-DD 형식이어야 합니다: '${start}'`;
  }
  if (end && !isValidIsoDate(end)) {
    return `종료일은 YYYY-MM-DD 형식이어야 합니다: '${end}'`;
  }
  if (start && end && start > end) {
    return "조회 시작일은 종료일보다 늦을 수 없습니다.";
  }
  return null;
}

function roundHalfAwayBigInt(value: bigint, divisor: bigint): bigint {
  const neg = value < 0n;
  const abs = neg ? -value : value;
  const q = abs / divisor;
  const rem = abs % divisor;
  const rounded = rem * 2n >= divisor ? q + 1n : q;
  return neg && rounded !== 0n ? -rounded : rounded;
}

function formatBigIntFixed(
  scaled: bigint,
  scale: number,
  minDp: number,
  maxDp: number,
): string {
  const neg = scaled < 0n;
  const abs = neg ? -scaled : scaled;
  const factor = 10n ** BigInt(scale);
  const intPart = abs / factor;
  const fracVal = abs % factor;
  const intStr = intPart.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ",");
  const fracFull = fracVal.toString().padStart(scale, "0").slice(0, maxDp);
  let fracTrimmed = fracFull.replace(/0+$/, "");
  while (fracTrimmed.length < minDp) {
    fracTrimmed += "0";
  }
  const sign = neg ? "-" : "";
  return fracTrimmed.length > 0 ? `${sign}${intStr}.${fracTrimmed}` : `${sign}${intStr}`;
}

function formatKg1e10(kg1e10: bigint): string {
  const scaled4dp = roundHalfAwayBigInt(kg1e10, 1_000_000n);
  return formatKg(Number(scaled4dp));
}

function formatTon1e10(kg1e10: bigint): string {
  const scaledTon4 = roundHalfAwayBigInt(kg1e10, 1_000_000_000n);
  if (kg1e10 !== 0n && scaledTon4 === 0n) {
    const scaledTon5 = roundHalfAwayBigInt(kg1e10, 100_000_000n);
    return formatBigIntFixed(scaledTon5, 5, 5, 5);
  }
  return formatBigIntFixed(scaledTon4, 4, 4, 4);
}

function formatPeriodLabel(filter: AnnualReportFilter): string {
  if (filter.periodMode === "custom_range") {
    const s = filter.startDate.trim();
    const e = filter.endDate.trim();
    if (s && e) return `${s} ~ ${e}`;
    if (s) return `${s} ~`;
    if (e) return `~ ${e}`;
    return "전체 기간";
  }
  return filter.targetYear !== null
    ? `${filter.targetYear}년 (01.01 ~ 12.31)`
    : "전체 기간";
}

export function computeAnnualReportFromDataset(
  dataset: ReportSampleDataset,
  filter: AnnualReportFilter,
): AnnualSubstanceReport {
  const err = validateReportFilter(filter);
  if (err) {
    throw new Error(err);
  }

  const importerMap = new Map(dataset.domestic_importers.map((i) => [i.id, i.label]));
  const branchMap = new Map(dataset.overseas_branches.map((b) => [b.id, b.label]));
  const productMap = new Map(dataset.products.map((p) => [p.id, p]));
  const substanceMap = new Map(dataset.substances.map((s) => [s.id, s]));

  const start = filter.periodMode === "custom_range" ? filter.startDate.trim() : "";
  const end = filter.periodMode === "custom_range" ? filter.endDate.trim() : "";

  const matchingShipments = dataset.shipments
    .filter((s) => {
      if (filter.periodMode === "year" && filter.targetYear !== null) {
        if (Number(s.shipment_date.slice(0, 4)) !== filter.targetYear) return false;
      } else if (filter.periodMode === "custom_range") {
        if (start && s.shipment_date < start) return false;
        if (end && s.shipment_date > end) return false;
      }
      if (filter.domesticImporterId !== null && s.domestic_importer_id !== filter.domesticImporterId) {
        return false;
      }
      if (filter.overseasBranchId !== null && s.overseas_branch_id !== filter.overseasBranchId) {
        return false;
      }
      if (filter.productId !== null && s.product_id !== filter.productId) {
        return false;
      }
      return true;
    })
    .sort((a, b) => a.shipment_date.localeCompare(b.shipment_date) || a.id - b.id);

  let totalProductKgScaled = 0n;

  interface Acc {
    substanceId: number;
    casNumber: string;
    substanceName: string | null;
    totalKg1e10: bigint;
    breakdown: SubstanceShipmentBreakdown[];
  }
  const byCas = new Map<string, Acc>();

  for (const sh of matchingShipments) {
    const product = productMap.get(sh.product_id);
    if (!product) continue;

    const parsedWeight = parseScaled4dp(sh.import_weight_kg_text);
    if (!parsedWeight.ok) continue;

    const weightScaled = BigInt(parsedWeight.scaled);
    totalProductKgScaled += weightScaled;

    for (const comp of product.components) {
      const substance = substanceMap.get(comp.substance_id);
      if (!substance) continue;

      const parsedPct = parseScaled4dp(comp.weight_pct_text);
      if (!parsedPct.ok) continue;

      const pctScaled = BigInt(parsedPct.scaled);
      const subKg1e10 = weightScaled * pctScaled;

      let acc = byCas.get(substance.cas_number);
      if (!acc) {
        acc = {
          substanceId: substance.id,
          casNumber: substance.cas_number,
          substanceName: substance.substance_name,
          totalKg1e10: 0n,
          breakdown: [],
        };
        byCas.set(substance.cas_number, acc);
      }

      acc.totalKg1e10 += subKg1e10;
      acc.breakdown.push({
        shipment_id: sh.id,
        shipment_date: sh.shipment_date,
        domestic_importer_name: importerMap.get(sh.domestic_importer_id) ?? "-",
        overseas_branch_name: branchMap.get(sh.overseas_branch_id) ?? "-",
        product_code: product.product_code,
        product_name: product.product_name,
        import_weight_kg_formatted: formatKg(parsedWeight.scaled),
        weight_pct_formatted: formatPct(parsedPct.scaled),
        substance_weight_kg_formatted: formatKg1e10(subKg1e10),
        substance_weight_ton_formatted: formatTon1e10(subKg1e10),
      });
    }
  }

  const sortedEntries = [...byCas.values()].sort((a, b) =>
    a.casNumber < b.casNumber ? -1 : a.casNumber > b.casNumber ? 1 : 0,
  );

  let totalSubstanceKg1e10 = 0n;
  const rows: SubstanceSummaryRow[] = sortedEntries.map((acc, idx) => {
    totalSubstanceKg1e10 += acc.totalKg1e10;
    return {
      no: idx + 1,
      substance_id: acc.substanceId,
      cas_number: acc.casNumber,
      substance_name: acc.substanceName,
      total_substance_kg_formatted: formatKg1e10(acc.totalKg1e10),
      total_substance_ton_formatted: formatTon1e10(acc.totalKg1e10),
      shipment_count: acc.breakdown.length,
      breakdown: acc.breakdown,
    };
  });

  const selectedProduct =
    filter.productId !== null ? productMap.get(filter.productId) : undefined;
  const productLabel = selectedProduct
    ? selectedProduct.product_name
      ? `${selectedProduct.product_code} (${selectedProduct.product_name})`
      : selectedProduct.product_code
    : "전체 수입제품";

  return {
    period_label: formatPeriodLabel(filter),
    domestic_importer_label:
      filter.domesticImporterId !== null
        ? importerMap.get(filter.domesticImporterId) ?? "전체 국내 수입사"
        : "전체 국내 수입사",
    overseas_branch_label:
      filter.overseasBranchId !== null
        ? branchMap.get(filter.overseasBranchId) ?? "전체 해외지사"
        : "전체 해외지사",
    product_label: productLabel,
    matching_shipment_count: matchingShipments.length,
    unique_substance_count: rows.length,
    total_product_weight_kg_formatted: formatKg(Number(totalProductKgScaled)),
    total_product_weight_ton_formatted: formatTon1e10(totalProductKgScaled * 1_000_000n),
    total_substance_kg_formatted: formatKg1e10(totalSubstanceKg1e10),
    total_substance_ton_formatted: formatTon1e10(totalSubstanceKg1e10),
    rows,
  };
}

export function buildReportDisplayModel(
  report: AnnualSubstanceReport,
  mode: ReportMode,
  issueDate = new Date().toISOString().slice(0, 10),
): ReportDisplayModel {
  const includeBreakdown = mode === "internal_detail";
  // Confidentiality guard: strip breakdown data completely in external summary mode
  const rows: SubstanceSummaryRow[] = report.rows.map((row) => ({
    ...row,
    breakdown: includeBreakdown ? row.breakdown : [],
  }));

  return {
    mode,
    modeLabel: includeBreakdown ? "내부 검증용 상세본" : "대외 제출용 요약본",
    issueDate,
    includeBreakdown,
    report,
    rows,
  };
}

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

export function renderPrintableReportHtml(
  report: AnnualSubstanceReport,
  mode: ReportMode,
  issueDate = new Date().toISOString().slice(0, 10),
): string {
  const model = buildReportDisplayModel(report, mode, issueDate);
  const rowsHtml = model.rows
    .map((row) => {
      const summaryTr = `<tr>
        <td>${row.no}</td>
        <td>${escapeHtml(row.cas_number)}</td>
        <td>${escapeHtml(row.substance_name ?? "— (미입력)")}</td>
        <td>${escapeHtml(row.total_substance_kg_formatted)} kg</td>
        <td>${escapeHtml(row.total_substance_ton_formatted)} ton</td>
      </tr>`;

      if (!model.includeBreakdown || row.breakdown.length === 0) {
        return summaryTr;
      }

      const detailRows = row.breakdown
        .map(
          (b) => `<tr>
            <td>${escapeHtml(b.shipment_date)}</td>
            <td>${escapeHtml(b.domestic_importer_name)}</td>
            <td>${escapeHtml(b.overseas_branch_name)}</td>
            <td>${escapeHtml(b.product_code)}</td>
            <td>${escapeHtml(b.import_weight_kg_formatted)} kg</td>
            <td>${escapeHtml(b.weight_pct_formatted)}%</td>
            <td>${escapeHtml(b.substance_weight_kg_formatted)} kg / ${escapeHtml(b.substance_weight_ton_formatted)} ton</td>
          </tr>`,
        )
        .join("");

      return `${summaryTr}<tr><td colspan="5">
        <table>
          <thead><tr>
            <th>수입일자</th>
            <th>국내 수입사</th>
            <th>해외지사</th>
            <th>수입제품코드</th>
            <th>제품 수입량 (kg)</th>
            <th>함유율 (%)</th>
            <th>화학물질 환산 중량 (kg / ton)</th>
          </tr></thead>
          <tbody>${detailRows}</tbody>
        </table>
      </td></tr>`;
    })
    .join("");

  return `<section class="a4-sheet">
    <h1>연간 화학물질 수입량 집계 보고서</h1>
    <div>보고서 구분: ${escapeHtml(model.modeLabel)} | 발행일자: ${escapeHtml(model.issueDate)}</div>
    <div>대상 기간: ${escapeHtml(report.period_label)} | 국내 수입사: ${escapeHtml(report.domestic_importer_label)} | 해외지사: ${escapeHtml(report.overseas_branch_label)}</div>
    <div>집계 물질 수: ${report.unique_substance_count}종 | 수입제품 총 중량: ${escapeHtml(report.total_product_weight_kg_formatted)} kg (${escapeHtml(report.total_product_weight_ton_formatted)} ton)</div>
    <table>
      <thead><tr>
        <th>NO</th>
        <th>CAS번호</th>
        <th>화학물질명</th>
        <th>연간 총 수입량 (kg)</th>
        <th>연간 총 수입량 (ton)</th>
      </tr></thead>
      <tbody>${rowsHtml}</tbody>
      <tfoot><tr>
        <th colspan="3">합계</th>
        <th>${escapeHtml(report.total_substance_kg_formatted)} kg</th>
        <th>${escapeHtml(report.total_substance_ton_formatted)} ton</th>
      </tr></tfoot>
    </table>
  </section>`;
}

export interface ReportClient {
  getFilterOptions(): Promise<ReportFilterOptions>;
  queryAnnualTotals(filter: AnnualReportFilter): Promise<AnnualSubstanceReport>;
  exportXlsx(filter: AnnualReportFilter, mode: ReportMode): Promise<ReportExportResult>;
}

type TauriInvoke = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;

function detectTauriInvoke(): TauriInvoke | null {
  const g = globalThis as {
    __TAURI_INTERNALS__?: { invoke?: TauriInvoke };
  };
  return typeof g.__TAURI_INTERNALS__?.invoke === "function"
    ? g.__TAURI_INTERNALS__.invoke
    : null;
}

export function createReportClient(invokeOverride?: TauriInvoke): ReportClient {
  const invoke = invokeOverride ?? detectTauriInvoke();
  if (invoke) {
    return {
      getFilterOptions: () => invoke<ReportFilterOptions>("get_report_filter_options"),
      queryAnnualTotals: (filter) =>
        invoke<AnnualSubstanceReport>("query_annual_substance_totals", { filter }),
      exportXlsx: (filter, mode) =>
        invoke<ReportExportResult>("export_annual_report_xlsx", { filter, mode }),
    };
  }

  return {
    async getFilterOptions(): Promise<ReportFilterOptions> {
      const years = Array.from(
        new Set(SEED_REPORT_DATASET.shipments.map((s) => Number(s.shipment_date.slice(0, 4)))),
      ).sort((a, b) => b - a);
      return {
        years,
        domestic_importers: SEED_REPORT_DATASET.domestic_importers,
        overseas_branches: SEED_REPORT_DATASET.overseas_branches,
        imported_products: SEED_REPORT_DATASET.products.map((p) => ({
          id: p.id,
          product_code: p.product_code,
          product_name: p.product_name,
          overseas_branch_id: p.overseas_branch_id,
        })),
      };
    },
    async queryAnnualTotals(filter): Promise<AnnualSubstanceReport> {
      return computeAnnualReportFromDataset(SEED_REPORT_DATASET, filter);
    },
    async exportXlsx(_filter, mode): Promise<ReportExportResult> {
      const isInternal = mode === "internal_detail";
      const fileTag = isInternal ? "내부검증용_상세본" : "대외제출용_요약본";
      const sheetCount = isInternal ? 2 : 1;
      return {
        savedPath: `연간물질집계_${fileTag}.xlsx`,
        sheetCount,
        message: isInternal
          ? "내부 검증용 상세본(.xlsx, 2개 시트) 내보내기 준비 완료 — CAS번호·수입제품코드 텍스트(@) 포맷 적용"
          : "대외 제출용 요약본(.xlsx, 1개 시트 · 함유율% 제외) 내보내기 준비 완료 — CAS번호 텍스트(@) 포맷 적용",
      };
    },
  };
}
