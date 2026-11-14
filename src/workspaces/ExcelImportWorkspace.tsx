import React, { useEffect, useMemo, useReducer, useRef, useState } from "react";
import { subscribeDbRestoredEvent } from "../lib/ipc";
import {
  type BomProductSummary,
  type ExcelImportClient,
  type ImportCommitResult,
  type ImportKind,
  type ImportPreviewRow,
  type PreviewRowStatus,
  canCommitImport,
  computeEffectiveCommitPlan,
  createExcelImportClient,
  createInitialImportState,
  getConflictToggleCopy,
  reduceImportState,
} from "./excelImportLogic";

export interface ExcelImportWorkspaceProps {
  client?: ExcelImportClient;
  refreshKey?: number;
  onImportCommitted?: (result: ImportCommitResult) => void;
}

function statusBadgeStyle(status: PreviewRowStatus): React.CSSProperties {
  if (status === "error") {
    return {
      background: "var(--danger-soft)",
      color: "var(--danger)",
      border: "1px solid #fca5a5",
      padding: "2px 7px",
      borderRadius: "4px",
      fontWeight: 700,
      fontSize: "11px",
      whiteSpace: "nowrap",
    };
  }
  if (status === "conflict") {
    return {
      background: "var(--warn-soft)",
      color: "var(--warn)",
      border: "1px solid #fcd34d",
      padding: "2px 7px",
      borderRadius: "4px",
      fontWeight: 700,
      fontSize: "11px",
      whiteSpace: "nowrap",
    };
  }
  return {
    background: "var(--success-soft)",
    color: "var(--success)",
    border: "1px solid #86efac",
    padding: "2px 7px",
    borderRadius: "4px",
    fontWeight: 700,
    fontSize: "11px",
    whiteSpace: "nowrap",
  };
}

function statusLabel(kind: ImportKind, status: PreviewRowStatus): string {
  if (status === "error") return "오류";
  if (status === "conflict") {
    return kind === "bom" ? "기존 충돌" : "중복 의심";
  }
  return "정상";
}

function rowBackgroundStyle(status: PreviewRowStatus): React.CSSProperties | undefined {
  if (status === "error") {
    return { backgroundColor: "var(--danger-soft)" };
  }
  if (status === "conflict") {
    return { backgroundColor: "var(--warn-soft)" };
  }
  return undefined;
}

function formatError(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

export function ExcelImportWorkspace({
  client: clientProp,
  refreshKey = 0,
  onImportCommitted,
}: ExcelImportWorkspaceProps) {
  const client = useMemo(
    () => clientProp ?? createExcelImportClient(),
    [clientProp],
  );
  const [state, dispatch] = useReducer(
    reduceImportState,
    "bom",
    createInitialImportState,
  );
  const [restoreTick, setRestoreTick] = useState(0);
  const prevRefreshSigRef = useRef(`${refreshKey}:0`);
  const fileInputRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    return subscribeDbRestoredEvent(() => {
      setRestoreTick((prev) => prev + 1);
    });
  }, []);

  useEffect(() => {
    const nextSig = `${refreshKey}:${restoreTick}`;
    if (prevRefreshSigRef.current === nextSig) {
      return;
    }
    prevRefreshSigRef.current = nextSig;
    dispatch({ type: "CLEAR_PREVIEW" });
  }, [refreshKey, restoreTick]);

  const conflictCopy = getConflictToggleCopy(state.kind);
  const commitAllowed = canCommitImport(state);
  const commitPlanText = computeEffectiveCommitPlan(state.preview, state.options);
  const isBusy = state.busyAction !== "none";

  const handleDownloadTemplate = async (targetKind: ImportKind) => {
    try {
      const savePath = await client.pickTemplateSavePath(targetKind);
      if (!savePath) return;
      dispatch({ type: "TEMPLATE_STARTED" });
      const result = await client.downloadImportTemplate(targetKind, savePath);
      dispatch({ type: "TEMPLATE_SUCCEEDED", result });
    } catch (err) {
      dispatch({ type: "TEMPLATE_FAILED", message: formatError(err) });
    }
  };

  const runPreview = async (filePath: string) => {
    dispatch({ type: "PREVIEW_STARTED", filePath });
    try {
      const preview = await client.previewExcelImport(state.kind, filePath);
      dispatch({ type: "PREVIEW_SUCCEEDED", filePath, preview });
    } catch (err) {
      dispatch({ type: "PREVIEW_FAILED", message: formatError(err) });
    }
  };

  const handlePickFile = async () => {
    if (!client.isTauriRuntime && client.registerInlineCsvFile && fileInputRef.current) {
      fileInputRef.current.value = "";
      fileInputRef.current.click();
      return;
    }
    try {
      const picked = await client.pickImportFile();
      if (!picked) return;
      await runPreview(picked);
    } catch (err) {
      dispatch({ type: "PREVIEW_FAILED", message: formatError(err) });
    }
  };

  const handleBrowserFileSelected = async (
    e: React.ChangeEvent<HTMLInputElement>,
  ) => {
    const file = e.target.files?.[0];
    if (!file) return;
    if (!file.name.toLowerCase().endsWith(".csv")) {
      dispatch({
        type: "PREVIEW_FAILED",
        message:
          "브라우저 단독 미리보기 모드에서는 .csv 파일만 직접 읽을 수 있습니다. (.xlsx/.xls는 데스크톱 앱에서 지원됩니다)",
      });
      return;
    }
    try {
      const text = await file.text();
      const registeredPath =
        client.registerInlineCsvFile?.(file.name, text) ?? file.name;
      await runPreview(registeredPath);
    } catch (err) {
      dispatch({ type: "PREVIEW_FAILED", message: formatError(err) });
    }
  };

  const handleCommit = async () => {
    if (!commitAllowed || !state.selectedFilePath) return;
    dispatch({ type: "COMMIT_STARTED" });
    try {
      const result = await client.commitExcelImport(
        state.kind,
        state.selectedFilePath,
        state.options,
      );
      dispatch({ type: "COMMIT_SUCCEEDED", result });
      onImportCommitted?.(result);
    } catch (err) {
      dispatch({ type: "COMMIT_FAILED", message: formatError(err) });
    }
  };

  return (
    <section
      style={{ display: "flex", flexDirection: "column", gap: "14px" }}
      aria-label="3. 엑셀 일괄 가져오기 작업공간"
    >
      {/* Top Control Card */}
      <div className="card">
        <div className="report-toolbar-top">
          <div>
            <h2 className="card-title" style={{ marginBottom: "4px" }}>
              3. 엑셀 일괄 가져오기
            </h2>
            <p style={{ margin: 0, color: "var(--text-muted)", fontSize: "12px" }}>
              기존 Access 내보내기 파일(.xlsx / .xls / .csv)을 사전 검증 후 단일
              트랜잭션으로 안전하게 적재합니다.
            </p>
          </div>
          <div className="report-action-buttons">
            <button
              type="button"
              className="btn"
              disabled={isBusy}
              onClick={() => void handleDownloadTemplate("bom")}
            >
              제품구성성분_BOM_양식.xlsx 다운로드
            </button>
            <button
              type="button"
              className="btn"
              disabled={isBusy}
              onClick={() => void handleDownloadTemplate("shipments")}
            >
              수입실적_양식.xlsx 다운로드
            </button>
          </div>
        </div>

        {/* Import Type Selector */}
        <div
          className="report-mode-toggle-row"
          role="radiogroup"
          aria-label="가져오기 대상 유형"
          style={{ marginTop: "6px" }}
        >
          <span className="report-mode-toggle-label">1) 가져오기 대상:</span>
          <label
            className={`mode-pill ${state.kind === "bom" ? "active" : ""}`}
          >
            <input
              type="radio"
              name="importKind"
              value="bom"
              checked={state.kind === "bom"}
              disabled={isBusy}
              onChange={() => dispatch({ type: "SET_KIND", kind: "bom" })}
            />
            제품 구성성분(BOM) 일괄 가져오기
          </label>
          <label
            className={`mode-pill ${state.kind === "shipments" ? "active" : ""}`}
          >
            <input
              type="radio"
              name="importKind"
              value="shipments"
              checked={state.kind === "shipments"}
              disabled={isBusy}
              onChange={() =>
                dispatch({ type: "SET_KIND", kind: "shipments" })
              }
            />
            수입실적 일괄 가져오기
          </label>
        </div>

        {/* Conflict / Duplicate Policy Toggles */}
        <div
          className="report-mode-toggle-row"
          role="radiogroup"
          aria-label={conflictCopy.title}
        >
          <span className="report-mode-toggle-label">
            2) {conflictCopy.title}:
          </span>
          {state.kind === "bom" ? (
            <>
              <label
                className={`mode-pill ${state.options.overwrite_existing ? "active" : ""}`}
              >
                <input
                  type="radio"
                  name="bomConflictPolicy"
                  checked={state.options.overwrite_existing}
                  disabled={isBusy}
                  onChange={() =>
                    dispatch({ type: "SET_OVERWRITE_EXISTING", value: true })
                  }
                />
                기존 수입제품 레시피 덮어쓰기 (기본값: 켜짐)
              </label>
              <label
                className={`mode-pill ${!state.options.overwrite_existing ? "active" : ""}`}
              >
                <input
                  type="radio"
                  name="bomConflictPolicy"
                  checked={!state.options.overwrite_existing}
                  disabled={isBusy}
                  onChange={() =>
                    dispatch({ type: "SET_OVERWRITE_EXISTING", value: false })
                  }
                />
                기존 제품 건너뛰기
              </label>
            </>
          ) : (
            <>
              <label
                className={`mode-pill ${state.options.skip_duplicates ? "active" : ""}`}
              >
                <input
                  type="radio"
                  name="shipmentDupPolicy"
                  checked={state.options.skip_duplicates}
                  disabled={isBusy}
                  onChange={() =>
                    dispatch({ type: "SET_SKIP_DUPLICATES", value: true })
                  }
                />
                중복 수입실적 건너뛰기 (기본값: 켜짐)
              </label>
              <label
                className={`mode-pill ${!state.options.skip_duplicates ? "active" : ""}`}
              >
                <input
                  type="radio"
                  name="shipmentDupPolicy"
                  checked={!state.options.skip_duplicates}
                  disabled={isBusy}
                  onChange={() =>
                    dispatch({ type: "SET_SKIP_DUPLICATES", value: false })
                  }
                />
                중복 포함 강제 추가
              </label>
            </>
          )}
        </div>

        {/* File Picker & Commit Bar */}
        <div
          style={{
            display: "flex",
            flexWrap: "wrap",
            alignItems: "center",
            justifyContent: "space-between",
            gap: "10px",
            marginTop: "12px",
            paddingTop: "12px",
            borderTop: "1px solid var(--border-light)",
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: "8px", flexWrap: "wrap" }}>
            <input
              ref={fileInputRef}
              type="file"
              accept=".csv,.xlsx,.xls"
              style={{ display: "none" }}
              onChange={(e) => void handleBrowserFileSelected(e)}
            />
            <button
              type="button"
              className="btn"
              disabled={isBusy}
              onClick={() => void handlePickFile()}
            >
              {state.busyAction === "preview"
                ? "파일 검증 중..."
                : "파일 선택 (.xlsx / .xls / .csv)"}
            </button>
            {state.selectedFilePath && (
              <>
                <span className="mono" style={{ fontSize: "12px", color: "var(--text-muted)" }}>
                  선택 파일: <strong>{state.preview?.file_name ?? state.selectedFilePath}</strong>
                </span>
                <button
                  type="button"
                  className="btn"
                  disabled={isBusy}
                  onClick={() => void runPreview(state.selectedFilePath!)}
                >
                  다시 검증
                </button>
                <button
                  type="button"
                  className="btn"
                  disabled={isBusy}
                  onClick={() => dispatch({ type: "CLEAR_PREVIEW" })}
                >
                  초기화
                </button>
              </>
            )}
          </div>

          <button
            type="button"
            className="btn btn-primary"
            disabled={!commitAllowed}
            onClick={() => void handleCommit()}
          >
            {state.busyAction === "commit"
              ? "DB 적재 중..."
              : "일괄 가져오기 실행"}
          </button>
        </div>

        <p style={{ margin: "8px 0 0", fontSize: "11.5px", color: "var(--text-muted)" }}>
          ※ BOM 양식의 <span className="mono">CAS번호</span> 열은 엑셀 텍스트 서식(
          <span className="mono">@</span>)이 적용되어 <span className="mono">50-00-0</span>{" "}
          등이 날짜로 자동 변환되지 않습니다.
        </p>

        {state.errorMessage && (
          <div className="report-alert report-alert-error" role="alert">
            {state.errorMessage}
          </div>
        )}
        {state.feedbackMessage && (
          <div className="report-alert report-alert-info" role="status">
            {state.feedbackMessage}
          </div>
        )}
      </div>

      {/* Preview Results */}
      {state.preview && (
        <div className="card">
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "repeat(4, minmax(120px, 1fr))",
              gap: "8px",
              marginBottom: "12px",
            }}
          >
            <div className="report-stat-box">
              <span>
                {state.preview.kind === "bom" ? "전체 행 / 제품 수" : "전체 행 수"}
              </span>
              <strong className="mono">
                {state.preview.kind === "bom"
                  ? `${state.preview.total_rows}행 / ${state.preview.total_products}개`
                  : `${state.preview.total_rows}건`}
              </strong>
            </div>
            <div className="report-stat-box">
              <span>정상 (신규)</span>
              <strong className="mono" style={{ color: "var(--success)" }}>
                {state.preview.valid_count}
                {state.preview.kind === "bom" ? "개" : "건"}
              </strong>
            </div>
            <div className="report-stat-box">
              <span>
                {state.preview.kind === "bom" ? "기존 제품 충돌" : "중복 의심"}
              </span>
              <strong className="mono" style={{ color: "var(--warn)" }}>
                {state.preview.conflict_count}
                {state.preview.kind === "bom" ? "개" : "건"}
              </strong>
            </div>
            <div className="report-stat-box">
              <span>검증 오류</span>
              <strong
                className="mono"
                style={{
                  color:
                    state.preview.error_count > 0
                      ? "var(--danger)"
                      : "var(--text)",
                }}
              >
                {state.preview.error_count}행
              </strong>
            </div>
          </div>

          {!state.preview.can_commit ? (
            <div className="report-alert report-alert-error" role="alert" style={{ marginBottom: "12px" }}>
              검증 오류가 {state.preview.error_count}행 발견되어 적재 버튼이
              비활성화되었습니다. 아래 붉은색으로 표시된 행의 오류를 수정한 뒤
              다시 업로드해 주세요.
              {state.preview.unknown_product_codes.length > 0 && (
                <div style={{ marginTop: "4px" }}>
                  미등록 수입제품코드:{" "}
                  <span className="mono">
                    {state.preview.unknown_product_codes.join(", ")}
                  </span>
                </div>
              )}
            </div>
          ) : (
            <div
              className="report-alert report-alert-info"
              role="status"
              style={{ marginBottom: "12px" }}
            >
              사전 검증 통과 — {commitPlanText}
            </div>
          )}

          {/* Product-level summary table for BOM */}
          {state.preview.kind === "bom" &&
            state.preview.bom_products.length > 0 && (
              <div style={{ marginBottom: "14px" }}>
                <h3
                  style={{
                    margin: "0 0 8px 0",
                    fontSize: "13px",
                    fontWeight: 700,
                  }}
                >
                  제품별 구성성분(BOM) 합계 검증
                </h3>
                <div style={{ overflowX: "auto" }}>
                  <table className="report-table">
                    <thead>
                      <tr>
                        <th>상태</th>
                        <th>수입제품코드</th>
                        <th>제품명</th>
                        <th>해외지사</th>
                        <th className="num">성분 수</th>
                        <th className="num">함유율 합계 (%)</th>
                        <th>DB 상태 / 오류 상세</th>
                      </tr>
                    </thead>
                    <tbody>
                      {state.preview.bom_products.map((prod: BomProductSummary) => (
                        <tr
                          key={prod.product_code}
                          style={rowBackgroundStyle(prod.status)}
                        >
                          <td>
                            <span style={statusBadgeStyle(prod.status)}>
                              {statusLabel("bom", prod.status)}
                            </span>
                          </td>
                          <td className="mono">
                            <strong>{prod.product_code}</strong>
                          </td>
                          <td>{prod.product_name ?? "—"}</td>
                          <td>{prod.overseas_branch_name || "—"}</td>
                          <td className="num">{prod.component_count}</td>
                          <td className="num">{prod.weight_pct_sum_display}%</td>
                          <td
                            style={{
                              color:
                                prod.status === "error"
                                  ? "var(--danger)"
                                  : "var(--text-muted)",
                              fontSize: "11.5px",
                            }}
                          >
                            {prod.errors.length > 0
                              ? prod.errors.join(" / ")
                              : prod.exists_in_db
                                ? state.options.overwrite_existing
                                  ? "기존 제품 레시피 덮어쓰기 예정"
                                  : "기존 제품 건너뛰기 예정"
                                : "신규 제품 등록 예정"}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>
            )}

          {/* Row-level validation table */}
          <h3 style={{ margin: "0 0 8px 0", fontSize: "13px", fontWeight: 700 }}>
            행별 상세 미리보기
          </h3>
          <div style={{ overflowX: "auto" }}>
            <table className="report-table">
              <thead>
                {state.preview.kind === "bom" ? (
                  <tr>
                    <th style={{ width: "54px" }}>행</th>
                    <th style={{ width: "78px" }}>상태</th>
                    <th>해외지사</th>
                    <th>수입제품코드</th>
                    <th>제품명(선택)</th>
                    <th>CAS번호</th>
                    <th>화학물질명(선택)</th>
                    <th className="num">함유율(%)</th>
                    <th>검증 오류 / 비고</th>
                  </tr>
                ) : (
                  <tr>
                    <th style={{ width: "54px" }}>행</th>
                    <th style={{ width: "78px" }}>상태</th>
                    <th>수입일자</th>
                    <th>국내 수입사</th>
                    <th>해외지사</th>
                    <th>수입제품코드</th>
                    <th className="num">제품 수입량(kg)</th>
                    <th>비고</th>
                    <th>검증 오류 / 충돌 사유</th>
                  </tr>
                )}
              </thead>
              <tbody>
                {state.preview.rows.map((row: ImportPreviewRow) => (
                  <tr
                    key={`${row.row_number}-${row.product_code}`}
                    style={rowBackgroundStyle(row.status)}
                  >
                    <td className="mono">{row.row_number}</td>
                    <td>
                      <span style={statusBadgeStyle(row.status)}>
                        {statusLabel(state.preview!.kind, row.status)}
                      </span>
                    </td>
                    {state.preview!.kind === "bom" ? (
                      <>
                        <td>{row.overseas_branch_name || "—"}</td>
                        <td className="mono">{row.product_code || "—"}</td>
                        <td>{row.product_name ?? "—"}</td>
                        <td className="mono">{row.cas_number ?? "—"}</td>
                        <td>{row.substance_name ?? "—"}</td>
                        <td className="num">{row.weight_pct_text ?? "—"}</td>
                      </>
                    ) : (
                      <>
                        <td className="mono">{row.shipment_date ?? "—"}</td>
                        <td>{row.domestic_importer_name ?? "—"}</td>
                        <td>{row.overseas_branch_name || "—"}</td>
                        <td className="mono">{row.product_code || "—"}</td>
                        <td className="num">
                          {row.import_weight_kg_text ?? "—"}
                        </td>
                        <td>{row.note ?? "—"}</td>
                      </>
                    )}
                    <td
                      style={{
                        color:
                          row.status === "error"
                            ? "var(--danger)"
                            : row.status === "conflict"
                              ? "var(--warn)"
                              : "var(--text-muted)",
                        fontSize: "11.5px",
                      }}
                    >
                      {row.errors.length > 0
                        ? row.errors.join(" / ")
                        : row.conflict_reason ?? "정상"}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </section>
  );
}
