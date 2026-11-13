import {
  formatKg,
  formatTonFromKgScaled,
  parseScaled4dp,
} from "../lib/decimal";

export interface OverseasBranchOption {
  id: number;
  name: string;
}

export interface DomesticImporterOption {
  id: number;
  name: string;
  note: string | null;
}

export interface ProductOption {
  product_id: number;
  product_code: string;
  product_name: string | null;
  overseas_branch_id: number;
  overseas_branch_name: string;
}

export interface SaveShipmentInput {
  id: number | null;
  shipment_date: string;
  domestic_importer_name: string;
  product_id: number;
  import_weight_kg: string;
  note: string | null;
}

export interface ShipmentRecord {
  id: number;
  shipment_date: string;
  shipment_year: number;
  domestic_importer_id: number;
  domestic_importer_name: string;
  overseas_branch_id: number;
  overseas_branch_name: string;
  product_id: number;
  product_code: string;
  product_name: string | null;
  import_weight_kg_text: string;
  import_weight_kg_scaled: number;
  import_weight_kg_display: string;
  import_weight_ton_display: string;
  note: string | null;
}

export interface ShipmentFormState {
  editingId: number | null;
  shipmentDate: string;
  domesticImporterName: string;
  productId: number | null;
  importWeightKgInput: string;
  note: string;
}

export interface ShipmentFilter {
  year: number | null;
  domestic_importer_id: number | null;
  overseas_branch_id: number | null;
  search_text: string;
}

export interface ShipmentSummary {
  total_count: number;
  total_weight_kg_text: string;
  total_weight_ton_text: string;
}

export interface VirtualWindowInput {
  count: number;
  estimateSize: number;
  overscan?: number;
  scrollOffset: number;
  viewportHeight: number;
}

export interface VirtualRowItem {
  index: number;
  key: number;
  start: number;
  size: number;
  end: number;
}

export interface VirtualWindowResult {
  totalSize: number;
  startIndex: number;
  endIndex: number;
  virtualItems: VirtualRowItem[];
}

export function filterShipments(
  rows: ShipmentRecord[],
  filter: ShipmentFilter,
): ShipmentRecord[] {
  const search = filter.search_text.trim().toLowerCase();
  return rows.filter((row) => {
    if (filter.year !== null && row.shipment_year !== filter.year) {
      return false;
    }
    if (
      filter.domestic_importer_id !== null &&
      row.domestic_importer_id !== filter.domestic_importer_id
    ) {
      return false;
    }
    if (
      filter.overseas_branch_id !== null &&
      row.overseas_branch_id !== filter.overseas_branch_id
    ) {
      return false;
    }
    if (search.length > 0) {
      const codeMatch = row.product_code.toLowerCase().includes(search);
      const nameMatch = (row.product_name ?? "").toLowerCase().includes(search);
      if (!codeMatch && !nameMatch) return false;
    }
    return true;
  });
}

export function summarizeShipments(rows: ShipmentRecord[]): ShipmentSummary {
  let totalScaled = 0n;
  for (const row of rows) {
    totalScaled += BigInt(row.import_weight_kg_scaled);
  }
  const scaledNum = Number(totalScaled);
  return {
    total_count: rows.length,
    total_weight_kg_text: formatKg(scaledNum),
    total_weight_ton_text: formatTonFromKgScaled(scaledNum),
  };
}

/**
 * `@tanstack/react-virtual`-compatible fixed-height virtual window calculator.
 * Computes only the visible slice plus `overscan` buffer rows (~30-50 DOM nodes).
 */
export function computeVirtualWindow(
  input: VirtualWindowInput,
): VirtualWindowResult {
  const { count, estimateSize, viewportHeight } = input;
  const overscan = input.overscan ?? 8;
  if (count <= 0 || estimateSize <= 0) {
    return { totalSize: 0, startIndex: 0, endIndex: -1, virtualItems: [] };
  }

  const totalSize = count * estimateSize;
  const maxScroll = Math.max(0, totalSize - Math.max(0, viewportHeight));
  const clampedScroll = Math.min(Math.max(0, input.scrollOffset), maxScroll);
  const effectiveHeight = Math.max(estimateSize, viewportHeight);

  const visibleStart = Math.floor(clampedScroll / estimateSize);
  const visibleEnd = Math.min(
    count - 1,
    Math.ceil((clampedScroll + effectiveHeight) / estimateSize) - 1,
  );

  const startIndex = Math.max(0, visibleStart - overscan);
  const endIndex = Math.min(count - 1, visibleEnd + overscan);

  const virtualItems: VirtualRowItem[] = [];
  for (let i = startIndex; i <= endIndex; i++) {
    const start = i * estimateSize;
    virtualItems.push({
      index: i,
      key: i,
      start,
      size: estimateSize,
      end: start + estimateSize,
    });
  }

  return { totalSize, startIndex, endIndex, virtualItems };
}

export interface LiveWeightPreview {
  valid: boolean;
  kgText: string;
  tonText: string;
  error: string | null;
}

export function todayIsoDate(): string {
  const now = new Date();
  const y = now.getFullYear();
  const m = String(now.getMonth() + 1).padStart(2, "0");
  const d = String(now.getDate()).padStart(2, "0");
  return `${y}-${m}-${d}`;
}

export function createInitialShipmentForm(
  defaultDate = todayIsoDate(),
): ShipmentFormState {
  return {
    editingId: null,
    shipmentDate: defaultDate,
    domesticImporterName: "",
    productId: null,
    importWeightKgInput: "",
    note: "",
  };
}

export function loadShipmentIntoForm(row: ShipmentRecord): ShipmentFormState {
  return {
    editingId: row.id,
    shipmentDate: row.shipment_date,
    domesticImporterName: row.domestic_importer_name,
    productId: row.product_id,
    importWeightKgInput: row.import_weight_kg_text,
    note: row.note ?? "",
  };
}

export function resolveSelectedProduct(
  products: ProductOption[],
  productId: number | null,
): ProductOption | null {
  if (productId == null) return null;
  return products.find((p) => p.product_id === productId) ?? null;
}

export interface ShipmentQueryResult {
  rows: ShipmentRecord[];
  summary: ShipmentSummary;
  available_years: number[];
}

export interface ShipmentsClient {
  listDomesticImporters(): Promise<DomesticImporterOption[]>;
  listProductOptions(): Promise<ProductOption[]>;
  queryShipments(filter: ShipmentFilter): Promise<ShipmentQueryResult>;
  saveShipment(input: SaveShipmentInput): Promise<ShipmentRecord>;
  deleteShipment(shipmentId: number): Promise<void>;
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

export function createShipmentsClient(
  invokeOverride?: TauriInvoke,
): ShipmentsClient {
  const invoke = invokeOverride ?? detectTauriInvoke();
  if (invoke) {
    return {
      listDomesticImporters: () =>
        invoke<DomesticImporterOption[]>("list_domestic_importers"),
      listProductOptions: () =>
        invoke<ProductOption[]>("list_product_options"),
      queryShipments: (filter) =>
        invoke<ShipmentQueryResult>("query_shipments", { filter }),
      saveShipment: (input) =>
        invoke<ShipmentRecord>("save_shipment", { input }),
      deleteShipment: (shipmentId) =>
        invoke<void>("delete_shipment", { shipmentId }),
    };
  }

  // Standalone browser / offline unit-test fallback matching db.rs seed dataset
  const importers: DomesticImporterOption[] = [
    { id: 1, name: "(주)대한케미칼", note: "서울 본사 / 연간 보고 대상" },
    { id: 2, name: "한성정밀화학(주)", note: "울산 공장" },
    { id: 3, name: "세진소재산업(주)", note: null },
  ];
  const products: ProductOption[] = [
    {
      product_id: 1,
      product_code: "PRD-EU-101",
      product_name: "코팅솔벤트 A-100",
      overseas_branch_id: 1,
      overseas_branch_name: "유럽지사 (독일)",
    },
    {
      product_id: 2,
      product_code: "PRD-EU-204",
      product_name: "고기능 세정제 B-200",
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

  const makeRow = (
    id: number,
    date: string,
    importerId: number,
    productId: number,
    weightStr: string,
    note: string | null,
  ): ShipmentRecord => {
    const importer = importers.find((i) => i.id === importerId)!;
    const product = products.find((p) => p.product_id === productId)!;
    const parsed = parseScaled4dp(weightStr);
    if (!parsed.ok || parsed.scaled <= 0) {
      throw new Error("제품 수입량(kg)은 0보다 커야 합니다.");
    }
    return {
      id,
      shipment_date: date,
      shipment_year: Number(date.slice(0, 4)),
      domestic_importer_id: importer.id,
      domestic_importer_name: importer.name,
      overseas_branch_id: product.overseas_branch_id,
      overseas_branch_name: product.overseas_branch_name,
      product_id: product.product_id,
      product_code: product.product_code,
      product_name: product.product_name,
      import_weight_kg_text: parsed.canonical,
      import_weight_kg_scaled: parsed.scaled,
      import_weight_kg_display: formatKg(parsed.scaled),
      import_weight_ton_display: formatTonFromKgScaled(parsed.scaled),
      note,
    };
  };

  let nextImporterId = 4;
  let nextShipmentId = 7;
  let rows: ShipmentRecord[] = [
    makeRow(1, "2026-01-15", 1, 1, "2400", "1분기 정기 입고"),
    makeRow(2, "2026-03-10", 1, 2, "1500.5", null),
    makeRow(3, "2026-05-22", 1, 3, "3200", "상반기 추가 물량"),
    makeRow(4, "2026-08-04", 1, 1, "1100.25", null),
    makeRow(5, "2026-04-18", 2, 2, "4000", "울산 공장 직송"),
    makeRow(6, "2025-11-12", 1, 1, "1800", "전년도 비교용 실적"),
  ];

  const sortRows = (list: ShipmentRecord[]) =>
    [...list].sort((a, b) => {
      if (a.shipment_date !== b.shipment_date) {
        return a.shipment_date < b.shipment_date ? 1 : -1;
      }
      return b.id - a.id;
    });

  return {
    async listDomesticImporters() {
      return [...importers].sort((a, b) => a.name.localeCompare(b.name, "ko"));
    },
    async listProductOptions() {
      return [...products].sort((a, b) =>
        a.product_code.localeCompare(b.product_code),
      );
    },
    async queryShipments(filter) {
      const matched = sortRows(filterShipments(rows, filter));
      const years = Array.from(new Set(rows.map((r) => r.shipment_year))).sort(
        (a, b) => b - a,
      );
      return {
        rows: matched,
        summary: summarizeShipments(matched),
        available_years: years,
      };
    },
    async saveShipment(input) {
      const validDate = validateIsoCalendarDate(input.shipment_date);
      if (!validDate) {
        throw new Error("수입일자는 YYYY-MM-DD 형식이어야 합니다.");
      }
      const importerName = input.domestic_importer_name.trim();
      if (!importerName) {
        throw new Error("국내 수입사를 입력해 주세요.");
      }
      let importer = importers.find(
        (i) => i.name.toLowerCase() === importerName.toLowerCase(),
      );
      if (!importer) {
        importer = { id: nextImporterId++, name: importerName, note: null };
        importers.push(importer);
      }
      const product = products.find((p) => p.product_id === input.product_id);
      if (!product) {
        throw new Error("선택한 수입제품이 존재하지 않습니다.");
      }

      const trimmedNote = input.note?.trim() || null;
      if (input.id != null) {
        const idx = rows.findIndex((r) => r.id === input.id);
        if (idx === -1) {
          throw new Error(`수정할 수입실적(ID: ${input.id})을 찾을 수 없습니다.`);
        }
        const updated = makeRow(
          input.id,
          validDate,
          importer.id,
          product.product_id,
          input.import_weight_kg,
          trimmedNote,
        );
        rows[idx] = updated;
        return updated;
      }

      const created = makeRow(
        nextShipmentId++,
        validDate,
        importer.id,
        product.product_id,
        input.import_weight_kg,
        trimmedNote,
      );
      rows.push(created);
      return created;
    },
    async deleteShipment(shipmentId) {
      const beforeLen = rows.length;
      rows = rows.filter((r) => r.id !== shipmentId);
      if (rows.length === beforeLen) {
        throw new Error(`삭제할 수입실적(ID: ${shipmentId})을 찾을 수 없습니다.`);
      }
    },
  };
}

export function validateIsoCalendarDate(raw: string): string | null {
  const trimmed = raw.trim();
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(trimmed);
  if (!match) return null;
  const year = Number(match[1]);
  const month = Number(match[2]);
  const day = Number(match[3]);
  if (year < 1900 || year > 2100 || month < 1 || month > 12) return null;

  const isLeap = (year % 4 === 0 && year % 100 !== 0) || year % 400 === 0;
  const maxDays =
    month === 2
      ? isLeap
        ? 29
        : 28
      : [4, 6, 9, 11].includes(month)
        ? 30
        : 31;

  if (day < 1 || day > maxDays) return null;
  return trimmed;
}

export function computeLiveWeightPreview(weightInput: string): LiveWeightPreview {
  const parsed = parseScaled4dp(weightInput);
  if (!parsed.ok) {
    return {
      valid: false,
      kgText: "-",
      tonText: "-",
      error: parsed.error,
    };
  }
  if (parsed.scaled <= 0) {
    return {
      valid: false,
      kgText: "-",
      tonText: "-",
      error: "제품 수입량(kg)은 0보다 커야 합니다.",
    };
  }
  return {
    valid: true,
    kgText: formatKg(parsed.scaled),
    tonText: formatTonFromKgScaled(parsed.scaled),
    error: null,
  };
}

export function validateShipmentForm(
  form: ShipmentFormState,
  products: ProductOption[],
): { ok: true; payload: SaveShipmentInput } | { ok: false; error: string } {
  const validDate = validateIsoCalendarDate(form.shipmentDate);
  if (!validDate) {
    return {
      ok: false,
      error: "수입일자는 올바른 YYYY-MM-DD 형식의 날짜여야 합니다.",
    };
  }

  const importerName = form.domesticImporterName.trim();
  if (!importerName) {
    return { ok: false, error: "국내 수입사를 입력하거나 선택해 주세요." };
  }

  const product = resolveSelectedProduct(products, form.productId);
  if (!product) {
    return { ok: false, error: "수입제품코드를 선택해 주세요." };
  }

  const parsedWeight = parseScaled4dp(form.importWeightKgInput);
  if (!parsedWeight.ok) {
    return { ok: false, error: parsedWeight.error };
  }
  if (parsedWeight.scaled <= 0) {
    return { ok: false, error: "제품 수입량(kg)은 0보다 커야 합니다." };
  }

  const trimmedNote = form.note.trim();
  return {
    ok: true,
    payload: {
      id: form.editingId,
      shipment_date: validDate,
      domestic_importer_name: importerName,
      product_id: product.product_id,
      import_weight_kg: parsedWeight.canonical,
      note: trimmedNote.length > 0 ? trimmedNote : null,
    },
  };
}
