import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { subscribeDbRestoredEvent } from "../lib/ipc";
import { MasterModal } from "./MasterModal";
import {
  computeLiveWeightPreview,
  computeVirtualWindow,
  createInitialShipmentForm,
  createShipmentsClient,
  loadShipmentIntoForm,
  resolveSelectedProduct,
  validateShipmentForm,
  type DomesticImporterOption,
  type OverseasBranchOption,
  type ProductOption,
  type ShipmentFilter,
  type ShipmentFormState,
  type ShipmentQueryResult,
  type ShipmentRecord,
  type ShipmentsClient,
} from "./shipmentsLogic";

const VIRTUAL_ROW_HEIGHT = 40;
const VIRTUAL_VIEWPORT_HEIGHT = 520;
const VIRTUAL_OVERSCAN = 8;

export interface ShipmentsWorkspaceProps {
  client?: ShipmentsClient;
  refreshKey?: number;
  onOpenMasterModal?: (target: "domestic_importer") => void;
}

const DEFAULT_FILTER: ShipmentFilter = {
  year: null,
  domestic_importer_id: null,
  overseas_branch_id: null,
  search_text: "",
};

export function ShipmentsWorkspace({
  client,
  refreshKey = 0,
  onOpenMasterModal,
}: ShipmentsWorkspaceProps) {
  const api = useMemo(() => client ?? createShipmentsClient(), [client]);
  const [restoreTick, setRestoreTick] = useState(0);

  const [importers, setImporters] = useState<DomesticImporterOption[]>([]);
  const [products, setProducts] = useState<ProductOption[]>([]);
  const [queryResult, setQueryResult] = useState<ShipmentQueryResult>({
    rows: [],
    summary: {
      total_count: 0,
      total_weight_kg_text: "0.00",
      total_weight_ton_text: "0.0000",
    },
    available_years: [],
  });
  const [filter, setFilter] = useState<ShipmentFilter>(DEFAULT_FILTER);
  const [form, setForm] = useState<ShipmentFormState>(() =>
    createInitialShipmentForm(),
  );
  const [saving, setSaving] = useState(false);
  const [loading, setLoading] = useState(true);
  const [feedback, setFeedback] = useState<string | null>(null);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [masterOpen, setMasterOpen] = useState(false);
  const [scrollTop, setScrollTop] = useState(0);

  const dateInputRef = useRef<HTMLInputElement | null>(null);
  const scrollContainerRef = useRef<HTMLDivElement | null>(null);

  const branchOptions = useMemo<OverseasBranchOption[]>(() => {
    const map = new Map<number, string>();
    for (const p of products) {
      map.set(p.overseas_branch_id, p.overseas_branch_name);
    }
    return Array.from(map.entries())
      .map(([id, name]) => ({ id, name }))
      .sort((a, b) => a.name.localeCompare(b.name, "ko"));
  }, [products]);

  const selectedProduct = useMemo(
    () => resolveSelectedProduct(products, form.productId),
    [products, form.productId],
  );

  const livePreview = useMemo(
    () => computeLiveWeightPreview(form.importWeightKgInput),
    [form.importWeightKgInput],
  );

  const refreshAll = useCallback(
    async (activeFilter: ShipmentFilter) => {
      setLoading(true);
      try {
        const [impList, prodList, qRes] = await Promise.all([
          api.listDomesticImporters(),
          api.listProductOptions(),
          api.queryShipments(activeFilter),
        ]);
        setImporters(impList);
        setProducts(prodList);
        setQueryResult(qRes);
        setForm((prev) => {
          const validSelected =
            prev.productId !== null &&
            prodList.some((p) => p.product_id === prev.productId);
          return {
            ...prev,
            productId: validSelected ? prev.productId : prodList[0]?.product_id ?? null,
          };
        });
      } catch (err) {
        setErrorMsg(err instanceof Error ? err.message : String(err));
      } finally {
        setLoading(false);
      }
    },
    [api],
  );

  useEffect(() => {
    return subscribeDbRestoredEvent(() => {
      setRestoreTick((prev) => prev + 1);
    });
  }, []);

  useEffect(() => {
    setScrollTop(0);
    if (scrollContainerRef.current) {
      scrollContainerRef.current.scrollTop = 0;
    }
    void refreshAll(filter);
  }, [refreshAll, filter, refreshKey, restoreTick]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setErrorMsg(null);
    setFeedback(null);

    const validated = validateShipmentForm(form, products);
    if (!validated.ok) {
      setErrorMsg(validated.error);
      return;
    }

    setSaving(true);
    try {
      const saved = await api.saveShipment(validated.payload);
      const isEdit = form.editingId !== null;
      setFeedback(
        isEdit
          ? `수입실적 #${saved.id} 수정됨: ${saved.shipment_date} | ${saved.domestic_importer_name} | ${saved.product_code} (${saved.import_weight_kg_display} kg)`
          : `신규 수입실적 등록됨: ${saved.shipment_date} | ${saved.domestic_importer_name} | ${saved.product_code} (${saved.import_weight_kg_display} kg)`,
      );

      setForm((prev) => ({
        editingId: null,
        shipmentDate: prev.shipmentDate,
        domesticImporterName: prev.domesticImporterName,
        productId: prev.productId,
        importWeightKgInput: "",
        note: "",
      }));
      await refreshAll(filter);
      dateInputRef.current?.focus();
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const handleEditRow = (row: ShipmentRecord) => {
    setErrorMsg(null);
    setFeedback(null);
    setForm(loadShipmentIntoForm(row));
    dateInputRef.current?.focus();
  };

  const handleCancelEdit = () => {
    setErrorMsg(null);
    setForm((prev) => ({
      ...createInitialShipmentForm(prev.shipmentDate),
      productId: products[0]?.product_id ?? null,
    }));
  };

  const handleDeleteRow = async (row: ShipmentRecord) => {
    setErrorMsg(null);
    setFeedback(null);
    try {
      await api.deleteShipment(row.id);
      if (form.editingId === row.id) {
        handleCancelEdit();
      }
      setFeedback(
        `수입실적 삭제됨: ${row.shipment_date} | ${row.domestic_importer_name} | ${row.product_code}`,
      );
      await refreshAll(filter);
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    }
  };

  const handleOpenMaster = () => {
    if (onOpenMasterModal) {
      onOpenMasterModal("domestic_importer");
    } else {
      setMasterOpen(true);
    }
  };

  const virtualWindow = useMemo(
    () =>
      computeVirtualWindow({
        count: queryResult.rows.length,
        estimateSize: VIRTUAL_ROW_HEIGHT,
        overscan: VIRTUAL_OVERSCAN,
        scrollOffset: scrollTop,
        viewportHeight: VIRTUAL_VIEWPORT_HEIGHT,
      }),
    [queryResult.rows.length, scrollTop],
  );

  const visibleItems = virtualWindow.virtualItems;
  const topSpacerHeight =
    visibleItems.length > 0 ? visibleItems[0].start : 0;
  const bottomSpacerHeight =
    visibleItems.length > 0
      ? Math.max(
          0,
          virtualWindow.totalSize - visibleItems[visibleItems.length - 1].end,
        )
      : 0;

  return (
    <section className="card" aria-label="2. 수입실적 관리">
      <div style={styles.headerRow}>
        <div>
          <h2 className="card-title" style={{ marginBottom: 2 }}>
            2. 수입실적 관리
          </h2>
          <div style={styles.subtitle}>
            수입일자 · 국내 수입사 · 수입제품코드(해외지사 자동연결) · 제품 수입량(kg / ton) 빠른 입력 및 가상화 이력 조회
          </div>
        </div>
        <span style={styles.countPill}>
          조회 결과 {queryResult.summary.total_count.toLocaleString()}건
        </span>
      </div>

      {/* Top Quick-Entry / Edit Form Bar (Tab -> Enter flow) */}
      <form
        onSubmit={handleSubmit}
        style={styles.entryForm}
        aria-label="수입실적 빠른 입력 폼"
      >
        <div style={styles.formField}>
          <label htmlFor="shipment-date-input" style={styles.label}>
            수입일자
          </label>
          <input
            ref={dateInputRef}
            id="shipment-date-input"
            type="date"
            value={form.shipmentDate}
            onChange={(e) =>
              setForm((prev) => ({ ...prev, shipmentDate: e.target.value }))
            }
            style={{ ...styles.input, width: 145, fontFamily: "var(--mono)" }}
            required
          />
        </div>

        <div style={styles.formField}>
          <div style={styles.labelRow}>
            <label htmlFor="shipment-importer-input" style={styles.label}>
              국내 수입사 (신규 입력 가능)
            </label>
            <button
              type="button"
              onClick={handleOpenMaster}
              style={styles.inlineLinkBtn}
            >
              [수입사 관리/수정]
            </button>
          </div>
          <input
            id="shipment-importer-input"
            list="domestic-importers-datalist"
            placeholder="예: (주)대한케미칼"
            value={form.domesticImporterName}
            onChange={(e) =>
              setForm((prev) => ({
                ...prev,
                domesticImporterName: e.target.value,
              }))
            }
            style={{ ...styles.input, width: 190 }}
            required
          />
          <datalist id="domestic-importers-datalist">
            {importers.map((imp) => (
              <option key={imp.id} value={imp.name} />
            ))}
          </datalist>
        </div>

        <div style={styles.formField}>
          <label htmlFor="shipment-product-select" style={styles.label}>
            수입제품코드
          </label>
          <select
            id="shipment-product-select"
            value={form.productId ?? ""}
            onChange={(e) =>
              setForm((prev) => ({
                ...prev,
                productId: e.target.value ? Number(e.target.value) : null,
              }))
            }
            style={{ ...styles.input, minWidth: 230 }}
            required
          >
            <option value="">-- 수입제품 선택 --</option>
            {products.map((p) => (
              <option key={p.product_id} value={p.product_id}>
                {p.product_code}
                {p.product_name ? ` (${p.product_name})` : ""}
              </option>
            ))}
          </select>
        </div>

        <div style={styles.formField}>
          <label htmlFor="shipment-branch-locked" style={styles.label}>
            해외지사 (제품 연동 잠금)
          </label>
          <input
            id="shipment-branch-locked"
            type="text"
            readOnly
            tabIndex={-1}
            value={selectedProduct?.overseas_branch_name ?? "제품 선택 시 자동 연결"}
            style={{ ...styles.input, ...styles.lockedInput, width: 155 }}
            aria-readonly="true"
          />
        </div>

        <div style={styles.formField}>
          <div style={styles.labelRow}>
            <label htmlFor="shipment-weight-input" style={styles.label}>
              제품 수입량 (kg)
            </label>
            <span
              style={{
                fontSize: 11,
                fontFamily: "var(--mono)",
                color: livePreview.valid ? "var(--primary)" : "var(--text-muted)",
                fontWeight: 600,
              }}
            >
              제품 환산 중량 (ton): {livePreview.valid ? `${livePreview.tonText} ton` : "-"}
            </span>
          </div>
          <input
            id="shipment-weight-input"
            type="text"
            inputMode="decimal"
            placeholder="예: 1250.5"
            value={form.importWeightKgInput}
            onChange={(e) =>
              setForm((prev) => ({
                ...prev,
                importWeightKgInput: e.target.value,
              }))
            }
            style={{
              ...styles.input,
              width: 145,
              textAlign: "right",
              fontFamily: "var(--mono)",
            }}
            required
          />
        </div>

        <div style={{ ...styles.formField, flex: "1 1 160px" }}>
          <label htmlFor="shipment-note-input" style={styles.label}>
            비고 (선택)
          </label>
          <input
            id="shipment-note-input"
            type="text"
            placeholder="메모 입력 (선택)"
            value={form.note}
            onChange={(e) =>
              setForm((prev) => ({ ...prev, note: e.target.value }))
            }
            style={{ ...styles.input, width: "100%" }}
          />
        </div>

        <div style={styles.formActions}>
          <button type="submit" disabled={saving} style={styles.primaryBtn}>
            {form.editingId !== null ? "수정 저장" : "+ 수입실적 등록"}
          </button>
          {form.editingId !== null && (
            <button
              type="button"
              onClick={handleCancelEdit}
              style={styles.secondaryBtn}
            >
              취소
            </button>
          )}
        </div>
      </form>

      {errorMsg && (
        <div role="alert" style={styles.errorBanner}>
          {errorMsg}
        </div>
      )}
      {feedback && (
        <div role="status" style={styles.successBanner}>
          {feedback}
        </div>
      )}

      {/* Filter & Summary Strip */}
      <div style={styles.filterStrip}>
        <div style={styles.filterControls}>
          <label style={styles.filterItem}>
            <span style={styles.filterLabel}>연도</span>
            <select
              aria-label="연도 필터"
              value={filter.year ?? ""}
              onChange={(e) =>
                setFilter((prev) => ({
                  ...prev,
                  year: e.target.value ? Number(e.target.value) : null,
                }))
              }
              style={styles.filterSelect}
            >
              <option value="">전체 연도</option>
              {queryResult.available_years.map((yr) => (
                <option key={yr} value={yr}>
                  {yr}년
                </option>
              ))}
            </select>
          </label>

          <label style={styles.filterItem}>
            <span style={styles.filterLabel}>국내 수입사</span>
            <select
              aria-label="국내 수입사 필터"
              value={filter.domestic_importer_id ?? ""}
              onChange={(e) =>
                setFilter((prev) => ({
                  ...prev,
                  domestic_importer_id: e.target.value
                    ? Number(e.target.value)
                    : null,
                }))
              }
              style={styles.filterSelect}
            >
              <option value="">전체 수입사</option>
              {importers.map((imp) => (
                <option key={imp.id} value={imp.id}>
                  {imp.name}
                </option>
              ))}
            </select>
          </label>

          <label style={styles.filterItem}>
            <span style={styles.filterLabel}>해외지사</span>
            <select
              aria-label="해외지사 필터"
              value={filter.overseas_branch_id ?? ""}
              onChange={(e) =>
                setFilter((prev) => ({
                  ...prev,
                  overseas_branch_id: e.target.value
                    ? Number(e.target.value)
                    : null,
                }))
              }
              style={styles.filterSelect}
            >
              <option value="">전체 지사</option>
              {branchOptions.map((b) => (
                <option key={b.id} value={b.id}>
                  {b.name}
                </option>
              ))}
            </select>
          </label>

          <label style={styles.filterItem}>
            <span style={styles.filterLabel}>수입제품코드 / 제품명</span>
            <input
              aria-label="수입제품코드 검색"
              type="search"
              placeholder="제품코드 또는 제품명 검색"
              value={filter.search_text}
              onChange={(e) =>
                setFilter((prev) => ({ ...prev, search_text: e.target.value }))
              }
              style={{ ...styles.filterSelect, minWidth: 185 }}
            />
          </label>

          <button
            type="button"
            onClick={() => setFilter(DEFAULT_FILTER)}
            style={styles.secondaryBtn}
          >
            필터 초기화
          </button>
        </div>

        <div style={styles.summaryStrip} aria-live="polite">
          <div style={styles.summaryMetric}>
            <span style={styles.summaryLabel}>건수</span>
            <strong style={styles.summaryValue}>
              {queryResult.summary.total_count.toLocaleString()}건
            </strong>
          </div>
          <div style={styles.summaryMetric}>
            <span style={styles.summaryLabel}>합계 (kg)</span>
            <strong style={styles.summaryValue}>
              {queryResult.summary.total_weight_kg_text} kg
            </strong>
          </div>
          <div style={styles.summaryMetric}>
            <span style={styles.summaryLabel}>합계 (ton)</span>
            <strong style={{ ...styles.summaryValue, color: "var(--primary)" }}>
              {queryResult.summary.total_weight_ton_text} ton
            </strong>
          </div>
        </div>
      </div>

      {/* Virtualized Shipment History Grid */}
      <div
        ref={scrollContainerRef}
        onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}
        style={styles.tableViewport}
        aria-label="수입실적 가상화 목록"
      >
        <table style={styles.table}>
          <thead style={styles.thead}>
            <tr>
              <th style={{ ...styles.th, width: 105 }}>수입일자</th>
              <th style={{ ...styles.th, width: 160 }}>국내 수입사</th>
              <th style={{ ...styles.th, width: 140 }}>해외지사</th>
              <th style={styles.th}>수입제품코드 (제품명)</th>
              <th style={{ ...styles.th, width: 135, textAlign: "right" }}>
                제품 수입량 (kg)
              </th>
              <th style={{ ...styles.th, width: 125, textAlign: "right" }}>
                제품 환산 중량 (ton)
              </th>
              <th style={{ ...styles.th, width: 160 }}>비고</th>
              <th style={{ ...styles.th, width: 115, textAlign: "center" }}>
                수정 / 삭제
              </th>
            </tr>
          </thead>
          <tbody>
            {queryResult.rows.length === 0 ? (
              <tr>
                <td colSpan={8} style={styles.emptyCell}>
                  {loading
                    ? "수입실적을 불러오는 중..."
                    : "조건에 해당하는 수입실적이 없습니다."}
                </td>
              </tr>
            ) : (
              <>
                {topSpacerHeight > 0 && (
                  <tr aria-hidden="true">
                    <td
                      colSpan={8}
                      style={{ height: topSpacerHeight, padding: 0, border: "none" }}
                    />
                  </tr>
                )}
                {visibleItems.map((item) => {
                  const row = queryResult.rows[item.index];
                  const isEditing = form.editingId === row.id;
                  return (
                    <tr
                      key={row.id}
                      data-index={item.index}
                      style={{
                        height: VIRTUAL_ROW_HEIGHT,
                        background: isEditing
                          ? "var(--primary-soft)"
                          : item.index % 2 === 1
                            ? "var(--surface-alt)"
                            : "var(--surface)",
                      }}
                    >
                      <td style={{ ...styles.td, fontFamily: "var(--mono)" }}>
                        {row.shipment_date}
                      </td>
                      <td style={{ ...styles.td, fontWeight: 600 }}>
                        {row.domestic_importer_name}
                      </td>
                      <td style={styles.td}>{row.overseas_branch_name}</td>
                      <td style={styles.td}>
                        <span style={{ fontFamily: "var(--mono)", fontWeight: 600 }}>
                          {row.product_code}
                        </span>
                        {row.product_name ? (
                          <span style={{ color: "var(--text-muted)" }}>
                            {" "}
                            ({row.product_name})
                          </span>
                        ) : null}
                      </td>
                      <td
                        style={{
                          ...styles.td,
                          textAlign: "right",
                          fontFamily: "var(--mono)",
                        }}
                      >
                        {row.import_weight_kg_display}
                      </td>
                      <td
                        style={{
                          ...styles.td,
                          textAlign: "right",
                          fontFamily: "var(--mono)",
                          color: "var(--text-muted)",
                        }}
                      >
                        {row.import_weight_ton_display}
                      </td>
                      <td
                        style={{
                          ...styles.td,
                          color: "var(--text-muted)",
                          maxWidth: 160,
                          overflow: "hidden",
                          textOverflow: "ellipsis",
                          whiteSpace: "nowrap",
                        }}
                        title={row.note ?? undefined}
                      >
                        {row.note ?? "-"}
                      </td>
                      <td style={{ ...styles.td, textAlign: "center" }}>
                        <div style={styles.rowActions}>
                          <button
                            type="button"
                            onClick={() => handleEditRow(row)}
                            style={styles.rowEditBtn}
                          >
                            수정
                          </button>
                          <button
                            type="button"
                            onClick={() => void handleDeleteRow(row)}
                            style={styles.rowDeleteBtn}
                          >
                            삭제
                          </button>
                        </div>
                      </td>
                    </tr>
                  );
                })}
                {bottomSpacerHeight > 0 && (
                  <tr aria-hidden="true">
                    <td
                      colSpan={8}
                      style={{
                        height: bottomSpacerHeight,
                        padding: 0,
                        border: "none",
                      }}
                    />
                  </tr>
                )}
              </>
            )}
          </tbody>
        </table>
      </div>

      <MasterModal
        open={masterOpen}
        kind="domestic_importer"
        onChanged={() => {
          void refreshAll(filter);
        }}
        onClose={() => {
          setMasterOpen(false);
          void refreshAll(filter);
        }}
      />
    </section>
  );
}

const styles: Record<string, React.CSSProperties> = {
  headerRow: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "flex-start",
    gap: 12,
    marginBottom: 12,
  },
  subtitle: {
    fontSize: 12,
    color: "var(--text-muted)",
  },
  countPill: {
    background: "var(--primary-soft)",
    color: "var(--primary)",
    borderRadius: 999,
    padding: "3px 10px",
    fontSize: 12,
    fontWeight: 700,
    whiteSpace: "nowrap",
  },
  entryForm: {
    display: "flex",
    flexWrap: "wrap",
    gap: 10,
    alignItems: "flex-end",
    background: "var(--surface-alt)",
    border: "1px solid var(--border-light)",
    borderRadius: 6,
    padding: 12,
    marginBottom: 10,
  },
  formField: {
    display: "flex",
    flexDirection: "column",
    gap: 4,
  },
  labelRow: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    gap: 8,
  },
  label: {
    fontSize: 11.5,
    fontWeight: 600,
    color: "var(--text-muted)",
  },
  inlineLinkBtn: {
    background: "transparent",
    border: "none",
    padding: 0,
    color: "var(--primary)",
    cursor: "pointer",
    fontSize: 11,
    fontWeight: 700,
    fontFamily: "var(--font)",
  },
  input: {
    height: 32,
    padding: "4px 8px",
    borderRadius: 4,
    border: "1px solid var(--border)",
    background: "#ffffff",
    color: "var(--text)",
    fontFamily: "var(--font)",
    fontSize: 12.5,
  },
  lockedInput: {
    background: "#eef2f7",
    color: "var(--text-muted)",
    cursor: "not-allowed",
  },
  formActions: {
    display: "flex",
    gap: 6,
    alignItems: "center",
  },
  primaryBtn: {
    height: 32,
    padding: "0 14px",
    borderRadius: 4,
    border: "none",
    background: "var(--primary)",
    color: "#ffffff",
    fontFamily: "var(--font)",
    fontSize: 12.5,
    fontWeight: 700,
    cursor: "pointer",
  },
  secondaryBtn: {
    height: 30,
    padding: "0 10px",
    borderRadius: 4,
    border: "1px solid var(--border)",
    background: "#ffffff",
    color: "var(--text)",
    fontFamily: "var(--font)",
    fontSize: 12,
    fontWeight: 600,
    cursor: "pointer",
  },
  errorBanner: {
    background: "var(--danger-soft)",
    color: "var(--danger)",
    border: "1px solid #fecaca",
    borderRadius: 5,
    padding: "8px 10px",
    fontSize: 12.5,
    fontWeight: 600,
    marginBottom: 10,
  },
  successBanner: {
    background: "var(--success-soft)",
    color: "var(--success)",
    border: "1px solid #bbf7d0",
    borderRadius: 5,
    padding: "8px 10px",
    fontSize: 12.5,
    fontWeight: 600,
    marginBottom: 10,
  },
  filterStrip: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "flex-end",
    flexWrap: "wrap",
    gap: 12,
    padding: "10px 12px",
    background: "#ffffff",
    border: "1px solid var(--border-light)",
    borderRadius: 6,
    marginBottom: 10,
  },
  filterControls: {
    display: "flex",
    flexWrap: "wrap",
    alignItems: "flex-end",
    gap: 8,
  },
  filterItem: {
    display: "flex",
    flexDirection: "column",
    gap: 3,
  },
  filterLabel: {
    fontSize: 11,
    color: "var(--text-muted)",
    fontWeight: 600,
  },
  filterSelect: {
    height: 30,
    padding: "3px 8px",
    borderRadius: 4,
    border: "1px solid var(--border)",
    background: "#ffffff",
    color: "var(--text)",
    fontFamily: "var(--font)",
    fontSize: 12,
  },
  summaryStrip: {
    display: "flex",
    gap: 16,
    alignItems: "center",
    background: "var(--surface-alt)",
    border: "1px solid var(--border-light)",
    borderRadius: 5,
    padding: "6px 12px",
  },
  summaryMetric: {
    display: "flex",
    alignItems: "baseline",
    gap: 6,
  },
  summaryLabel: {
    fontSize: 11.5,
    color: "var(--text-muted)",
  },
  summaryValue: {
    fontFamily: "var(--mono)",
    fontSize: 13,
  },
  tableViewport: {
    height: VIRTUAL_VIEWPORT_HEIGHT,
    overflowY: "auto",
    border: "1px solid var(--border)",
    borderRadius: 6,
    background: "#ffffff",
  },
  table: {
    width: "100%",
    borderCollapse: "collapse",
    fontSize: 12.5,
  },
  thead: {
    position: "sticky",
    top: 0,
    zIndex: 1,
    background: "#f1f5f9",
  },
  th: {
    textAlign: "left",
    padding: "8px 10px",
    borderBottom: "1px solid var(--border)",
    fontWeight: 700,
    fontSize: 12,
    color: "var(--text)",
    whiteSpace: "nowrap",
  },
  td: {
    padding: "7px 10px",
    borderBottom: "1px solid var(--border-light)",
    verticalAlign: "middle",
  },
  emptyCell: {
    textAlign: "center",
    padding: 28,
    color: "var(--text-muted)",
  },
  rowActions: {
    display: "inline-flex",
    gap: 4,
  },
  rowEditBtn: {
    padding: "3px 8px",
    borderRadius: 4,
    border: "1px solid var(--border)",
    background: "#ffffff",
    color: "var(--text)",
    cursor: "pointer",
    fontSize: 11.5,
    fontWeight: 600,
    fontFamily: "var(--font)",
  },
  rowDeleteBtn: {
    padding: "3px 8px",
    borderRadius: 4,
    border: "1px solid #fecaca",
    background: "var(--danger-soft)",
    color: "var(--danger)",
    cursor: "pointer",
    fontSize: 11.5,
    fontWeight: 600,
    fontFamily: "var(--font)",
  },
};
