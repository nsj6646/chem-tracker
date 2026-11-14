export type MasterEntityKind =
  | "domestic_importer"
  | "overseas_branch"
  | "chemical_substance";

export interface MasterItem {
  id: number;
  kind: MasterEntityKind;
  name: string | null;
  cas_number: string | null;
  substance_name: string | null;
  note: string | null;
  reference_count: number;
  product_count: number;
  component_count: number;
  shipment_count: number;
}

export interface UpdateMasterItemInput {
  id: number;
  kind: MasterEntityKind;
  name: string | null;
  cas_number: string | null;
  substance_name: string | null;
  note: string | null;
}

export interface DeleteMasterResult {
  kind: MasterEntityKind;
  deleted_id: number;
  deleted_label: string;
}

export interface MasterEditDraft {
  name: string;
  casNumber: string;
  substanceName: string;
  note: string;
}

export interface MasterEntityMeta {
  kind: MasterEntityKind;
  label: string;
  title: string;
  subtitle: string;
  referenceColumnLabel: string;
  supportsNote: boolean;
}

export interface MastersIpcClient {
  listMasterItems(kind: MasterEntityKind): Promise<MasterItem[]>;
  updateMasterItem(input: UpdateMasterItemInput): Promise<MasterItem>;
  deleteMasterItem(
    kind: MasterEntityKind,
    id: number,
  ): Promise<DeleteMasterResult>;
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

const MASTER_ENTITY_META: Record<MasterEntityKind, MasterEntityMeta> = {
  domestic_importer: {
    kind: "domestic_importer",
    label: "국내 수입사",
    title: "국내 수입사 관리/수정",
    subtitle:
      "수입사 이름을 수정하면 연결된 모든 수입실적에 즉시 반영됩니다. 수입실적에서 참조 중인 항목은 삭제할 수 없습니다.",
    referenceColumnLabel: "참조 현황",
    supportsNote: true,
  },
  overseas_branch: {
    kind: "overseas_branch",
    label: "해외지사",
    title: "해외지사 관리/수정",
    subtitle:
      "해외지사 이름을 수정하면 연결된 수입제품 및 수입실적에 즉시 반영됩니다. 참조 중인 항목은 삭제할 수 없습니다.",
    referenceColumnLabel: "참조 현황",
    supportsNote: true,
  },
  chemical_substance: {
    kind: "chemical_substance",
    label: "화학물질",
    title: "화학물질 (CAS번호) 관리/수정",
    subtitle:
      "CAS번호 오탈자 및 화학물질명을 수정하면 연결된 모든 제품 레시피(BOM)와 연간 보고서에 즉시 반영됩니다.",
    referenceColumnLabel: "참조 현황",
    supportsNote: false,
  },
};

export function getMasterEntityMeta(kind: MasterEntityKind): MasterEntityMeta {
  return MASTER_ENTITY_META[kind];
}

export function normalizeCasNumber(raw: string): string {
  const unified = raw
    .trim()
    .replace(/[\u2010-\u2015\u2212]/g, "-");
  return unified
    .replace(/\s*-\s*/g, "-")
    .replace(/\s+/g, " ")
    .trim();
}

export function formatMasterReferenceSummary(item: MasterItem): string {
  if (item.reference_count === 0) {
    return "미사용 (0건)";
  }
  switch (item.kind) {
    case "domestic_importer":
      return `수입실적 ${item.shipment_count}건`;
    case "overseas_branch":
      return `제품 ${item.product_count}건 · 실적 ${item.shipment_count}건 (총 ${item.reference_count}건)`;
    case "chemical_substance":
      return `BOM 성분 ${item.component_count}건`;
  }
}

export function buildDeleteBlockedReason(item: MasterItem): string | null {
  if (item.reference_count <= 0) {
    return null;
  }
  switch (item.kind) {
    case "domestic_importer":
      return `해당 국내 수입사('${item.name ?? ""}')을(를) 참조하는 수입실적이 ${item.shipment_count}건 존재하여 삭제할 수 없습니다.`;
    case "overseas_branch":
      if (item.product_count > 0 && item.shipment_count > 0) {
        return `해당 해외지사('${item.name ?? ""}')을(를) 참조하는 레코드가 총 ${item.reference_count}건(수입제품 ${item.product_count}건, 수입실적 ${item.shipment_count}건) 존재하여 삭제할 수 없습니다.`;
      }
      if (item.product_count > 0) {
        return `해당 해외지사('${item.name ?? ""}')을(를) 참조하는 수입제품이 ${item.product_count}건 존재하여 삭제할 수 없습니다.`;
      }
      return `해당 해외지사('${item.name ?? ""}')을(를) 참조하는 수입실적이 ${item.shipment_count}건 존재하여 삭제할 수 없습니다.`;
    case "chemical_substance":
      return `해당 화학물질('${item.cas_number ?? ""}')을(를) 참조하는 제품 구성성분이 ${item.component_count}건 존재하여 삭제할 수 없습니다.`;
  }
}

export function itemToEditDraft(item: MasterItem): MasterEditDraft {
  return {
    name: item.name ?? "",
    casNumber: item.cas_number ?? "",
    substanceName: item.substance_name ?? "",
    note: item.note ?? "",
  };
}

export function validateMasterDraft(
  kind: MasterEntityKind,
  draft: MasterEditDraft,
  existingItems: readonly MasterItem[],
  editingId: number,
): { ok: true; payload: UpdateMasterItemInput } | { ok: false; error: string } {
  if (kind === "chemical_substance") {
    const cas = normalizeCasNumber(draft.casNumber);
    if (!cas) {
      return { ok: false, error: "CAS번호를 입력해 주세요." };
    }
    const conflict = existingItems.find(
      (item) =>
        item.kind === kind &&
        item.id !== editingId &&
        (item.cas_number ?? "") === cas,
    );
    if (conflict) {
      return {
        ok: false,
        error: `이미 존재하는 CAS번호입니다: '${cas}'`,
      };
    }
    const substanceName = draft.substanceName.trim() || null;
    return {
      ok: true,
      payload: {
        id: editingId,
        kind,
        name: null,
        cas_number: cas,
        substance_name: substanceName,
        note: null,
      },
    };
  }

  const label = getMasterEntityMeta(kind).label;
  const trimmedName = draft.name.trim();
  if (!trimmedName) {
    return { ok: false, error: `${label} 이름을 입력해 주세요.` };
  }
  const conflict = existingItems.find(
    (item) =>
      item.kind === kind &&
      item.id !== editingId &&
      (item.name ?? "").toLowerCase() === trimmedName.toLowerCase(),
  );
  if (conflict) {
    return {
      ok: false,
      error: `이미 존재하는 ${label}입니다: '${trimmedName}'`,
    };
  }
  const trimmedNote = draft.note.trim() || null;
  return {
    ok: true,
    payload: {
      id: editingId,
      kind,
      name: trimmedName,
      cas_number: null,
      substance_name: null,
      note: trimmedNote,
    },
  };
}

const DEFAULT_SEEDED_ITEMS: MasterItem[] = [
  // Domestic Importers (matching db.rs sample dataset)
  {
    id: 1,
    kind: "domestic_importer",
    name: "(주)대한케미칼",
    cas_number: null,
    substance_name: null,
    note: "서울 본사 / 연간 보고 대상",
    reference_count: 5,
    product_count: 0,
    component_count: 0,
    shipment_count: 5,
  },
  {
    id: 2,
    kind: "domestic_importer",
    name: "한성정밀화학(주)",
    cas_number: null,
    substance_name: null,
    note: "울산 공장",
    reference_count: 1,
    product_count: 0,
    component_count: 0,
    shipment_count: 1,
  },
  {
    id: 3,
    kind: "domestic_importer",
    name: "세진소재산업(주)",
    cas_number: null,
    substance_name: null,
    note: null,
    reference_count: 0,
    product_count: 0,
    component_count: 0,
    shipment_count: 0,
  },
  // Overseas Branches
  {
    id: 1,
    kind: "overseas_branch",
    name: "유럽지사 (독일)",
    cas_number: null,
    substance_name: null,
    note: "Frankfurt Hub",
    reference_count: 7,
    product_count: 2,
    component_count: 0,
    shipment_count: 5,
  },
  {
    id: 2,
    kind: "overseas_branch",
    name: "북미지사 (미국)",
    cas_number: null,
    substance_name: null,
    note: "Houston Plant",
    reference_count: 2,
    product_count: 1,
    component_count: 0,
    shipment_count: 1,
  },
  {
    id: 3,
    kind: "overseas_branch",
    name: "일본지사 (도쿄)",
    cas_number: null,
    substance_name: null,
    note: "Tokyo Office",
    reference_count: 0,
    product_count: 0,
    component_count: 0,
    shipment_count: 0,
  },
  // Chemical Substances
  {
    id: 1,
    kind: "chemical_substance",
    name: null,
    cas_number: "67-64-1",
    substance_name: "아세톤 (Acetone)",
    note: null,
    reference_count: 2,
    product_count: 2,
    component_count: 2,
    shipment_count: 0,
  },
  {
    id: 2,
    kind: "chemical_substance",
    name: null,
    cas_number: "108-88-3",
    substance_name: "톨루엔 (Toluene)",
    note: null,
    reference_count: 2,
    product_count: 2,
    component_count: 2,
    shipment_count: 0,
  },
  {
    id: 3,
    kind: "chemical_substance",
    name: null,
    cas_number: "141-78-6",
    substance_name: "초산에틸 (Ethyl acetate)",
    note: null,
    reference_count: 1,
    product_count: 1,
    component_count: 1,
    shipment_count: 0,
  },
  {
    id: 4,
    kind: "chemical_substance",
    name: null,
    cas_number: "71-36-3",
    substance_name: "1-부탄올 (1-Butanol)",
    note: null,
    reference_count: 1,
    product_count: 1,
    component_count: 1,
    shipment_count: 0,
  },
  {
    id: 5,
    kind: "chemical_substance",
    name: null,
    cas_number: "7732-18-5",
    substance_name: "정제수 (Water)",
    note: null,
    reference_count: 1,
    product_count: 1,
    component_count: 1,
    shipment_count: 0,
  },
  {
    id: 6,
    kind: "chemical_substance",
    name: null,
    cas_number: "1330-20-7",
    substance_name: "크실렌 (Xylene)",
    note: null,
    reference_count: 1,
    product_count: 1,
    component_count: 1,
    shipment_count: 0,
  },
  {
    id: 7,
    kind: "chemical_substance",
    name: null,
    cas_number: "9002-88-4",
    substance_name: null,
    note: null,
    reference_count: 1,
    product_count: 1,
    component_count: 1,
    shipment_count: 0,
  },
];

export function createOfflineMastersIpc(
  seedItems: readonly MasterItem[] = DEFAULT_SEEDED_ITEMS,
): MastersIpcClient {
  let items: MasterItem[] = seedItems.map((item) => ({ ...item }));

  return {
    async listMasterItems(kind) {
      return items
        .filter((item) => item.kind === kind)
        .map((item) => ({ ...item }))
        .sort((a, b) => {
          if (kind === "chemical_substance") {
            return (a.cas_number ?? "").localeCompare(b.cas_number ?? "");
          }
          return (a.name ?? "").localeCompare(b.name ?? "", "ko");
        });
    },

    async updateMasterItem(input) {
      const idx = items.findIndex(
        (item) => item.kind === input.kind && item.id === input.id,
      );
      if (idx < 0) {
        throw new Error(
          `수정할 ${getMasterEntityMeta(input.kind).label}을(를) 찾을 수 없습니다 (ID: ${input.id}).`,
        );
      }

      const validation = validateMasterDraft(
        input.kind,
        {
          name: input.name ?? "",
          casNumber: input.cas_number ?? "",
          substanceName: input.substance_name ?? "",
          note: input.note ?? "",
        },
        items,
        input.id,
      );
      if (!validation.ok) {
        throw new Error(validation.error);
      }

      const existing = items[idx];
      const updated: MasterItem = {
        ...existing,
        name: validation.payload.name,
        cas_number: validation.payload.cas_number,
        substance_name: validation.payload.substance_name,
        note: validation.payload.note,
      };
      items[idx] = updated;
      return { ...updated };
    },

    async deleteMasterItem(kind, id) {
      const idx = items.findIndex(
        (item) => item.kind === kind && item.id === id,
      );
      if (idx < 0) {
        throw new Error(
          `삭제할 ${getMasterEntityMeta(kind).label}을(를) 찾을 수 없습니다 (ID: ${id}).`,
        );
      }
      const target = items[idx];
      const blocked = buildDeleteBlockedReason(target);
      if (blocked) {
        throw new Error(blocked);
      }
      items.splice(idx, 1);
      return {
        kind,
        deleted_id: target.id,
        deleted_label:
          kind === "chemical_substance"
            ? (target.cas_number ?? "")
            : (target.name ?? ""),
      };
    },
  };
}

export function createMastersIpcClient(
  invokeOverride?: TauriInvoke,
): MastersIpcClient {
  const invoke = invokeOverride ?? detectTauriInvoke();
  if (invoke) {
    return {
      listMasterItems: (kind) =>
        invoke<MasterItem[]>("list_master_items", { kind }),
      updateMasterItem: (input) =>
        invoke<MasterItem>("update_master_item", { input }),
      deleteMasterItem: (kind, id) =>
        invoke<DeleteMasterResult>("delete_master_item", { kind, id }),
    };
  }
  return createOfflineMastersIpc();
}
