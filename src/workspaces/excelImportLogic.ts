import {
  HUNDRED_PERCENT_SCALED,
  formatPct,
  normalizeCasNumber,
  parseScaled4dp,
  validateIsoCalendarDate,
} from "../lib/decimal";
import { type TauriInvoke, detectTauriInvoke } from "../lib/ipc";

export type ImportKind = "bom" | "shipments";
export type PreviewRowStatus = "valid" | "conflict" | "error";

export interface TemplateDownloadResult {
  kind: ImportKind;
  output_path: string;
  sheet_name: string;
}

export interface BomProductSummary {
  product_code: string;
  product_name: string | null;
  overseas_branch_name: string;
  row_numbers: number[];
  component_count: number;
  weight_pct_sum_scaled: number;
  weight_pct_sum_display: string;
  exists_in_db: boolean;
  status: PreviewRowStatus;
  errors: string[];
}

export interface ImportPreviewRow {
  row_number: number;
  status: PreviewRowStatus;
  is_conflict: boolean;
  conflict_reason: string | null;
  errors: string[];
  overseas_branch_name: string;
  product_code: string;
  product_name: string | null;
  cas_number: string | null;
  substance_name: string | null;
  weight_pct_text: string | null;
  weight_pct_scaled: number | null;
  shipment_date: string | null;
  domestic_importer_name: string | null;
  import_weight_kg_text: string | null;
  import_weight_kg_scaled: number | null;
  note: string | null;
}

export interface ImportPreviewResult {
  kind: ImportKind;
  file_path: string;
  file_name: string;
  total_rows: number;
  total_products: number;
  valid_count: number;
  conflict_count: number;
  error_count: number;
  can_commit: boolean;
  unknown_product_codes: string[];
  bom_products: BomProductSummary[];
  rows: ImportPreviewRow[];
}

export interface ImportOptions {
  overwrite_existing: boolean;
  skip_duplicates: boolean;
}

export interface ImportCommitResult {
  kind: ImportKind;
  inserted_count: number;
  updated_count: number;
  skipped_count: number;
  total_processed: number;
  message: string;
}

export interface ExcelImportClient {
  isTauriRuntime: boolean;
  pickTemplateSavePath(kind: ImportKind): Promise<string | null>;
  downloadImportTemplate(
    kind: ImportKind,
    outputPath: string,
  ): Promise<TemplateDownloadResult>;
  pickImportFile(): Promise<string | null>;
  previewExcelImport(
    kind: ImportKind,
    filePath: string,
  ): Promise<ImportPreviewResult>;
  commitExcelImport(
    kind: ImportKind,
    filePath: string,
    options: ImportOptions,
  ): Promise<ImportCommitResult>;
  registerInlineCsvFile?(fileName: string, csvText: string): string;
}

export interface ImportWorkspaceState {
  kind: ImportKind;
  selectedFilePath: string | null;
  preview: ImportPreviewResult | null;
  options: ImportOptions;
  busyAction: "none" | "template" | "preview" | "commit";
  feedbackMessage: string | null;
  errorMessage: string | null;
  lastCommitResult: ImportCommitResult | null;
}

export type ImportWorkspaceAction =
  | { type: "SET_KIND"; kind: ImportKind }
  | { type: "SET_OVERWRITE_EXISTING"; value: boolean }
  | { type: "SET_SKIP_DUPLICATES"; value: boolean }
  | { type: "TEMPLATE_STARTED" }
  | { type: "TEMPLATE_SUCCEEDED"; result: TemplateDownloadResult }
  | { type: "TEMPLATE_FAILED"; message: string }
  | { type: "PREVIEW_STARTED"; filePath: string }
  | {
      type: "PREVIEW_SUCCEEDED";
      filePath: string;
      preview: ImportPreviewResult;
    }
  | { type: "PREVIEW_FAILED"; message: string }
  | { type: "COMMIT_STARTED" }
  | { type: "COMMIT_SUCCEEDED"; result: ImportCommitResult }
  | { type: "COMMIT_FAILED"; message: string }
  | { type: "CLEAR_PREVIEW" };

export function defaultTemplateFilename(kind: ImportKind): string {
  return kind === "bom"
    ? "제품구성성분_BOM_양식.xlsx"
    : "수입실적_양식.xlsx";
}

export function createInitialImportOptions(): ImportOptions {
  return {
    overwrite_existing: true,
    skip_duplicates: true,
  };
}

export function createInitialImportState(
  initialKind: ImportKind = "bom",
): ImportWorkspaceState {
  return {
    kind: initialKind,
    selectedFilePath: null,
    preview: null,
    options: createInitialImportOptions(),
    busyAction: "none",
    feedbackMessage: null,
    errorMessage: null,
    lastCommitResult: null,
  };
}

export function reduceImportState(
  state: ImportWorkspaceState,
  action: ImportWorkspaceAction,
): ImportWorkspaceState {
  switch (action.type) {
    case "SET_KIND":
      if (action.kind === state.kind) return state;
      return {
        ...state,
        kind: action.kind,
        selectedFilePath: null,
        preview: null,
        errorMessage: null,
        feedbackMessage: null,
        lastCommitResult: null,
      };
    case "SET_OVERWRITE_EXISTING":
      return {
        ...state,
        options: { ...state.options, overwrite_existing: action.value },
      };
    case "SET_SKIP_DUPLICATES":
      return {
        ...state,
        options: { ...state.options, skip_duplicates: action.value },
      };
    case "TEMPLATE_STARTED":
      return {
        ...state,
        busyAction: "template",
        errorMessage: null,
        feedbackMessage: null,
      };
    case "TEMPLATE_SUCCEEDED":
      return {
        ...state,
        busyAction: "none",
        errorMessage: null,
        feedbackMessage: `양식 저장 완료: ${action.result.output_path}`,
      };
    case "TEMPLATE_FAILED":
      return {
        ...state,
        busyAction: "none",
        errorMessage: action.message,
      };
    case "PREVIEW_STARTED":
      return {
        ...state,
        busyAction: "preview",
        selectedFilePath: action.filePath,
        errorMessage: null,
        feedbackMessage: null,
        lastCommitResult: null,
      };
    case "PREVIEW_SUCCEEDED":
      return {
        ...state,
        busyAction: "none",
        selectedFilePath: action.filePath,
        preview: action.preview,
        errorMessage: null,
      };
    case "PREVIEW_FAILED":
      return {
        ...state,
        busyAction: "none",
        preview: null,
        errorMessage: action.message,
      };
    case "COMMIT_STARTED":
      return {
        ...state,
        busyAction: "commit",
        errorMessage: null,
        feedbackMessage: null,
      };
    case "COMMIT_SUCCEEDED":
      return {
        ...state,
        busyAction: "none",
        lastCommitResult: action.result,
        feedbackMessage: action.result.message,
        errorMessage: null,
      };
    case "COMMIT_FAILED":
      return {
        ...state,
        busyAction: "none",
        errorMessage: action.message,
      };
    case "CLEAR_PREVIEW":
      return {
        ...state,
        selectedFilePath: null,
        preview: null,
        errorMessage: null,
        feedbackMessage: null,
        lastCommitResult: null,
      };
  }
}

export function canCommitImport(state: ImportWorkspaceState): boolean {
  return (
    state.busyAction === "none" &&
    state.preview !== null &&
    state.preview.can_commit &&
    state.preview.error_count === 0
  );
}

export function getConflictToggleCopy(kind: ImportKind): {
  title: string;
  activeLabel: string;
  inactiveLabel: string;
} {
  if (kind === "bom") {
    return {
      title: "기존 수입제품 충돌 처리",
      activeLabel: "기존 수입제품 레시피 덮어쓰기 (기본값: 켜짐)",
      inactiveLabel: "기존 제품 건너뛰기",
    };
  }
  return {
    title: "중복 수입실적 처리",
    activeLabel: "중복 수입실적 건너뛰기 (기본값: 켜짐)",
    inactiveLabel: "중복 포함 강제 추가",
  };
}

export function computeEffectiveCommitPlan(
  preview: ImportPreviewResult | null,
  options: ImportOptions,
): string {
  if (!preview) return "";
  if (!preview.can_commit || preview.error_count > 0) {
    return `오류 ${preview.error_count}건을 수정한 파일을 다시 업로드해야 일괄 적재를 실행할 수 있습니다.`;
  }
  if (preview.kind === "bom") {
    const newCount = preview.valid_count;
    if (options.overwrite_existing) {
      return `실행 예정: 신규 ${newCount}개 등록 · 기존 ${preview.conflict_count}개 덮어쓰기`;
    }
    return `실행 예정: 신규 ${newCount}개 등록 · 기존 ${preview.conflict_count}개 건너뛰기`;
  }
  const newCount = preview.valid_count;
  if (options.skip_duplicates) {
    return `실행 예정: 신규 ${newCount}건 추가 · 중복 ${preview.conflict_count}건 건너뛰기`;
  }
  return `실행 예정: 신규 ${newCount}건 + 중복 ${preview.conflict_count}건 포함 총 ${newCount + preview.conflict_count}건 강제 추가`;
}

export interface ExistingShipmentKeyTuple {
  shipment_date: string;
  domestic_importer_name: string;
  overseas_branch_name: string;
  product_code: string;
  import_weight_kg_scaled: number;
}

export interface StandalonePreviewContext {
  existingProductCodes: string[];
  productBranches?: Record<string, string>;
  productNames?: Record<string, string | null>;
  existingShipments: ExistingShipmentKeyTuple[];
}

export function parseCsvRecords(csvText: string): string[][] {
  const clean = csvText.replace(/^\uFEFF/, "");
  const rows: string[][] = [];
  let row: string[] = [];
  let field = "";
  let inQuotes = false;
  let fieldStarted = false;

  for (let i = 0; i < clean.length; i++) {
    const ch = clean[i];
    if (inQuotes) {
      if (ch === '"') {
        if (clean[i + 1] === '"') {
          field += '"';
          i++;
        } else {
          inQuotes = false;
        }
      } else {
        field += ch;
      }
      continue;
    }

    if (ch === '"' && !fieldStarted && field === "") {
      inQuotes = true;
      fieldStarted = true;
    } else if (ch === ",") {
      row.push(field.trim());
      field = "";
      fieldStarted = false;
    } else if (ch === "\r") {
      if (clean[i + 1] === "\n") i++;
      row.push(field.trim());
      rows.push(row);
      row = [];
      field = "";
      fieldStarted = false;
    } else if (ch === "\n") {
      row.push(field.trim());
      rows.push(row);
      row = [];
      field = "";
      fieldStarted = false;
    } else {
      field += ch;
      fieldStarted = true;
    }
  }

  if (fieldStarted || field !== "" || row.length > 0) {
    row.push(field.trim());
    rows.push(row);
  }

  return rows;
}

function normalizeHeaderKey(header: string): string {
  return header.replace(/\s+/g, "").toLowerCase();
}

function findColumnIndex(headers: string[], aliases: string[]): number | null {
  const normalizedAliases = aliases.map(normalizeHeaderKey);
  const idx = headers.findIndex((h) =>
    normalizedAliases.includes(normalizeHeaderKey(h)),
  );
  return idx >= 0 ? idx : null;
}

function normalizeCalendarDateText(raw: string): { ok: true; date: string } | { ok: false; error: string } {
  const trimmed = raw.trim();
  if (!trimmed) {
    return { ok: false, error: "수입일자가 비어 있습니다." };
  }
  const datePart = trimmed.split(/[T ]/)[0] ?? trimmed;
  let candidate = datePart;
  if (/^\d{8}$/.test(datePart)) {
    candidate = `${datePart.slice(0, 4)}-${datePart.slice(4, 6)}-${datePart.slice(6, 8)}`;
  } else {
    const parts = datePart.replaceAll("/", "-").replaceAll(".", "-").split("-");
    if (
      parts.length === 3 &&
      /^\d{4}$/.test(parts[0]) &&
      /^\d{1,2}$/.test(parts[1]) &&
      /^\d{1,2}$/.test(parts[2])
    ) {
      candidate = `${parts[0]}-${parts[1].padStart(2, "0")}-${parts[2].padStart(2, "0")}`;
    }
  }

    if (!/^\d{4}-\d{2}-\d{2}$/.test(candidate)) {
      return { ok: false, error: `수입일자는 YYYY-MM-DD 형식이어야 합니다: '${raw}'` };
    }
    const validated = validateIsoCalendarDate(candidate, 1900, 2100);
    if (!validated) {
      return { ok: false, error: `존재하지 않는 날짜이거나 허용 연도(1900~2100) 범위 밖입니다: '${raw}'` };
    }
    return { ok: true, date: validated };
}

export function parseCsvImportPreview(
  kind: ImportKind,
  filePath: string,
  csvText: string,
  ctx: StandalonePreviewContext,
): ImportPreviewResult {
  const rawRows = parseCsvRecords(csvText);
  const headerIndex = rawRows.findIndex((r) => r.some((c) => c.trim().length > 0));
  if (headerIndex < 0) {
    throw new Error("업로드한 파일이 비어 있습니다.");
  }
  const headers = rawRows[headerIndex];
  const dataRows = rawRows
    .map((cells, idx) => ({ rowNumber: idx + 1, cells }))
    .slice(headerIndex + 1)
    .filter((r) => r.cells.some((c) => c.trim().length > 0));

  if (dataRows.length === 0) {
    throw new Error("헤더 외에 가져올 데이터 행이 없습니다.");
  }

  const fileName = filePath.split(/[\\/]/).pop() ?? filePath;

  if (kind === "bom") {
    return previewBomCsvRows(filePath, fileName, headers, dataRows, ctx);
  }
  return previewShipmentCsvRows(filePath, fileName, headers, dataRows, ctx);
}

function previewBomCsvRows(
  filePath: string,
  fileName: string,
  headers: string[],
  dataRows: Array<{ rowNumber: number; cells: string[] }>,
  ctx: StandalonePreviewContext,
): ImportPreviewResult {
  const colBranch = findColumnIndex(headers, ["해외지사"]);
  const colCode = findColumnIndex(headers, ["수입제품코드"]);
  const colProdName = findColumnIndex(headers, ["제품명(선택)", "제품명"]);
  const colCas = findColumnIndex(headers, ["cas번호", "CAS번호"]);
  const colSubName = findColumnIndex(headers, ["화학물질명(선택)", "화학물질명"]);
  const colPct = findColumnIndex(headers, ["함유율(%)", "함유율"]);

  const missing: string[] = [];
  if (colBranch === null) missing.push("해외지사");
  if (colCode === null) missing.push("수입제품코드");
  if (colCas === null) missing.push("CAS번호");
  if (colPct === null) missing.push("함유율(%)");
  if (missing.length > 0) {
    throw new Error(`제품 구성성분(BOM) 필수 열이 누락되었습니다: ${missing.join(", ")}`);
  }

  const existingCodes = new Set(ctx.existingProductCodes.map((c) => c.trim().toLowerCase()));
  const groupOrder: string[] = [];
  const groupIndices = new Map<string, number[]>();

  const rows: ImportPreviewRow[] = dataRows.map((raw, idx) => {
    const branch = (raw.cells[colBranch!] ?? "").trim();
    const productCode = (raw.cells[colCode!] ?? "").trim();
    const productName = colProdName !== null ? (raw.cells[colProdName] ?? "").trim() || null : null;
    const cas = normalizeCasNumber(raw.cells[colCas!] ?? "");
    const substanceName = colSubName !== null ? (raw.cells[colSubName] ?? "").trim() || null : null;
    const pctRaw = (raw.cells[colPct!] ?? "").trim();

    const errors: string[] = [];
    if (!branch) errors.push("해외지사가 비어 있습니다.");
    if (!productCode) errors.push("수입제품코드가 비어 있습니다.");
    if (!cas) errors.push("CAS번호가 비어 있습니다.");

    let weightPctText: string | null = null;
    let weightPctScaled: number | null = null;
    if (!pctRaw) {
      errors.push("함유율(%)이 비어 있습니다.");
    } else {
      const parsed = parseScaled4dp(pctRaw);
      if (!parsed.ok) {
        errors.push(`함유율(%) 오류: ${parsed.error}`);
      } else if (parsed.scaled <= 0 || parsed.scaled > HUNDRED_PERCENT_SCALED) {
        errors.push(`함유율(%)은 0보다 크고 100 이하이어야 합니다: '${pctRaw}'`);
      } else {
        weightPctText = parsed.canonical;
        weightPctScaled = parsed.scaled;
      }
    }

    if (productCode) {
      const key = productCode.toLowerCase();
      const list = groupIndices.get(key);
      if (list) {
        list.push(idx);
      } else {
        groupOrder.push(key);
        groupIndices.set(key, [idx]);
      }
    }

    return {
      row_number: raw.rowNumber,
      status: errors.length > 0 ? "error" : "valid",
      is_conflict: false,
      conflict_reason: null,
      errors,
      overseas_branch_name: branch,
      product_code: productCode,
      product_name: productName,
      cas_number: cas || null,
      substance_name: substanceName,
      weight_pct_text: weightPctText,
      weight_pct_scaled: weightPctScaled,
      shipment_date: null,
      domestic_importer_name: null,
      import_weight_kg_text: null,
      import_weight_kg_scaled: null,
      note: null,
    };
  });

  const bomProducts: BomProductSummary[] = [];
  let validCount = 0;
  let conflictCount = 0;

  for (const key of groupOrder) {
    const indices = groupIndices.get(key) ?? [];
    const canonicalCode = rows[indices[0]].product_code;
    const groupErrors: string[] = [];
    let branchCanonical: string | null = null;
    let nameCanonical: string | null = null;
    const casMap = new Map<string, number[]>();
    let allPctOk = true;
    let sumScaled = 0;

    for (const idx of indices) {
      const r = rows[idx];
      if (r.overseas_branch_name) {
        if (!branchCanonical) branchCanonical = r.overseas_branch_name;
        else if (branchCanonical.toLowerCase() !== r.overseas_branch_name.toLowerCase()) {
          const msg = `수입제품코드 '${canonicalCode}'에 서로 다른 해외지사('${branchCanonical}' vs '${r.overseas_branch_name}')가 지정되었습니다.`;
          if (!groupErrors.includes(msg)) groupErrors.push(msg);
        }
      }
      if (r.product_name) {
        if (!nameCanonical) nameCanonical = r.product_name;
        else if (nameCanonical !== r.product_name) {
          const msg = `수입제품코드 '${canonicalCode}'에 서로 다른 제품명('${nameCanonical}' vs '${r.product_name}')이 입력되었습니다.`;
          if (!groupErrors.includes(msg)) groupErrors.push(msg);
        }
      }
      if (r.cas_number) {
        const list = casMap.get(r.cas_number) ?? [];
        list.push(idx);
        casMap.set(r.cas_number, list);
      }
      if (r.weight_pct_scaled !== null) {
        sumScaled += r.weight_pct_scaled;
      } else {
        allPctOk = false;
      }
    }

    for (const [cas, posList] of casMap.entries()) {
      if (posList.length > 1) {
        const msg = `동일한 제품 '${canonicalCode}' 내에 중복된 CAS번호가 있습니다: '${cas}'`;
        groupErrors.push(msg);
        for (const idx of posList) {
          if (!rows[idx].errors.includes(msg)) rows[idx].errors.push(msg);
        }
      }
    }

    if (allPctOk && sumScaled !== HUNDRED_PERCENT_SCALED) {
      const sumStr = formatPct(sumScaled);
      const diffScaled = sumScaled - HUNDRED_PERCENT_SCALED;
      const diffStr =
        diffScaled > 0 ? `+${formatPct(diffScaled)}` : `-${formatPct(-diffScaled)}`;
      const msg = `제품 '${canonicalCode}'의 함유율 합계가 ${sumStr}%입니다. 정확히 100%여야 합니다 (차이: ${diffStr}%).`;
      groupErrors.push(msg);
      for (const idx of indices) {
        rows[idx].errors.push(msg);
      }
    }

    const anyRowErr = indices.some((idx) => rows[idx].errors.length > 0);
    for (const idx of indices) {
      for (const gErr of groupErrors) {
        if (!rows[idx].errors.includes(gErr)) rows[idx].errors.push(gErr);
      }
      if (anyRowErr && rows[idx].errors.length === 0) {
        rows[idx].errors.push(`동일 제품 '${canonicalCode}'의 다른 행에 오류가 있어 가져올 수 없습니다.`);
      }
    }

    const existsInDb = existingCodes.has(key);
    let status: PreviewRowStatus;
    if (anyRowErr || groupErrors.length > 0) {
      status = "error";
    } else if (existsInDb) {
      status = "conflict";
      conflictCount++;
    } else {
      status = "valid";
      validCount++;
    }

    for (const idx of indices) {
      rows[idx].status = status;
      if (status === "conflict") {
        rows[idx].is_conflict = true;
        rows[idx].conflict_reason = "기존 DB에 동일한 수입제품코드가 존재합니다 (덮어쓰기/건너뛰기 대상).";
      }
    }

    const summaryErrors = Array.from(
      new Set([...groupErrors, ...indices.flatMap((idx) => rows[idx].errors)]),
    );
    bomProducts.push({
      product_code: canonicalCode,
      product_name: nameCanonical,
      overseas_branch_name: branchCanonical ?? "",
      row_numbers: indices.map((idx) => rows[idx].row_number),
      component_count: indices.length,
      weight_pct_sum_scaled: sumScaled,
      weight_pct_sum_display: formatPct(sumScaled),
      exists_in_db: existsInDb,
      status,
      errors: summaryErrors,
    });
  }

  const errorCount = rows.filter((r) => r.status === "error").length;
  return {
    kind: "bom",
    file_path: filePath,
    file_name: fileName,
    total_rows: rows.length,
    total_products: bomProducts.length,
    valid_count: validCount,
    conflict_count: conflictCount,
    error_count: errorCount,
    can_commit: errorCount === 0 && rows.length > 0,
    unknown_product_codes: [],
    bom_products: bomProducts,
    rows,
  };
}

function previewShipmentCsvRows(
  filePath: string,
  fileName: string,
  headers: string[],
  dataRows: Array<{ rowNumber: number; cells: string[] }>,
  ctx: StandalonePreviewContext,
): ImportPreviewResult {
  const colDate = findColumnIndex(headers, ["수입일자(YYYY-MM-DD)", "수입일자"]);
  const colImporter = findColumnIndex(headers, ["국내 수입사", "국내수입사"]);
  const colBranch = findColumnIndex(headers, ["해외지사"]);
  const colCode = findColumnIndex(headers, ["수입제품코드"]);
  const colWeight = findColumnIndex(headers, ["제품 수입량(kg)", "제품수입량(kg)", "제품 수입량"]);
  const colNote = findColumnIndex(headers, ["비고(선택)", "비고"]);

  const missing: string[] = [];
  if (colDate === null) missing.push("수입일자(YYYY-MM-DD)");
  if (colImporter === null) missing.push("국내 수입사");
  if (colBranch === null) missing.push("해외지사");
  if (colCode === null) missing.push("수입제품코드");
  if (colWeight === null) missing.push("제품 수입량(kg)");
  if (missing.length > 0) {
    throw new Error(`수입실적 필수 열이 누락되었습니다: ${missing.join(", ")}`);
  }

  const knownCodes = new Map<string, string>();
  for (const code of ctx.existingProductCodes) {
    knownCodes.set(code.trim().toLowerCase(), code.trim());
  }
  const branchByCode = new Map<string, string>();
  for (const [k, v] of Object.entries(ctx.productBranches ?? {})) {
    branchByCode.set(k.trim().toLowerCase(), v.trim());
  }
  const nameByCode = new Map<string, string | null>();
  for (const [k, v] of Object.entries(ctx.productNames ?? {})) {
    nameByCode.set(k.trim().toLowerCase(), v);
  }

  const makeKey = (
    date: string,
    imp: string,
    br: string,
    code: string,
    scaled: number,
  ) =>
    `${date}|${imp.trim().toLowerCase()}|${br.trim().toLowerCase()}|${code.trim().toLowerCase()}|${scaled}`;

  const dbDupSet = new Set(
    ctx.existingShipments.map((s) =>
      makeKey(
        s.shipment_date,
        s.domestic_importer_name,
        s.overseas_branch_name,
        s.product_code,
        s.import_weight_kg_scaled,
      ),
    ),
  );
  const batchSeen = new Map<string, number>();
  const unknownCodes: string[] = [];
  const unknownSeen = new Set<string>();
  let validCount = 0;
  let conflictCount = 0;

  const rows: ImportPreviewRow[] = dataRows.map((raw) => {
    const dateRaw = (raw.cells[colDate!] ?? "").trim();
    const importer = (raw.cells[colImporter!] ?? "").trim();
    const branch = (raw.cells[colBranch!] ?? "").trim();
    const productCode = (raw.cells[colCode!] ?? "").trim();
    const weightRaw = (raw.cells[colWeight!] ?? "").trim();
    const note = colNote !== null ? (raw.cells[colNote] ?? "").trim() || null : null;

    const errors: string[] = [];
    const dateRes = normalizeCalendarDateText(dateRaw);
    if (!dateRes.ok) errors.push(dateRes.error);

    if (!importer) errors.push("국내 수입사가 비어 있습니다.");
    if (!branch) errors.push("해외지사가 비어 있습니다.");

    let effectiveCode = productCode;
    let effectiveBranch = branch;
    let productName: string | null = null;

    if (!productCode) {
      errors.push("수입제품코드가 비어 있습니다.");
    } else {
      const codeKey = productCode.toLowerCase();
      const canonical = knownCodes.get(codeKey);
      if (!canonical) {
        if (!unknownSeen.has(codeKey)) {
          unknownSeen.add(codeKey);
          unknownCodes.push(productCode);
        }
        errors.push(`등록되지 않은 수입제품코드입니다: '${productCode}'. 먼저 '제품 구성성분(BOM)'을 등록하거나 가져와 주세요.`);
      } else {
        effectiveCode = canonical;
        productName = nameByCode.get(codeKey) ?? null;
        const dbBranch = branchByCode.get(codeKey);
        if (dbBranch && branch && branch.toLowerCase() !== dbBranch.toLowerCase()) {
          errors.push(`수입제품코드 '${canonical}'의 등록된 해외지사('${dbBranch}')와 일치하지 않습니다 (입력값: '${branch}').`);
        } else if (dbBranch && branch) {
          effectiveBranch = dbBranch;
        }
      }
    }

    let weightText: string | null = null;
    let weightScaled: number | null = null;
    if (!weightRaw) {
      errors.push("제품 수입량(kg)이 비어 있습니다.");
    } else {
      const parsed = parseScaled4dp(weightRaw);
      if (!parsed.ok) {
        errors.push(`제품 수입량(kg) 오류: ${parsed.error}`);
      } else if (parsed.scaled <= 0) {
        errors.push(`제품 수입량(kg)은 0보다 커야 합니다: '${weightRaw}'`);
      } else {
        weightText = parsed.canonical;
        weightScaled = parsed.scaled;
      }
    }

    let isConflict = false;
    let conflictReason: string | null = null;
    let status: PreviewRowStatus;
    if (errors.length > 0) {
      status = "error";
    } else {
      const key = makeKey(
        dateRes.ok ? dateRes.date : "",
        importer,
        effectiveBranch,
        effectiveCode,
        weightScaled ?? 0,
      );
      if (dbDupSet.has(key)) {
        status = "conflict";
        isConflict = true;
        conflictCount++;
        conflictReason = "기존 DB에 5개 항목이 동일한 수입실적이 이미 존재합니다.";
      } else if (batchSeen.has(key)) {
        status = "conflict";
        isConflict = true;
        conflictCount++;
        conflictReason = `업로드 파일 내 중복 의심 건입니다 (${batchSeen.get(key)}행과 5개 항목 동일).`;
      } else {
        batchSeen.set(key, raw.rowNumber);
        status = "valid";
        validCount++;
      }
    }

    return {
      row_number: raw.rowNumber,
      status,
      is_conflict: isConflict,
      conflict_reason: conflictReason,
      errors,
      overseas_branch_name: effectiveBranch,
      product_code: effectiveCode,
      product_name: productName,
      cas_number: null,
      substance_name: null,
      weight_pct_text: null,
      weight_pct_scaled: null,
      shipment_date: dateRes.ok ? dateRes.date : dateRaw || null,
      domestic_importer_name: importer || null,
      import_weight_kg_text: weightText ?? (weightRaw || null),
      import_weight_kg_scaled: weightScaled,
      note,
    };
  });

  const errorCount = rows.filter((r) => r.status === "error").length;
  return {
    kind: "shipments",
    file_path: filePath,
    file_name: fileName,
    total_rows: rows.length,
    total_products: 0,
    valid_count: validCount,
    conflict_count: conflictCount,
    error_count: errorCount,
    can_commit: errorCount === 0 && rows.length > 0,
    unknown_product_codes: unknownCodes,
    bom_products: [],
    rows,
  };
}

export function createExcelImportClient(
  invokeOverride?: TauriInvoke,
): ExcelImportClient {
  const invoke = invokeOverride ?? detectTauriInvoke();
  if (invoke) {
    return {
      isTauriRuntime: true,
      async pickTemplateSavePath(kind: ImportKind): Promise<string | null> {
        const defaultPath = defaultTemplateFilename(kind);
        const picked = await invoke<string | null>("plugin:dialog|save", {
          options: {
            title: "가져오기 양식 저장 (.xlsx)",
            defaultPath,
            filters: [{ name: "Excel Workbook", extensions: ["xlsx"] }],
          },
        });
        return picked && picked.trim().length > 0 ? picked : null;
      },
      downloadImportTemplate(
        kind: ImportKind,
        outputPath: string,
      ): Promise<TemplateDownloadResult> {
        return invoke<TemplateDownloadResult>("download_import_template", {
          kind,
          outputPath,
        });
      },
      async pickImportFile(): Promise<string | null> {
        const picked = await invoke<string | string[] | null>(
          "plugin:dialog|open",
          {
            options: {
              title: "가져올 엑셀/CSV 파일 선택",
              multiple: false,
              directory: false,
              filters: [
                { name: "Excel / CSV", extensions: ["xlsx", "xls", "csv"] },
              ],
            },
          },
        );
        if (Array.isArray(picked)) return picked[0] ?? null;
        return picked && picked.trim().length > 0 ? picked : null;
      },
      previewExcelImport(
        kind: ImportKind,
        filePath: string,
      ): Promise<ImportPreviewResult> {
        return invoke<ImportPreviewResult>("preview_excel_import", {
          kind,
          filePath,
        });
      },
      commitExcelImport(
        kind: ImportKind,
        filePath: string,
        options: ImportOptions,
      ): Promise<ImportCommitResult> {
        return invoke<ImportCommitResult>("commit_excel_import", {
          kind,
          filePath,
          options,
        });
      },
    };
  }

  // Standalone browser / offline unit-test fallback
  const inlineCsvFiles = new Map<string, string>();
  const productBranches: Record<string, string> = {
    "PRD-EU-101": "유럽지사 (독일)",
    "PRD-EU-204": "유럽지사 (독일)",
    "PRD-US-305": "북미지사 (미국)",
  };
  const productNames: Record<string, string | null> = {
    "PRD-EU-101": "코팅솔벤트 A-100",
    "PRD-EU-204": "고기능 세정제 B-200",
    "PRD-US-305": "표면처리제 C-300",
  };
  const existingShipments: ExistingShipmentKeyTuple[] = [
    {
      shipment_date: "2026-01-15",
      domestic_importer_name: "(주)대한케미칼",
      overseas_branch_name: "유럽지사 (독일)",
      product_code: "PRD-EU-101",
      import_weight_kg_scaled: 24_000_000,
    },
  ];

  const sampleBomCsv =
    "해외지사,수입제품코드,제품명(선택),CAS번호,화학물질명(선택),함유율(%)\n" +
    "유럽지사 (독일),PRD-SAMPLE-01,샘플 코팅제,50-00-0,포름알데히드,35.25\n" +
    "유럽지사 (독일),PRD-SAMPLE-01,샘플 코팅제,7732-18-5,정제수,64.75\n";
  const sampleShipCsv =
    "수입일자(YYYY-MM-DD),국내 수입사,해외지사,수입제품코드,제품 수입량(kg),비고(선택)\n" +
    "2026-03-15,(주)대한케미칼,유럽지사 (독일),PRD-EU-101,1250.5,예시 데이터\n";

  return {
    isTauriRuntime: false,
    registerInlineCsvFile(fileName: string, csvText: string): string {
      inlineCsvFiles.set(fileName, csvText);
      return fileName;
    },
    async pickTemplateSavePath(kind: ImportKind): Promise<string | null> {
      return defaultTemplateFilename(kind);
    },
    async downloadImportTemplate(
      kind: ImportKind,
      outputPath: string,
    ): Promise<TemplateDownloadResult> {
      return {
        kind,
        output_path: outputPath,
        sheet_name: kind === "bom" ? "제품구성성분_BOM" : "수입실적",
      };
    },
    async pickImportFile(): Promise<string | null> {
      return "sample_import.csv";
    },
    async previewExcelImport(
      kind: ImportKind,
      filePath: string,
    ): Promise<ImportPreviewResult> {
      const csvText =
        inlineCsvFiles.get(filePath) ??
        (kind === "bom" ? sampleBomCsv : sampleShipCsv);
      return parseCsvImportPreview(kind, filePath, csvText, {
        existingProductCodes: Object.keys(productBranches),
        productBranches,
        productNames,
        existingShipments,
      });
    },
    async commitExcelImport(
      kind: ImportKind,
      filePath: string,
      options: ImportOptions,
    ): Promise<ImportCommitResult> {
      const preview = await this.previewExcelImport(kind, filePath);
      if (!preview.can_commit) {
        throw new Error(
          `검증 오류가 ${preview.error_count}건 있어 일괄 가져오기를 실행할 수 없습니다.`,
        );
      }
      if (kind === "bom") {
        let inserted = 0;
        let updated = 0;
        let skipped = 0;
        for (const p of preview.bom_products) {
          const exists = Object.keys(productBranches).some(
            (c) => c.toLowerCase() === p.product_code.toLowerCase(),
          );
          if (exists) {
            if (options.overwrite_existing) {
              productBranches[p.product_code] = p.overseas_branch_name;
              productNames[p.product_code] = p.product_name;
              updated++;
            } else {
              skipped++;
            }
          } else {
            productBranches[p.product_code] = p.overseas_branch_name;
            productNames[p.product_code] = p.product_name;
            inserted++;
          }
        }
        return {
          kind,
          inserted_count: inserted,
          updated_count: updated,
          skipped_count: skipped,
          total_processed: inserted + updated + skipped,
          message: `제품 구성성분(BOM) 가져오기 완료: 신규 등록 ${inserted}개, 덮어쓰기 ${updated}개, 건너뛰기 ${skipped}개`,
        };
      }

      let inserted = 0;
      let skipped = 0;
      for (const r of preview.rows) {
        if (r.is_conflict && options.skip_duplicates) {
          skipped++;
          continue;
        }
        existingShipments.push({
          shipment_date: r.shipment_date ?? "",
          domestic_importer_name: r.domestic_importer_name ?? "",
          overseas_branch_name: r.overseas_branch_name,
          product_code: r.product_code,
          import_weight_kg_scaled: r.import_weight_kg_scaled ?? 0,
        });
        inserted++;
      }
      return {
        kind,
        inserted_count: inserted,
        updated_count: 0,
        skipped_count: skipped,
        total_processed: inserted + skipped,
        message: `수입실적 일괄 가져오기 완료: 신규 추가 ${inserted}건, 중복 건너뛰기 ${skipped}건`,
      };
    },
  };
}
