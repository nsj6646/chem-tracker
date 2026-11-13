import React from "react";

export interface MasterModalProps {
  open: boolean;
  onClose: () => void;
}

export function MasterModal({ open, onClose }: MasterModalProps) {
  if (!open) return null;
  return (
    <div role="dialog" aria-modal="true" className="card">
      <h3 className="card-title">마스터 항목 관리/수정</h3>
      <p>국내 수입사, 해외지사, 화학물질 관리 모달(Ticket 04) 영역입니다.</p>
      <button type="button" onClick={onClose}>
        닫기
      </button>
    </div>
  );
}
