import React from "react";
import type { DbStatus } from "../lib/ipc";

export interface DatabaseFooterPanelProps {
  dbStatus: DbStatus | null;
  seeding: boolean;
  feedbackMessage: string | null;
  errorMessage: string | null;
  onSeedSampleData: () => void;
  onBackupSave?: () => void;
  onBackupRestore?: () => void;
}

export function DatabaseFooterPanel({
  seeding,
  feedbackMessage,
  errorMessage,
  onSeedSampleData,
  onBackupSave,
  onBackupRestore,
}: DatabaseFooterPanelProps) {
  return (
    <div className="sidebar-footer">
      <div className="sidebar-footer-title">데이터베이스 관리</div>
      <button
        type="button"
        className="footer-btn"
        onClick={onBackupSave}
        disabled={!onBackupSave}
        title="VACUUM INTO 단일 파일 백업 (Ticket 07에서 연결)"
      >
        DB 백업 저장 (.db)
      </button>
      <button
        type="button"
        className="footer-btn"
        onClick={onBackupRestore}
        disabled={!onBackupRestore}
        title="무결성 검증 후 DB 복원 (Ticket 07에서 연결)"
      >
        DB 백업 불러오기 (복원)
      </button>
      <button
        type="button"
        className="footer-btn footer-btn-accent"
        onClick={onSeedSampleData}
        disabled={seeding}
      >
        {seeding ? "적재 중..." : "샘플 데이터 적재"}
      </button>
      {feedbackMessage && (
        <div className="footer-feedback" role="status">
          {feedbackMessage}
        </div>
      )}
      {errorMessage && (
        <div
          className="footer-feedback"
          style={{ color: "#fca5a5" }}
          role="alert"
        >
          {errorMessage}
        </div>
      )}
    </div>
  );
}
