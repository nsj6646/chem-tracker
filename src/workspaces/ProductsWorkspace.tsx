import React, { useCallback, useEffect, useId, useMemo, useState } from "react";
import { MasterModal } from "./MasterModal";
import {
  type BomRowDraft,
  type ChemicalSubstanceOption,
  type OverseasBranchOption,
  type ProductDetail,
  type ProductListItem,
  type ProductsIpcClient,
  autoFillSubstanceNameOnCasChange,
  createProductsIpcClient,
  evaluateBomGauge,
  filterProductList,
} from "./productsLogic";
import { formatPct, normalizeCasNumber } from "../lib/decimal";
import { subscribeDbRestoredEvent } from "../lib/ipc";

export type MasterModalTarget = "overseas_branch" | "chemical_substance";

export interface ProductsWorkspaceProps {
  ipc?: ProductsIpcClient;
  refreshKey?: number;
  onOpenMasterModal?: (target: MasterModalTarget) => void;
  onProductsChanged?: () => void;
}

interface ProductEditorDraft {
  id: number | null;
  productCode: string;
  productName: string;
  overseasBranchName: string;
  components: BomRowDraft[];
}

function emptyDraft(defaultBranchName: string): ProductEditorDraft {
  return {
    id: null,
    productCode: "",
    productName: "",
    overseasBranchName: defaultBranchName,
    components: [{ casNumber: "", substanceName: "", weightPctText: "" }],
  };
}

function detailToDraft(detail: ProductDetail): ProductEditorDraft {
  return {
    id: detail.id,
    productCode: detail.product_code,
    productName: detail.product_name ?? "",
    overseasBranchName: detail.overseas_branch_name,
    components:
      detail.components.length > 0
        ? detail.components.map((c) => ({
            casNumber: c.cas_number,
            substanceName: c.substance_name ?? "",
            weightPctText: c.weight_pct_text,
          }))
        : [{ casNumber: "", substanceName: "", weightPctText: "" }],
  };
}

export function ProductsWorkspace({
  ipc: ipcProp,
  refreshKey = 0,
  onOpenMasterModal,
  onProductsChanged,
}: ProductsWorkspaceProps) {
  const ipc = useMemo(() => ipcProp ?? createProductsIpcClient(), [ipcProp]);
  const casDatalistId = useId();
  const [restoreTick, setRestoreTick] = useState(0);

  const [products, setProducts] = useState<ProductListItem[]>([]);
  const [branches, setBranches] = useState<OverseasBranchOption[]>([]);
  const [substances, setSubstances] = useState<ChemicalSubstanceOption[]>([]);

  const [searchText, setSearchText] = useState("");
  const [filterBranchId, setFilterBranchId] = useState<number | null>(null);
  const [selectedProductId, setSelectedProductId] = useState<number | null>(null);
  const [draft, setDraft] = useState<ProductEditorDraft>(() => emptyDraft(""));

  const [addingBranch, setAddingBranch] = useState(false);
  const [newBranchName, setNewBranchName] = useState("");
  const [confirmingDelete, setConfirmingDelete] = useState(false);

  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [feedback, setFeedback] = useState<{ ok: boolean; text: string } | null>(null);
  const [masterTarget, setMasterTarget] = useState<MasterModalTarget | null>(null);

  const loadReferenceAndList = useCallback(
    async (preferSelectId?: number | null) => {
      const [prodList, branchList, substanceList] = await Promise.all([
        ipc.listProducts(null, null),
        ipc.listOverseasBranches(),
        ipc.searchChemicalSubstances(null),
      ]);
      setProducts(prodList);
      setBranches(branchList);
      setSubstances(substanceList);

      const targetId =
        preferSelectId !== undefined
          ? preferSelectId
          : prodList.length > 0
            ? prodList[0].id
            : null;

      if (targetId !== null) {
        const exists = prodList.some((p) => p.id === targetId);
        if (exists) {
          const detail = await ipc.getProductDetail(targetId);
          setSelectedProductId(detail.id);
          setDraft(detailToDraft(detail));
          return;
        }
      }

      setSelectedProductId(null);
      setDraft(emptyDraft(branchList[0]?.name ?? ""));
    },
    [ipc],
  );

  useEffect(() => {
    return subscribeDbRestoredEvent(() => {
      setRestoreTick((prev) => prev + 1);
    });
  }, []);

  useEffect(() => {
    let active = true;
    setLoading(true);
    loadReferenceAndList()
      .catch((err) => {
        if (active) {
          setFeedback({
            ok: false,
            text: err instanceof Error ? err.message : String(err),
          });
        }
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [loadReferenceAndList, refreshKey, restoreTick]);

  const filteredProducts = useMemo(
    () => filterProductList(products, searchText, filterBranchId),
    [products, searchText, filterBranchId],
  );

  const gauge = useMemo(
    () => evaluateBomGauge(draft.components),
    [draft.components],
  );

  const branchOptions = useMemo(() => {
    const names = branches.map((b) => b.name);
    const current = draft.overseasBranchName.trim();
    if (
      current &&
      !names.some((n) => n.toLowerCase() === current.toLowerCase())
    ) {
      return [...names, current];
    }
    return names;
  }, [branches, draft.overseasBranchName]);

  const handleSelectProduct = async (productId: number) => {
    setFeedback(null);
    setConfirmingDelete(false);
    setAddingBranch(false);
    try {
      const detail = await ipc.getProductDetail(productId);
      setSelectedProductId(detail.id);
      setDraft(detailToDraft(detail));
    } catch (err) {
      setFeedback({
        ok: false,
        text: err instanceof Error ? err.message : String(err),
      });
    }
  };

  const handleStartNewProduct = () => {
    setSelectedProductId(null);
    setConfirmingDelete(false);
    setAddingBranch(false);
    setFeedback(null);
    setDraft(emptyDraft(branches[0]?.name ?? ""));
  };

  const handleApplyNewBranch = () => {
    const trimmed = newBranchName.trim();
    if (!trimmed) {
      setFeedback({ ok: false, text: "새 해외지사 이름을 입력해 주세요." });
      return;
    }
    setDraft((prev) => ({ ...prev, overseasBranchName: trimmed }));
    setNewBranchName("");
    setAddingBranch(false);
    setFeedback({
      ok: true,
      text: `해외지사 '${trimmed}'이(가) 선택되었습니다. 제품 저장 시 자동 등록됩니다.`,
    });
  };

  const handleTriggerMasterModal = (target: MasterModalTarget) => {
    if (onOpenMasterModal) {
      onOpenMasterModal(target);
      return;
    }
    setMasterTarget(target);
  };

  const refreshAfterMasterChange = useCallback(async () => {
    if (selectedProductId !== null) {
      await loadReferenceAndList(selectedProductId);
    } else {
      const [prodList, branchList, substanceList] = await Promise.all([
        ipc.listProducts(null, null),
        ipc.listOverseasBranches(),
        ipc.searchChemicalSubstances(null),
      ]);
      setProducts(prodList);
      setBranches(branchList);
      setSubstances(substanceList);
    }
    onProductsChanged?.();
  }, [ipc, loadReferenceAndList, onProductsChanged, selectedProductId]);

  const handleComponentCasChange = (index: number, newCas: string) => {
    setFeedback(null);
    setDraft((prev) => ({
      ...prev,
      components: prev.components.map((row, idx) =>
        idx === index
          ? autoFillSubstanceNameOnCasChange(row, newCas, substances)
          : row,
      ),
    }));
  };

  const handleComponentFieldChange = (
    index: number,
    field: "substanceName" | "weightPctText",
    value: string,
  ) => {
    setFeedback(null);
    setDraft((prev) => ({
      ...prev,
      components: prev.components.map((row, idx) =>
        idx === index ? { ...row, [field]: value } : row,
      ),
    }));
  };

  const handleAddComponentRow = () => {
    setFeedback(null);
    setDraft((prev) => ({
      ...prev,
      components: [
        ...prev.components,
        { casNumber: "", substanceName: "", weightPctText: "" },
      ],
    }));
  };

  const handleRemoveComponentRow = (index: number) => {
    setFeedback(null);
    setDraft((prev) => ({
      ...prev,
      components: prev.components.filter((_, idx) => idx !== index),
    }));
  };

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!draft.productCode.trim()) {
      setFeedback({ ok: false, text: "수입제품코드를 입력해 주세요." });
      return;
    }
    if (!draft.overseasBranchName.trim()) {
      setFeedback({ ok: false, text: "해외지사를 선택하거나 입력해 주세요." });
      return;
    }
    if (!gauge.canSave) {
      setFeedback({
        ok: false,
        text: gauge.rowErrors[0] ?? gauge.bannerText,
      });
      return;
    }

    setSaving(true);
    setFeedback(null);
    setConfirmingDelete(false);
    try {
      const result = await ipc.saveProductWithBom({
        id: draft.id,
        product_code: draft.productCode.trim(),
        product_name: draft.productName.trim() || null,
        overseas_branch_name: draft.overseasBranchName.trim(),
        components: draft.components.map((c) => ({
          cas_number: normalizeCasNumber(c.casNumber),
          substance_name: c.substanceName.trim() || null,
          weight_pct_text: c.weightPctText.trim(),
        })),
      });

      await loadReferenceAndList(result.product.id);
      setFeedback({
        ok: true,
        text: result.created
          ? `수입제품 '${result.product.product_code}' 및 레시피(100.00%)가 등록되었습니다.`
          : `수입제품 '${result.product.product_code}' 레시피(100.00%)가 저장되었습니다.`,
      });
      onProductsChanged?.();
    } catch (err) {
      setFeedback({
        ok: false,
        text: err instanceof Error ? err.message : String(err),
      });
    } finally {
      setSaving(false);
    }
  };

  const handleConfirmDelete = async () => {
    if (draft.id === null) return;
    setSaving(true);
    setFeedback(null);
    try {
      const deleted = await ipc.deleteProduct(draft.id);
      setConfirmingDelete(false);
      await loadReferenceAndList(undefined);
      setFeedback({
        ok: true,
        text: `수입제품 '${deleted.deleted_product_code}'이(가) 삭제되었습니다.`,
      });
      onProductsChanged?.();
    } catch (err) {
      setConfirmingDelete(false);
      setFeedback({
        ok: false,
        text: err instanceof Error ? err.message : String(err),
      });
    } finally {
      setSaving(false);
    }
  };

  const canSubmit =
    !saving &&
    gauge.canSave &&
    draft.productCode.trim().length > 0 &&
    draft.overseasBranchName.trim().length > 0;

  return (
    <section aria-label="1. 수입제품 및 레시피(BOM) 작업공간">
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "minmax(340px, 1fr) minmax(460px, 1.35fr)",
          gap: "14px",
          alignItems: "start",
        }}
      >
        {/* Left Pane: Imported Product Master List */}
        <div className="card">
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              gap: "8px",
              marginBottom: "10px",
              flexWrap: "wrap",
            }}
          >
            <h2 className="card-title" style={{ margin: 0 }}>
              1. 수입제품 목록 ({filteredProducts.length}건)
            </h2>
            <button
              type="button"
              onClick={handleStartNewProduct}
              style={{
                background: "var(--primary)",
                color: "#ffffff",
                border: "1px solid var(--primary)",
                borderRadius: "5px",
                padding: "6px 11px",
                fontWeight: 700,
                cursor: "pointer",
              }}
            >
              + 신규 수입제품 등록
            </button>
          </div>

          <div
            style={{
              display: "grid",
              gridTemplateColumns: "1fr 165px",
              gap: "8px",
              marginBottom: "10px",
            }}
          >
            <input
              type="text"
              aria-label="수입제품 검색"
              placeholder="수입제품코드, 제품명, 해외지사 검색..."
              value={searchText}
              onChange={(e) => setSearchText(e.target.value)}
              style={{
                padding: "6px 9px",
                border: "1px solid var(--border)",
                borderRadius: "4px",
              }}
            />
            <select
              aria-label="해외지사 필터"
              value={filterBranchId === null ? "" : String(filterBranchId)}
              onChange={(e) =>
                setFilterBranchId(
                  e.target.value ? Number(e.target.value) : null,
                )
              }
              style={{
                padding: "6px 8px",
                border: "1px solid var(--border)",
                borderRadius: "4px",
                background: "#ffffff",
              }}
            >
              <option value="">전체 해외지사</option>
              {branches.map((b) => (
                <option key={b.id} value={b.id}>
                  {b.name}
                </option>
              ))}
            </select>
          </div>

          <div style={{ overflowX: "auto" }}>
            <table
              style={{
                width: "100%",
                borderCollapse: "collapse",
                fontSize: "12.5px",
              }}
            >
              <thead>
                <tr style={{ background: "var(--surface-alt)" }}>
                  <th style={thStyle}>수입제품코드</th>
                  <th style={thStyle}>제품명(선택)</th>
                  <th style={thStyle}>해외지사</th>
                  <th style={{ ...thStyle, textAlign: "right" }}>구성 물질 수</th>
                  <th style={{ ...thStyle, textAlign: "center" }}>검증 상태</th>
                </tr>
              </thead>
              <tbody>
                {loading ? (
                  <tr>
                    <td colSpan={5} style={{ ...tdStyle, textAlign: "center" }}>
                      목록을 불러오는 중...
                    </td>
                  </tr>
                ) : filteredProducts.length === 0 ? (
                  <tr>
                    <td
                      colSpan={5}
                      style={{
                        ...tdStyle,
                        textAlign: "center",
                        color: "var(--text-muted)",
                      }}
                    >
                      조건에 맞는 수입제품이 없습니다.
                    </td>
                  </tr>
                ) : (
                  filteredProducts.map((p) => {
                    const selected = p.id === selectedProductId;
                    return (
                      <tr
                        key={p.id}
                        onClick={() => handleSelectProduct(p.id)}
                        style={{
                          cursor: "pointer",
                          background: selected ? "#eff6ff" : "#ffffff",
                        }}
                      >
                        <td
                          style={{
                            ...tdStyle,
                            fontFamily: "var(--mono)",
                            fontWeight: 700,
                          }}
                        >
                          {p.product_code}
                        </td>
                        <td style={tdStyle}>
                          {p.product_name ? (
                            p.product_name
                          ) : (
                            <span style={{ color: "#94a3b8" }}>(미입력)</span>
                          )}
                        </td>
                        <td style={tdStyle}>{p.overseas_branch_name}</td>
                        <td style={{ ...tdStyle, textAlign: "right" }}>
                          {p.component_count}종
                        </td>
                        <td style={{ ...tdStyle, textAlign: "center" }}>
                          <span
                            style={{
                              display: "inline-block",
                              padding: "2px 7px",
                              borderRadius: "99px",
                              fontSize: "11px",
                              fontWeight: 700,
                              background: p.is_valid_100_pct
                                ? "var(--success-soft)"
                                : "var(--danger-soft)",
                              color: p.is_valid_100_pct
                                ? "var(--success)"
                                : "var(--danger)",
                            }}
                          >
                            {p.is_valid_100_pct
                              ? "100.00% 정상"
                              : `${formatPct(p.weight_pct_sum_scaled)}% 오류`}
                          </span>
                        </td>
                      </tr>
                    );
                  })
                )}
              </tbody>
            </table>
          </div>
        </div>

        {/* Right Pane: Confidential Product Recipe (BOM) Editor */}
        <form className="card" onSubmit={handleSave}>
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              gap: "8px",
              marginBottom: "12px",
              flexWrap: "wrap",
            }}
          >
            <h2 className="card-title" style={{ margin: 0 }}>
              {draft.id === null
                ? "신규 수입제품 및 레시피(BOM) 등록"
                : `제품 구성성분(BOM) 편집 — ${draft.productCode || "(코드 미입력)"}`}
            </h2>

            {draft.id !== null && !confirmingDelete && (
              <button
                type="button"
                disabled={saving}
                onClick={() => setConfirmingDelete(true)}
                style={{
                  background: "var(--danger-soft)",
                  color: "var(--danger)",
                  border: "1px solid #fca5a5",
                  borderRadius: "5px",
                  padding: "5px 10px",
                  fontWeight: 700,
                  cursor: "pointer",
                }}
              >
                제품 삭제
              </button>
            )}

            {draft.id !== null && confirmingDelete && (
              <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
                <span style={{ fontSize: "12px", color: "var(--danger)", fontWeight: 700 }}>
                  삭제할까요?
                </span>
                <button
                  type="button"
                  disabled={saving}
                  onClick={handleConfirmDelete}
                  style={{
                    background: "var(--danger)",
                    color: "#ffffff",
                    border: "1px solid var(--danger)",
                    borderRadius: "4px",
                    padding: "4px 9px",
                    fontWeight: 700,
                    cursor: "pointer",
                  }}
                >
                  삭제 확인
                </button>
                <button
                  type="button"
                  disabled={saving}
                  onClick={() => setConfirmingDelete(false)}
                  style={secondaryBtnStyle}
                >
                  취소
                </button>
              </div>
            )}
          </div>

          {/* Product Header Inputs */}
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "150px 1fr",
              gap: "10px",
              marginBottom: "10px",
            }}
          >
            <label style={labelStackStyle}>
              <span style={fieldLabelStyle}>수입제품코드 *</span>
              <input
                type="text"
                required
                placeholder="예: PRD-EU-101"
                value={draft.productCode}
                onChange={(e) => {
                  setFeedback(null);
                  setDraft((prev) => ({ ...prev, productCode: e.target.value }));
                }}
                style={{ ...inputStyle, fontFamily: "var(--mono)" }}
              />
            </label>

            <label style={labelStackStyle}>
              <span style={fieldLabelStyle}>제품명 (선택)</span>
              <input
                type="text"
                placeholder="예: 코팅솔벤트 A-100"
                value={draft.productName}
                onChange={(e) => {
                  setFeedback(null);
                  setDraft((prev) => ({ ...prev, productName: e.target.value }));
                }}
                style={inputStyle}
              />
            </label>
          </div>

          {/* Overseas Branch Selector + Inline Add + Master Modal Trigger */}
          <div style={{ marginBottom: "12px" }}>
            <div
              style={{
                display: "flex",
                justifyContent: "space-between",
                alignItems: "center",
                marginBottom: "4px",
              }}
            >
              <span style={fieldLabelStyle}>공급 해외지사 *</span>
              <div style={{ display: "flex", gap: "6px" }}>
                <button
                  type="button"
                  onClick={() => setAddingBranch((v) => !v)}
                  style={smallActionBtnStyle}
                >
                  + 새 해외지사 추가
                </button>
                <button
                  type="button"
                  onClick={() => handleTriggerMasterModal("overseas_branch")}
                  style={smallActionBtnStyle}
                >
                  [관리/수정]
                </button>
              </div>
            </div>

            <select
              aria-label="공급 해외지사"
              value={draft.overseasBranchName}
              onChange={(e) => {
                setFeedback(null);
                setDraft((prev) => ({
                  ...prev,
                  overseasBranchName: e.target.value,
                }));
              }}
              style={{ ...inputStyle, width: "100%", background: "#ffffff" }}
            >
              {branchOptions.length === 0 && (
                <option value="">해외지사를 추가해 주세요</option>
              )}
              {branchOptions.map((name) => (
                <option key={name} value={name}>
                  {name}
                </option>
              ))}
            </select>

            {addingBranch && (
              <div
                style={{
                  display: "flex",
                  gap: "6px",
                  marginTop: "6px",
                  alignItems: "center",
                }}
              >
                <input
                  type="text"
                  aria-label="새 해외지사 이름"
                  placeholder="새 해외지사 이름 입력 (예: 대만지사)"
                  value={newBranchName}
                  onChange={(e) => setNewBranchName(e.target.value)}
                  style={{ ...inputStyle, flex: 1 }}
                />
                <button
                  type="button"
                  onClick={handleApplyNewBranch}
                  style={smallPrimaryBtnStyle}
                >
                  적용
                </button>
                <button
                  type="button"
                  onClick={() => {
                    setAddingBranch(false);
                    setNewBranchName("");
                  }}
                  style={secondaryBtnStyle}
                >
                  취소
                </button>
              </div>
            )}
          </div>

          {/* Live Total Percentage Gauge */}
          <div
            role="status"
            aria-live="polite"
            style={{
              padding: "9px 12px",
              borderRadius: "5px",
              marginBottom: "12px",
              fontWeight: 700,
              fontSize: "12.5px",
              border: `1px solid ${
                gauge.isExact100 ? "#86efac" : "#fca5a5"
              }`,
              background: gauge.isExact100
                ? "var(--success-soft)"
                : "var(--danger-soft)",
              color: gauge.isExact100 ? "var(--success)" : "var(--danger)",
            }}
          >
            <div>{gauge.bannerText}</div>
            {gauge.rowErrors.length > 0 && (
              <div
                style={{
                  marginTop: "4px",
                  fontSize: "11.5px",
                  fontWeight: 600,
                  color: "var(--danger)",
                }}
              >
                {gauge.rowErrors[0]}
              </div>
            )}
          </div>

          {/* BOM Component Grid */}
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              marginBottom: "6px",
              gap: "8px",
            }}
          >
            <strong style={{ fontSize: "12.5px" }}>
              제품 구성성분 (대외비 BOM)
            </strong>
            <div style={{ display: "flex", gap: "6px" }}>
              <button
                type="button"
                onClick={() => handleTriggerMasterModal("chemical_substance")}
                style={smallActionBtnStyle}
              >
                [화학물질 관리/수정]
              </button>
              <button
                type="button"
                onClick={handleAddComponentRow}
                style={smallActionBtnStyle}
              >
                + 성분 추가
              </button>
            </div>
          </div>

          <datalist id={casDatalistId}>
            {substances.map((s) => (
              <option key={s.id} value={s.cas_number}>
                {s.substance_name
                  ? `${s.cas_number} — ${s.substance_name}`
                  : s.cas_number}
              </option>
            ))}
          </datalist>

          <table
            style={{
              width: "100%",
              borderCollapse: "collapse",
              fontSize: "12.5px",
              marginBottom: "12px",
            }}
          >
            <thead>
              <tr style={{ background: "var(--surface-alt)" }}>
                <th style={{ ...thStyle, width: "145px" }}>CAS번호</th>
                <th style={thStyle}>화학물질명 (선택)</th>
                <th style={{ ...thStyle, width: "115px", textAlign: "right" }}>
                  함유율 (%)
                </th>
                <th style={{ ...thStyle, width: "62px", textAlign: "center" }}>
                  삭제
                </th>
              </tr>
            </thead>
            <tbody>
              {draft.components.map((row, idx) => (
                <tr key={idx}>
                  <td style={tdStyle}>
                    <input
                      type="text"
                      list={casDatalistId}
                      aria-label={`구성성분 ${idx + 1} CAS번호`}
                      placeholder="예: 67-64-1"
                      value={row.casNumber}
                      onChange={(e) =>
                        handleComponentCasChange(idx, e.target.value)
                      }
                      style={{
                        ...inputStyle,
                        width: "100%",
                        fontFamily: "var(--mono)",
                      }}
                    />
                  </td>
                  <td style={tdStyle}>
                    <input
                      type="text"
                      aria-label={`구성성분 ${idx + 1} 화학물질명`}
                      placeholder="예: 아세톤 (Acetone)"
                      value={row.substanceName}
                      onChange={(e) =>
                        handleComponentFieldChange(
                          idx,
                          "substanceName",
                          e.target.value,
                        )
                      }
                      style={{ ...inputStyle, width: "100%" }}
                    />
                  </td>
                  <td style={tdStyle}>
                    <input
                      type="text"
                      inputMode="decimal"
                      aria-label={`구성성분 ${idx + 1} 함유율 (%)`}
                      placeholder="0.0000"
                      value={row.weightPctText}
                      onChange={(e) =>
                        handleComponentFieldChange(
                          idx,
                          "weightPctText",
                          e.target.value,
                        )
                      }
                      style={{
                        ...inputStyle,
                        width: "100%",
                        textAlign: "right",
                        fontFamily: "var(--mono)",
                      }}
                    />
                  </td>
                  <td style={{ ...tdStyle, textAlign: "center" }}>
                    <button
                      type="button"
                      onClick={() => handleRemoveComponentRow(idx)}
                      disabled={draft.components.length <= 1}
                      style={{
                        ...smallActionBtnStyle,
                        color: "var(--danger)",
                        borderColor: "#fca5a5",
                        opacity: draft.components.length <= 1 ? 0.5 : 1,
                      }}
                    >
                      삭제
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>

          {feedback && (
            <div
              role="alert"
              style={{
                padding: "8px 11px",
                borderRadius: "5px",
                marginBottom: "10px",
                fontSize: "12px",
                fontWeight: 600,
                background: feedback.ok
                  ? "var(--primary-soft)"
                  : "var(--danger-soft)",
                color: feedback.ok ? "var(--primary)" : "var(--danger)",
                border: `1px solid ${feedback.ok ? "#93c5fd" : "#fca5a5"}`,
              }}
            >
              {feedback.text}
            </div>
          )}

          <div
            style={{
              display: "flex",
              justifyContent: "flex-end",
              gap: "8px",
            }}
          >
            <button
              type="button"
              onClick={handleStartNewProduct}
              disabled={saving}
              style={secondaryBtnStyle}
            >
              새로 입력
            </button>
            <button
              type="submit"
              disabled={!canSubmit}
              style={{
                background: canSubmit ? "var(--primary)" : "#94a3b8",
                color: "#ffffff",
                border: "none",
                borderRadius: "5px",
                padding: "7px 16px",
                fontWeight: 700,
                cursor: canSubmit ? "pointer" : "not-allowed",
              }}
            >
              {saving ? "저장 중..." : "저장"}
            </button>
          </div>
        </form>
      </div>

      <MasterModal
        open={masterTarget !== null}
        kind={masterTarget ?? "overseas_branch"}
        onChanged={() => {
          void refreshAfterMasterChange();
        }}
        onClose={() => {
          setMasterTarget(null);
          void refreshAfterMasterChange();
        }}
      />
    </section>
  );
}

const thStyle: React.CSSProperties = {
  border: "1px solid var(--border-light)",
  padding: "6px 8px",
  textAlign: "left",
  fontWeight: 700,
  color: "#334155",
  whiteSpace: "nowrap",
};

const tdStyle: React.CSSProperties = {
  border: "1px solid var(--border-light)",
  padding: "6px 8px",
  verticalAlign: "middle",
};

const labelStackStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: "4px",
};

const fieldLabelStyle: React.CSSProperties = {
  fontSize: "11.5px",
  fontWeight: 700,
  color: "var(--text-muted)",
};

const inputStyle: React.CSSProperties = {
  padding: "5px 8px",
  border: "1px solid var(--border)",
  borderRadius: "4px",
  fontSize: "12.5px",
};

const smallActionBtnStyle: React.CSSProperties = {
  padding: "3px 8px",
  fontSize: "11.5px",
  fontWeight: 600,
  borderRadius: "4px",
  border: "1px solid var(--border)",
  background: "var(--surface)",
  color: "var(--text)",
  cursor: "pointer",
};

const smallPrimaryBtnStyle: React.CSSProperties = {
  padding: "5px 10px",
  fontSize: "12px",
  fontWeight: 700,
  borderRadius: "4px",
  border: "1px solid var(--primary)",
  background: "var(--primary)",
  color: "#ffffff",
  cursor: "pointer",
};

const secondaryBtnStyle: React.CSSProperties = {
  padding: "6px 12px",
  fontSize: "12.5px",
  fontWeight: 600,
  borderRadius: "5px",
  border: "1px solid var(--border)",
  background: "var(--surface)",
  color: "var(--text)",
  cursor: "pointer",
};
