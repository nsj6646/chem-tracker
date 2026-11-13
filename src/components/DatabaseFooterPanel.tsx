import React, { useMemo, useReducer } from "react";
import type { DbStatus } from "../lib/ipc";
import {
  type BackupIpcAdapter,
  createBackupIpcAdapter,
  createInitialBackupPanelState,
  executeBackupRestore,
  executeBackupSave,
  reduceBackupPanelState,
} from "./backupLogic";

export interface DatabaseFooterPanelProps {
  dbStatus: DbStatus | null;
  seeding: boolean;
  feedbackMessage: string | null;
  errorMessage: string | null;
  onSeedSampleData: () => void;
  onBackupSave?: () => void;
  onBackupRestore?: () => void;
  onDatabaseRestored?: (status: DbStatus) => void;
  backupAdapter?: BackupIpcAdapter;
}

export function DatabaseFooterPanel({
  seeding,
  feedbackMessage,
  errorMessage,
  onSeedSampleData,
  onBackupSave,
  onBackupRestore,
  onDatabaseRestored,
  backupAdapter,
}: DatabaseFooterPanelProps) {
  const adapter = useMemo(
    () => backupAdapter ?? createBackupIpcAdapter(),
    [backupAdapter],
  );
  const [backupState, backupDispatch] = useReducer(
    reduceBackupPanelState,
    undefined,
    createInitialBackupPanelState,
  );

  const isBusy = seeding || backupState.busyAction !== "none";

  const handleBackupSave = () => {
    if (onBackupSave) {
      onBackupSave();
      return;
    }
    void executeBackupSave(adapter, backupDispatch);
  };

  const handleBackupRestore = () => {
    if (onBackupRestore) {
      onBackupRestore();
      return;
    }
    void executeBackupRestore(adapter, backupDispatch, {
      onRestored: onDatabaseRestored,
    });
  };

  const activeFeedback = backupState.feedbackMessage ?? feedbackMessage;
  const activeError = backupState.errorMessage ?? errorMessage;

  return (
    <div className="sidebar-footer">
      <div className="sidebar-footer-title">데이터베이스 관리</div>
      <button
        type="button"
        className="footer-btn"
        onClick={handleBackupSave}
        disabled={isBusy}
        title="VACUUM INTO 단일 파일 백업 저장"
      >
        {backupState.busyAction === "backup"
          ? "백업 저장 중..."
          : "DB 백업 저장 (.db)"}
      </button>
      <button
        type="button"
        className="footer-btn"
        onClick={handleBackupRestore}
        disabled={isBusy}
        title="PRAGMA integrity_check 검증 후 DB 복원"
      >
        {backupState.busyAction === "restore"
          ? "복원 검증 중..."
          : "DB 백업 불러오기 (복원)"}
      </button>
      <button
        type="button"
        className="footer-btn footer-btn-accent"
        onClick={onSeedSampleData}
        disabled={isBusy}
      >
        {seeding ? "적재 중..." : "샘플 데이터 적재"}
      </button>
      {activeFeedback && (
        <div className="footer-feedback" role="status">
          {activeFeedback}
        </div>
      )}
      {activeError && (
        <div
          className="footer-feedback"
          style={{ color: "#fca5a5" }}
          role="alert"
        >
          {activeError}
        </div>
      )}
    </div>
  );
}
