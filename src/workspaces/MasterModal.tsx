import React, { useCallback, useEffect, useId, useMemo, useState } from "react";
import {
  type MasterEditDraft,
  type MasterEntityKind,
  type MasterItem,
  type MastersIpcClient,
  buildDeleteBlockedReason,
  createMastersIpcClient,
  formatMasterReferenceSummary,
  getMasterEntityMeta,
  itemToEditDraft,
  validateMasterDraft,
} from "./mastersLogic";

export interface MasterModalProps {
  open: boolean;
  kind?: MasterEntityKind;
  ipc?: MastersIpcClient;
  onClose: () => void;
  onChanged?: () => void;
}

export function MasterModal({
  open,
  kind = "domestic_importer",
  ipc: ipcProp,
  onClose,
  onChanged,
}: MasterModalProps) {
  const ipc = useMemo(() => ipcProp ?? createMastersIpcClient(), [ipcProp]);
  const titleId = useId();
  const meta = getMasterEntityMeta(kind);

  const [items, setItems] = useState<MasterItem[]>([]);
  const [searchText, setSearchText] = useState("");
  const [editingId, setEditingId] = useState<number | null>(null);
  const [draft, setDraft] = useState<MasterEditDraft>({
    name: "",
    casNumber: "",
    substanceName: "",
    note: "",
  });
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [feedback, setFeedback] = useState<string | null>(null);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  const loadItems = useCallback(async () => {
    setLoading(true);
    try {
      const list = await ipc.listMasterItems(kind);
      setItems(list);
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [ipc, kind]);

  useEffect(() => {
    if (!open) return;
    setEditingId(null);
    setSearchText("");
    setFeedback(null);
    setErrorMsg(null);
    void loadItems();
  }, [open, kind, loadItems]);

  useEffect(() => {
    if (!open) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        if (editingId !== null) {
          setEditingId(null);
          setErrorMsg(null);
        } else {
          onClose();
        }
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [open, editingId, onClose]);

  const filteredItems = useMemo(() => {
    const q = searchText.trim().toLowerCase();
    if (!q) return items;
    return items.filter((item) => {
      if (kind === "chemical_substance") {
        return (
          (item.cas_number ?? "").toLowerCase().includes(q) ||
          (item.substance_name ?? "").toLowerCase().includes(q)
        );
      }
      return (
        (item.name ?? "").toLowerCase().includes(q) ||
        (item.note ?? "").toLowerCase().includes(q)
      );
    });
  }, [items, kind, searchText]);

  if (!open) return null;

  const handleStartEdit = (item: MasterItem) => {
    setErrorMsg(null);
    setFeedback(null);
    setEditingId(item.id);
    setDraft(itemToEditDraft(item));
  };

  const handleCancelEdit = () => {
    setEditingId(null);
    setErrorMsg(null);
  };

  const handleSaveEdit = async (itemId: number) => {
    setErrorMsg(null);
    setFeedback(null);
    const validated = validateMasterDraft(kind, draft, items, itemId);
    if (!validated.ok) {
      setErrorMsg(validated.error);
      return;
    }

    setSaving(true);
    try {
      const updated = await ipc.updateMasterItem(validated.payload);
      setEditingId(null);
      await loadItems();
      const updatedLabel =
        kind === "chemical_substance"
          ? `${updated.cas_number ?? ""}${updated.substance_name ? ` (${updated.substance_name})` : ""}`
          : (updated.name ?? "");
      setFeedback(`${meta.label} '${updatedLabel}' 정보가 수정되었습니다.`);
      onChanged?.();
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async (item: MasterItem) => {
    setErrorMsg(null);
    setFeedback(null);
    const blockedReason = buildDeleteBlockedReason(item);
    if (blockedReason) {
      setErrorMsg(blockedReason);
      return;
    }

    setSaving(true);
    try {
      const deleted = await ipc.deleteMasterItem(kind, item.id);
      if (editingId === item.id) {
        setEditingId(null);
      }
      await loadItems();
      setFeedback(
        `${meta.label} '${deleted.deleted_label}'이(가) 삭제되었습니다.`,
      );
      onChanged?.();
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const handleEditRowKeyDown = (
    e: React.KeyboardEvent<HTMLInputElement>,
    itemId: number,
  ) => {
    if (e.key === "Enter") {
      e.preventDefault();
      void handleSaveEdit(itemId);
    }
  };

  return (
    <div
      style={styles.backdrop}
      onClick={(e) => {
        if (e.target === e.currentTarget) {
          onClose();
        }
      }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        className="card"
        style={styles.dialog}
      >
        <div style={styles.header}>
          <div>
            <h3 id={titleId} className="card-title" style={{ margin: "0 0 3px 0" }}>
              {meta.title}
            </h3>
            <p style={styles.subtitle}>{meta.subtitle}</p>
          </div>
          <div style={styles.headerRight}>
            <span style={styles.countPill}>총 {items.length}건</span>
            <button type="button" onClick={onClose} style={styles.closeBtn}>
              닫기
            </button>
          </div>
        </div>

        <div style={styles.toolbar}>
          <input
            type="search"
            aria-label="마스터 항목 검색"
            placeholder={
              kind === "chemical_substance"
                ? "CAS번호 또는 화학물질명 검색..."
                : `${meta.label} 또는 비고 검색...`
            }
            value={searchText}
            onChange={(e) => setSearchText(e.target.value)}
            style={styles.searchInput}
          />
          <span style={styles.toolbarHint}>
            참조 중인 항목(1건 이상)은 데이터 무결성 보호(`ON DELETE RESTRICT`)로 삭제가 제한됩니다.
          </span>
        </div>

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

        <div style={styles.tableWrap}>
          <table style={styles.table}>
            <thead>
              <tr style={{ background: "var(--surface-alt)" }}>
                {kind === "chemical_substance" ? (
                  <>
                    <th style={{ ...styles.th, width: 145 }}>CAS번호 *</th>
                    <th style={styles.th}>화학물질명 (선택)</th>
                  </>
                ) : (
                  <>
                    <th style={{ ...styles.th, width: 220 }}>{meta.label}명 *</th>
                    <th style={styles.th}>비고 (선택)</th>
                  </>
                )}
                <th style={{ ...styles.th, width: 175 }}>
                  {meta.referenceColumnLabel}
                </th>
                <th style={{ ...styles.th, width: 135, textAlign: "center" }}>
                  수정 / 삭제
                </th>
              </tr>
            </thead>
            <tbody>
              {loading ? (
                <tr>
                  <td colSpan={4} style={styles.emptyCell}>
                    목록을 불러오는 중...
                  </td>
                </tr>
              ) : filteredItems.length === 0 ? (
                <tr>
                  <td colSpan={4} style={styles.emptyCell}>
                    조건에 맞는 {meta.label} 항목이 없습니다.
                  </td>
                </tr>
              ) : (
                filteredItems.map((item) => {
                  const isEditing = editingId === item.id;
                  const blockedReason = buildDeleteBlockedReason(item);
                  const isReferenced = item.reference_count > 0;
                  return (
                    <tr
                      key={item.id}
                      style={{
                        background: isEditing ? "#eff6ff" : "#ffffff",
                      }}
                    >
                      {kind === "chemical_substance" ? (
                        <>
                          <td style={styles.td}>
                            {isEditing ? (
                              <input
                                type="text"
                                aria-label="CAS번호 수정"
                                value={draft.casNumber}
                                onChange={(e) =>
                                  setDraft((prev) => ({
                                    ...prev,
                                    casNumber: e.target.value,
                                  }))
                                }
                                onKeyDown={(e) => handleEditRowKeyDown(e, item.id)}
                                style={{
                                  ...styles.input,
                                  fontFamily: "var(--mono)",
                                }}
                                autoFocus
                              />
                            ) : (
                              <span
                                style={{
                                  fontFamily: "var(--mono)",
                                  fontWeight: 700,
                                }}
                              >
                                {item.cas_number}
                              </span>
                            )}
                          </td>
                          <td style={styles.td}>
                            {isEditing ? (
                              <input
                                type="text"
                                aria-label="화학물질명 수정"
                                placeholder="화학물질명 (선택)"
                                value={draft.substanceName}
                                onChange={(e) =>
                                  setDraft((prev) => ({
                                    ...prev,
                                    substanceName: e.target.value,
                                  }))
                                }
                                onKeyDown={(e) => handleEditRowKeyDown(e, item.id)}
                                style={styles.input}
                              />
                            ) : item.substance_name ? (
                              item.substance_name
                            ) : (
                              <span style={{ color: "#94a3b8" }}>(미입력)</span>
                            )}
                          </td>
                        </>
                      ) : (
                        <>
                          <td style={styles.td}>
                            {isEditing ? (
                              <input
                                type="text"
                                aria-label={`${meta.label} 이름 수정`}
                                value={draft.name}
                                onChange={(e) =>
                                  setDraft((prev) => ({
                                    ...prev,
                                    name: e.target.value,
                                  }))
                                }
                                onKeyDown={(e) => handleEditRowKeyDown(e, item.id)}
                                style={styles.input}
                                autoFocus
                              />
                            ) : (
                              <strong>{item.name}</strong>
                            )}
                          </td>
                          <td style={styles.td}>
                            {isEditing ? (
                              <input
                                type="text"
                                aria-label={`${meta.label} 비고 수정`}
                                placeholder="비고 (선택)"
                                value={draft.note}
                                onChange={(e) =>
                                  setDraft((prev) => ({
                                    ...prev,
                                    note: e.target.value,
                                  }))
                                }
                                onKeyDown={(e) => handleEditRowKeyDown(e, item.id)}
                                style={styles.input}
                              />
                            ) : item.note ? (
                              item.note
                            ) : (
                              <span style={{ color: "#94a3b8" }}>-</span>
                            )}
                          </td>
                        </>
                      )}

                      <td style={styles.td}>
                        <span
                          style={{
                            ...styles.refBadge,
                            background: isReferenced
                              ? "var(--warn-soft)"
                              : "var(--success-soft)",
                            color: isReferenced
                              ? "var(--warn)"
                              : "var(--success)",
                          }}
                        >
                          {formatMasterReferenceSummary(item)}
                        </span>
                      </td>

                      <td style={{ ...styles.td, textAlign: "center" }}>
                        {isEditing ? (
                          <div style={styles.rowActions}>
                            <button
                              type="button"
                              disabled={saving}
                              onClick={() => void handleSaveEdit(item.id)}
                              style={styles.saveBtn}
                            >
                              저장
                            </button>
                            <button
                              type="button"
                              disabled={saving}
                              onClick={handleCancelEdit}
                              style={styles.secondaryBtn}
                            >
                              취소
                            </button>
                          </div>
                        ) : (
                          <div style={styles.rowActions}>
                            <button
                              type="button"
                              disabled={saving}
                              onClick={() => handleStartEdit(item)}
                              style={styles.secondaryBtn}
                            >
                              수정
                            </button>
                            <button
                              type="button"
                              disabled={saving}
                              aria-disabled={isReferenced}
                              title={
                                blockedReason ??
                                `${meta.label} 삭제 (참조 없음)`
                              }
                              onClick={() => void handleDelete(item)}
                              style={
                                isReferenced
                                  ? styles.guardedDeleteBtn
                                  : styles.deleteBtn
                              }
                            >
                              삭제
                            </button>
                          </div>
                        )}
                      </td>
                    </tr>
                  );
                })
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  backdrop: {
    position: "fixed",
    inset: 0,
    background: "rgba(15, 23, 42, 0.48)",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    padding: 20,
    zIndex: 1000,
  },
  dialog: {
    width: "100%",
    maxWidth: 780,
    maxHeight: "82vh",
    display: "flex",
    flexDirection: "column",
    gap: 10,
    boxShadow: "0 18px 40px rgba(15, 23, 42, 0.25)",
  },
  header: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "flex-start",
    gap: 12,
  },
  subtitle: {
    margin: 0,
    fontSize: 12,
    color: "var(--text-muted)",
  },
  headerRight: {
    display: "flex",
    alignItems: "center",
    gap: 8,
    flexShrink: 0,
  },
  countPill: {
    background: "var(--primary-soft)",
    color: "var(--primary)",
    borderRadius: 999,
    padding: "3px 10px",
    fontSize: 12,
    fontWeight: 700,
  },
  closeBtn: {
    padding: "5px 11px",
    borderRadius: 5,
    border: "1px solid var(--border)",
    background: "#ffffff",
    color: "var(--text)",
    fontSize: 12,
    fontWeight: 600,
    cursor: "pointer",
  },
  toolbar: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: 10,
    flexWrap: "wrap",
  },
  searchInput: {
    flex: "1 1 220px",
    height: 30,
    padding: "4px 9px",
    border: "1px solid var(--border)",
    borderRadius: 4,
    fontSize: 12.5,
  },
  toolbarHint: {
    fontSize: 11.5,
    color: "var(--text-muted)",
  },
  errorBanner: {
    background: "var(--danger-soft)",
    color: "var(--danger)",
    border: "1px solid #fecaca",
    borderRadius: 5,
    padding: "8px 10px",
    fontSize: 12,
    fontWeight: 600,
  },
  successBanner: {
    background: "var(--success-soft)",
    color: "var(--success)",
    border: "1px solid #bbf7d0",
    borderRadius: 5,
    padding: "8px 10px",
    fontSize: 12,
    fontWeight: 600,
  },
  tableWrap: {
    overflowY: "auto",
    border: "1px solid var(--border-light)",
    borderRadius: 5,
    maxHeight: "52vh",
  },
  table: {
    width: "100%",
    borderCollapse: "collapse",
    fontSize: 12.5,
  },
  th: {
    borderBottom: "1px solid var(--border)",
    padding: "7px 9px",
    textAlign: "left",
    fontWeight: 700,
    color: "#334155",
    whiteSpace: "nowrap",
    position: "sticky",
    top: 0,
    background: "var(--surface-alt)",
  },
  td: {
    borderBottom: "1px solid var(--border-light)",
    padding: "6px 9px",
    verticalAlign: "middle",
  },
  emptyCell: {
    textAlign: "center",
    padding: 24,
    color: "var(--text-muted)",
  },
  input: {
    width: "100%",
    padding: "4px 7px",
    border: "1px solid var(--border)",
    borderRadius: 4,
    fontSize: 12.5,
  },
  refBadge: {
    display: "inline-block",
    padding: "2px 8px",
    borderRadius: 999,
    fontSize: 11,
    fontWeight: 700,
    whiteSpace: "nowrap",
  },
  rowActions: {
    display: "inline-flex",
    gap: 5,
    alignItems: "center",
  },
  saveBtn: {
    padding: "4px 9px",
    borderRadius: 4,
    border: "1px solid var(--primary)",
    background: "var(--primary)",
    color: "#ffffff",
    fontSize: 11.5,
    fontWeight: 700,
    cursor: "pointer",
  },
  secondaryBtn: {
    padding: "4px 9px",
    borderRadius: 4,
    border: "1px solid var(--border)",
    background: "#ffffff",
    color: "var(--text)",
    fontSize: 11.5,
    fontWeight: 600,
    cursor: "pointer",
  },
  deleteBtn: {
    padding: "4px 9px",
    borderRadius: 4,
    border: "1px solid #fecaca",
    background: "var(--danger-soft)",
    color: "var(--danger)",
    fontSize: 11.5,
    fontWeight: 600,
    cursor: "pointer",
  },
  guardedDeleteBtn: {
    padding: "4px 9px",
    borderRadius: 4,
    border: "1px solid var(--border-light)",
    background: "var(--surface-alt)",
    color: "#94a3b8",
    fontSize: 11.5,
    fontWeight: 600,
    cursor: "not-allowed",
  },
};
