import React, { useEffect, useMemo, useReducer } from "react";
import { DatabaseFooterPanel } from "./components/DatabaseFooterPanel";
import {
  APP_TITLE,
  WORKSPACE_TABS,
  createInitialShellState,
  formatDbStatusSummary,
  reduceShellState,
} from "./lib/appState";
import { createIpcClient } from "./lib/ipc";
import { AnnualReportWorkspace } from "./workspaces/AnnualReportWorkspace";
import { ExcelImportWorkspace } from "./workspaces/ExcelImportWorkspace";
import { ProductsWorkspace } from "./workspaces/ProductsWorkspace";
import { ShipmentsWorkspace } from "./workspaces/ShipmentsWorkspace";

export function App() {
  const ipc = useMemo(() => createIpcClient(), []);
  const [state, dispatch] = useReducer(
    reduceShellState,
    undefined,
    createInitialShellState,
  );

  useEffect(() => {
    let active = true;
    ipc
      .getDbStatus()
      .then((status) => {
        if (active) {
          dispatch({ type: "DB_STATUS_LOADED", status });
        }
      })
      .catch((err) => {
        if (active) {
          dispatch({
            type: "SEED_FAILED",
            message: err instanceof Error ? err.message : String(err),
          });
        }
      });
    return () => {
      active = false;
    };
  }, [ipc]);

  const handleSeedSampleData = async () => {
    dispatch({ type: "SEED_STARTED" });
    try {
      const result = await ipc.seedSampleData();
      dispatch({ type: "SEED_SUCCEEDED", result });
    } catch (err) {
      dispatch({
        type: "SEED_FAILED",
        message: err instanceof Error ? err.message : String(err),
      });
    }
  };

  return (
    <div className="app-shell">
      <aside className="sidebar" aria-label="주 메뉴">
        <div className="sidebar-top">
          <div className="sidebar-title">{APP_TITLE}</div>

          <div className="db-status-card" aria-live="polite">
            <div className="db-status-badge">
              <span>●</span>
              <span>
                {state.dbStatus?.integrity_ok ? "SQLite 연결됨" : "DB 상태 확인"}
              </span>
            </div>
            <div>{formatDbStatusSummary(state.dbStatus)}</div>
          </div>

          <nav className="nav-list">
            {WORKSPACE_TABS.map((tab) => (
              <button
                key={tab.id}
                type="button"
                className={`nav-btn${state.activeTab === tab.id ? " active" : ""}`}
                onClick={() => dispatch({ type: "SET_TAB", tab: tab.id })}
              >
                {tab.label}
              </button>
            ))}
          </nav>
        </div>

        <DatabaseFooterPanel
          dbStatus={state.dbStatus}
          seeding={state.seeding}
          feedbackMessage={state.feedbackMessage}
          errorMessage={state.errorMessage}
          onSeedSampleData={handleSeedSampleData}
        />
      </aside>

      <main className="workspace-main">
        {state.activeTab === "bom" && <ProductsWorkspace />}
        {state.activeTab === "shipments" && <ShipmentsWorkspace />}
        {state.activeTab === "excel" && <ExcelImportWorkspace />}
        {state.activeTab === "report" && <AnnualReportWorkspace />}
      </main>
    </div>
  );
}

export default App;
