import {
  HUNDRED_PERCENT_SCALED,
  formatPct,
  normalizeCasNumber,
  parseScaled4dp,
} from "../lib/decimal";
import { type TauriInvoke, detectTauriInvoke } from "../lib/ipc";

export interface ProductListItem {
  id: number;
  product_code: string;
  product_name: string | null;
  overseas_branch_id: number;
  overseas_branch_name: string;
  component_count: number;
  weight_pct_sum_scaled: number;
  is_valid_100_pct: boolean;
}

export interface ProductComponentDetail {
  id: number;
  substance_id: number;
  cas_number: string;
  substance_name: string | null;
  weight_pct_text: string;
  weight_pct_scaled: number;
}

export interface ProductDetail {
  id: number;
  product_code: string;
  product_name: string | null;
  overseas_branch_id: number;
  overseas_branch_name: string;
  weight_pct_sum_scaled: number;
  is_valid_100_pct: boolean;
  components: ProductComponentDetail[];
}

export interface OverseasBranchOption {
  id: number;
  name: string;
  note: string | null;
}

export interface ChemicalSubstanceOption {
  id: number;
  cas_number: string;
  substance_name: string | null;
}

export interface SaveBomComponentInput {
  cas_number: string;
  substance_name: string | null;
  weight_pct_text: string;
}

export interface SaveProductInput {
  id: number | null;
  product_code: string;
  product_name: string | null;
  overseas_branch_name: string;
  components: SaveBomComponentInput[];
}

export interface SaveProductResult {
  product: ProductDetail;
  created: boolean;
}

export interface DeleteProductResult {
  deleted_product_id: number;
  deleted_product_code: string;
}

export interface BomRowDraft {
  casNumber: string;
  substanceName: string;
  weightPctText: string;
}

export interface BomGaugeEvaluation {
  sumScaled: number;
  diffScaled: number;
  isExact100: boolean;
  canSave: boolean;
  duplicateCasNumbers: string[];
  rowErrors: string[];
  bannerText: string;
}

export interface ProductsIpcClient {
  listProducts(
    search?: string | null,
    overseasBranchId?: number | null,
  ): Promise<ProductListItem[]>;
  getProductDetail(productId: number): Promise<ProductDetail>;
  listOverseasBranches(): Promise<OverseasBranchOption[]>;
  searchChemicalSubstances(query?: string | null): Promise<ChemicalSubstanceOption[]>;
  saveProductWithBom(input: SaveProductInput): Promise<SaveProductResult>;
  deleteProduct(productId: number): Promise<DeleteProductResult>;
}

export function evaluateBomGauge(rows: readonly BomRowDraft[]): BomGaugeEvaluation {
  let sumScaled = 0;
  const seenCas = new Set<string>();
  const duplicateCas = new Set<string>();
  const rowErrors: string[] = [];

  if (rows.length === 0) {
    rowErrors.push("제품 구성성분(BOM)을 최소 1개 이상 입력해 주세요.");
  }

  for (let i = 0; i < rows.length; i += 1) {
    const row = rows[i];
    const cas = normalizeCasNumber(row.casNumber);
    if (!cas) {
      rowErrors.push(`${i + 1}번째 행의 CAS번호를 입력해 주세요.`);
    } else if (seenCas.has(cas)) {
      duplicateCas.add(cas);
      rowErrors.push(`중복된 CAS번호입니다: ${cas}`);
    } else {
      seenCas.add(cas);
    }

    const parsed = parseScaled4dp(row.weightPctText);
    if (!parsed.ok) {
      rowErrors.push(`${i + 1}번째 행 함유율 오류: ${parsed.error}`);
      continue;
    }

    if (parsed.scaled <= 0 || parsed.scaled > HUNDRED_PERCENT_SCALED) {
      rowErrors.push(
        `${i + 1}번째 행 함유율(%)은 0보다 크고 100 이하이어야 합니다.`,
      );
    }
    sumScaled += parsed.scaled;
  }

  const diffScaled = sumScaled - HUNDRED_PERCENT_SCALED;
  const isExact100 = sumScaled === HUNDRED_PERCENT_SCALED;
  const canSave = isExact100 && rowErrors.length === 0 && duplicateCas.size === 0;

  let bannerText: string;
  if (isExact100) {
    bannerText = "합계: 100.00% — 저장 가능";
  } else {
    const sumFormatted = formatPct(sumScaled);
    const diffFormatted =
      diffScaled > 0
        ? `+${formatPct(diffScaled)}`
        : `-${formatPct(-diffScaled)}`;
    bannerText = `합계: ${sumFormatted}% — 합계가 정확히 100%여야 저장할 수 있습니다 (차이: ${diffFormatted}%)`;
  }

  return {
    sumScaled,
    diffScaled,
    isExact100,
    canSave,
    duplicateCasNumbers: Array.from(duplicateCas),
    rowErrors,
    bannerText,
  };
}

export function filterProductList(
  products: readonly ProductListItem[],
  search: string,
  overseasBranchId: number | null,
): ProductListItem[] {
  const q = search.trim().toLowerCase();
  return products.filter((p) => {
    if (overseasBranchId !== null && p.overseas_branch_id !== overseasBranchId) {
      return false;
    }
    if (!q) {
      return true;
    }
    return (
      p.product_code.toLowerCase().includes(q) ||
      (p.product_name ?? "").toLowerCase().includes(q) ||
      p.overseas_branch_name.toLowerCase().includes(q)
    );
  });
}

export function findSubstanceSuggestions(
  substances: readonly ChemicalSubstanceOption[],
  query: string,
): ChemicalSubstanceOption[] {
  const q = query.trim().toLowerCase();
  if (!q) {
    return [...substances];
  }
  return substances.filter(
    (s) =>
      s.cas_number.toLowerCase().includes(q) ||
      (s.substance_name ?? "").toLowerCase().includes(q),
  );
}

export function autoFillSubstanceNameOnCasChange(
  row: BomRowDraft,
  newCas: string,
  substances: readonly ChemicalSubstanceOption[],
): BomRowDraft {
  const trimmedCas = newCas.trim();
  const matched = substances.find((s) => s.cas_number === trimmedCas);
  if (matched) {
    return {
      ...row,
      casNumber: newCas,
      substanceName: matched.substance_name ?? "",
    };
  }
  return {
    ...row,
    casNumber: newCas,
  };
}

function createOfflineProductsIpc(): ProductsIpcClient {
  let nextBranchId = 4;
  let nextSubstanceId = 8;
  let nextProductId = 4;
  let nextCompId = 10;

  const branches: OverseasBranchOption[] = [
    { id: 1, name: "유럽지사 (독일)", note: "Frankfurt Hub" },
    { id: 2, name: "북미지사 (미국)", note: "Houston Plant" },
    { id: 3, name: "일본지사 (도쿄)", note: "Tokyo Office" },
  ];

  const substances: ChemicalSubstanceOption[] = [
    { id: 1, cas_number: "67-64-1", substance_name: "아세톤 (Acetone)" },
    { id: 2, cas_number: "108-88-3", substance_name: "톨루엔 (Toluene)" },
    { id: 3, cas_number: "141-78-6", substance_name: "초산에틸 (Ethyl acetate)" },
    { id: 4, cas_number: "71-36-3", substance_name: "1-부탄올 (1-Butanol)" },
    { id: 5, cas_number: "7732-18-5", substance_name: "정제수 (Water)" },
    { id: 6, cas_number: "1330-20-7", substance_name: "크실렌 (Xylene)" },
    { id: 7, cas_number: "9002-88-4", substance_name: null },
  ];

  const shipmentCountByProductId = new Map<number, number>([
    [1, 3],
    [2, 2],
    [3, 1],
  ]);

  const products: ProductDetail[] = [
    {
      id: 1,
      product_code: "PRD-EU-101",
      product_name: "코팅솔벤트 A-100",
      overseas_branch_id: 1,
      overseas_branch_name: "유럽지사 (독일)",
      weight_pct_sum_scaled: HUNDRED_PERCENT_SCALED,
      is_valid_100_pct: true,
      components: [
        {
          id: 1,
          substance_id: 1,
          cas_number: "67-64-1",
          substance_name: "아세톤 (Acetone)",
          weight_pct_text: "45.25",
          weight_pct_scaled: 452_500,
        },
        {
          id: 2,
          substance_id: 2,
          cas_number: "108-88-3",
          substance_name: "톨루엔 (Toluene)",
          weight_pct_text: "34.75",
          weight_pct_scaled: 347_500,
        },
        {
          id: 3,
          substance_id: 3,
          cas_number: "141-78-6",
          substance_name: "초산에틸 (Ethyl acetate)",
          weight_pct_text: "20",
          weight_pct_scaled: 200_000,
        },
      ],
    },
    {
      id: 2,
      product_code: "PRD-EU-204",
      product_name: "고기능 세정제 B-200",
      overseas_branch_id: 1,
      overseas_branch_name: "유럽지사 (독일)",
      weight_pct_sum_scaled: HUNDRED_PERCENT_SCALED,
      is_valid_100_pct: true,
      components: [
        {
          id: 4,
          substance_id: 1,
          cas_number: "67-64-1",
          substance_name: "아세톤 (Acetone)",
          weight_pct_text: "60",
          weight_pct_scaled: 600_000,
        },
        {
          id: 5,
          substance_id: 4,
          cas_number: "71-36-3",
          substance_name: "1-부탄올 (1-Butanol)",
          weight_pct_text: "25.5",
          weight_pct_scaled: 255_000,
        },
        {
          id: 6,
          substance_id: 5,
          cas_number: "7732-18-5",
          substance_name: "정제수 (Water)",
          weight_pct_text: "14.5",
          weight_pct_scaled: 145_000,
        },
      ],
    },
    {
      id: 3,
      product_code: "PRD-US-305",
      product_name: "표면처리제 C-300",
      overseas_branch_id: 2,
      overseas_branch_name: "북미지사 (미국)",
      weight_pct_sum_scaled: HUNDRED_PERCENT_SCALED,
      is_valid_100_pct: true,
      components: [
        {
          id: 7,
          substance_id: 6,
          cas_number: "1330-20-7",
          substance_name: "크실렌 (Xylene)",
          weight_pct_text: "72.125",
          weight_pct_scaled: 721_250,
        },
        {
          id: 8,
          substance_id: 2,
          cas_number: "108-88-3",
          substance_name: "톨루엔 (Toluene)",
          weight_pct_text: "17.875",
          weight_pct_scaled: 178_750,
        },
        {
          id: 9,
          substance_id: 7,
          cas_number: "9002-88-4",
          substance_name: null,
          weight_pct_text: "10",
          weight_pct_scaled: 100_000,
        },
      ],
    },
  ];

  function toListItem(p: ProductDetail): ProductListItem {
    return {
      id: p.id,
      product_code: p.product_code,
      product_name: p.product_name,
      overseas_branch_id: p.overseas_branch_id,
      overseas_branch_name: p.overseas_branch_name,
      component_count: p.components.length,
      weight_pct_sum_scaled: p.weight_pct_sum_scaled,
      is_valid_100_pct: p.is_valid_100_pct,
    };
  }

  return {
    async listProducts(search = "", overseasBranchId = null) {
      const items = products
        .map(toListItem)
        .sort((a, b) => a.product_code.localeCompare(b.product_code));
      return filterProductList(items, search ?? "", overseasBranchId ?? null);
    },

    async getProductDetail(productId) {
      const found = products.find((p) => p.id === productId);
      if (!found) {
        throw new Error(`수입제품을 찾을 수 없습니다 (ID: ${productId}).`);
      }
      return structuredClone(found);
    },

    async listOverseasBranches() {
      return [...branches].sort((a, b) => a.name.localeCompare(b.name));
    },

    async searchChemicalSubstances(query = "") {
      return findSubstanceSuggestions(substances, query ?? "").sort((a, b) =>
        a.cas_number.localeCompare(b.cas_number),
      );
    },

    async saveProductWithBom(input) {
      const code = input.product_code.trim();
      if (!code) {
        throw new Error("수입제품코드를 입력해 주세요.");
      }
      const branchName = input.overseas_branch_name.trim();
      if (!branchName) {
        throw new Error("해외지사를 선택하거나 입력해 주세요.");
      }

      const gauge = evaluateBomGauge(
        input.components.map((c) => ({
          casNumber: c.cas_number,
          substanceName: c.substance_name ?? "",
          weightPctText: c.weight_pct_text,
        })),
      );
      if (!gauge.canSave) {
        throw new Error(gauge.rowErrors[0] ?? gauge.bannerText);
      }

      let branch = branches.find(
        (b) => b.name.toLowerCase() === branchName.toLowerCase(),
      );
      if (!branch) {
        branch = { id: nextBranchId++, name: branchName, note: null };
        branches.push(branch);
      }

      const savedComponents: ProductComponentDetail[] = input.components.map((c) => {
        const cas = c.cas_number.trim();
        const trimmedName = c.substance_name?.trim() || null;
        let sub = substances.find((s) => s.cas_number === cas);
        if (!sub) {
          sub = {
            id: nextSubstanceId++,
            cas_number: cas,
            substance_name: trimmedName,
          };
          substances.push(sub);
        } else if (trimmedName) {
          sub.substance_name = trimmedName;
        }
        const parsed = parseScaled4dp(c.weight_pct_text);
        if (!parsed.ok) {
          throw new Error(parsed.error);
        }
        return {
          id: nextCompId++,
          substance_id: sub.id,
          cas_number: sub.cas_number,
          substance_name: sub.substance_name,
          weight_pct_text: parsed.canonical,
          weight_pct_scaled: parsed.scaled,
        };
      });

      const productName = input.product_name?.trim() || null;
      let targetIdx = -1;
      let created = false;

      if (input.id !== null) {
        targetIdx = products.findIndex((p) => p.id === input.id);
        if (targetIdx < 0) {
          throw new Error(`수정할 수입제품을 찾을 수 없습니다 (ID: ${input.id}).`);
        }
      }
      const conflict = products.find(
        (p) =>
          p.id !== input.id &&
          p.product_code.toLowerCase() === code.toLowerCase(),
      );
      if (conflict) {
        throw new Error(`이미 사용 중인 수입제품코드입니다: '${code}'`);
      }

      const detail: ProductDetail = {
        id: targetIdx >= 0 ? products[targetIdx].id : nextProductId++,
        product_code: code,
        product_name: productName,
        overseas_branch_id: branch.id,
        overseas_branch_name: branch.name,
        weight_pct_sum_scaled: HUNDRED_PERCENT_SCALED,
        is_valid_100_pct: true,
        components: savedComponents,
      };

      if (targetIdx >= 0) {
        products[targetIdx] = detail;
      } else {
        products.push(detail);
        created = true;
      }

      return { product: structuredClone(detail), created };
    },

    async deleteProduct(productId) {
      const idx = products.findIndex((p) => p.id === productId);
      if (idx < 0) {
        throw new Error(`삭제할 수입제품을 찾을 수 없습니다 (ID: ${productId}).`);
      }
      const refCount = shipmentCountByProductId.get(productId) ?? 0;
      if (refCount > 0) {
        throw new Error(
          `해당 수입제품으로 등록된 수입실적이 ${refCount}건 존재하여 삭제할 수 없습니다. 수입실적을 먼저 삭제하거나 수정해 주세요.`,
        );
      }
      const [removed] = products.splice(idx, 1);
      return {
        deleted_product_id: removed.id,
        deleted_product_code: removed.product_code,
      };
    },
  };
}

export function createProductsIpcClient(
  invokeOverride?: TauriInvoke,
): ProductsIpcClient {
  const invoke = invokeOverride ?? detectTauriInvoke();
  if (invoke) {
    return {
      listProducts: (search = null, overseasBranchId = null) =>
        invoke<ProductListItem[]>("list_products", { search, overseasBranchId }),
      getProductDetail: (productId) =>
        invoke<ProductDetail>("get_product_detail", { productId }),
      listOverseasBranches: () =>
        invoke<OverseasBranchOption[]>("list_overseas_branches"),
      searchChemicalSubstances: (query = null) =>
        invoke<ChemicalSubstanceOption[]>("search_chemical_substances", { query }),
      saveProductWithBom: (input) =>
        invoke<SaveProductResult>("save_product_with_bom", { input }),
      deleteProduct: (productId) =>
        invoke<DeleteProductResult>("delete_product", { productId }),
    };
  }
  return createOfflineProductsIpc();
}
