import type { DbStatus, SeedResult } from "./ipc";

export const APP_TITLE = "화학물질 수입량 집계 시스템 — 코리아 지사";

export type WorkspaceTabId = "bom" | "shipments" | "excel" | "report";

export interface WorkspaceTabDef {
  id: WorkspaceTabId;
  label: string;
}

export const WORKSPACE_TABS: readonly WorkspaceTabDef[] = [
  { id: "bom", label: "1. 수입제품 및 레시피(BOM)" },
  { id: "shipments", label: "2. 수입실적 관리" },
  { id: "excel", label: "3. 엑셀 일괄 가져오기" },
  { id: "report", label: "4. 연간 물질 집계 & A4 보고서" },
] as const;

export interface ShellState {
  activeTab: WorkspaceTabId;
  dbStatus: DbStatus | null;
  seeding: boolean;
  feedbackMessage: string | null;
  errorMessage: string | null;
}

export type ShellAction =
  | { type: "SET_TAB"; tab: WorkspaceTabId }
  | { type: "DB_STATUS_LOADED"; status: DbStatus }
  | { type: "SEED_STARTED" }
  | { type: "SEED_SUCCEEDED"; result: SeedResult }
  | { type: "SEED_FAILED"; message: string };

export function createInitialShellState(): ShellState {
  return {
    activeTab: "bom",
    dbStatus: null,
    seeding: false,
    feedbackMessage: null,
    errorMessage: null,
  };
}

export function reduceShellState(
  state: ShellState,
  action: ShellAction,
): ShellState {
  switch (action.type) {
    case "SET_TAB":
      return { ...state, activeTab: action.tab };
    case "DB_STATUS_LOADED":
      return { ...state, dbStatus: action.status, errorMessage: null };
    case "SEED_STARTED":
      return { ...state, seeding: true, errorMessage: null, feedbackMessage: null };
    case "SEED_SUCCEEDED":
      return {
        ...state,
        seeding: false,
        dbStatus: action.result.status,
        feedbackMessage: action.result.already_seeded
          ? "이미 데이터가 존재하여 샘플 적재를 건너뛰었습니다."
          : `샘플 데이터 적재 완료 (제품 ${action.result.inserted_products}건, 실적 ${action.result.inserted_shipments}건)`,
      };
    case "SEED_FAILED":
      return { ...state, seeding: false, errorMessage: action.message };
  }
}

export function formatDbStatusSummary(status: DbStatus | null): string {
  if (!status) {
    return "SQLite 연결 확인 중...";
  }
  const mode = status.journal_mode.toUpperCase();
  const health = status.integrity_ok ? "정상" : "점검 필요";
  return `${mode} · ${health} | 제품 ${status.imported_products_count} · 실적 ${status.import_shipments_count} · 물질 ${status.chemical_substances_count}`;
}
