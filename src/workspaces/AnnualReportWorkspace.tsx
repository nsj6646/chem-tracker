import React, { useEffect, useMemo, useState } from "react";
import {
  type AnnualReportFilter,
  type AnnualSubstanceReport,
  type ReportDisplayModel,
  type ReportFilterOptions,
  type ReportMode,
  buildReportDisplayModel,
  createDefaultReportFilter,
  createReportClient,
  validateReportFilter,
} from "./annualReportLogic";

export function AnnualReportSheet({ model }: { model: ReportDisplayModel }) {
  const { report, includeBreakdown, modeLabel, issueDate, rows } = model;

  return (
    <section className="a4-sheet" aria-label="연간 화학물질 수입량 집계 보고서">
      <header className="report-header">
        <div>
          <h2 className="report-title">연간 화학물질 수입량 집계 보고서</h2>
          <p className="report-sub">
            코리아 지사 수입제품 구성성분(BOM) 기반 CAS번호별 환산 중량 집계표
          </p>
        </div>
        <div className="report-header-right">
          <div>
            보고서 구분: <strong>{modeLabel}</strong>
          </div>
          <div>
            발행일자: <span className="mono">{issueDate}</span>
          </div>
        </div>
      </header>

      <div className="report-meta-grid">
        <div className="report-meta-item">
          <strong>대상 기간</strong>
          <span>{report.period_label}</span>
        </div>
        <div className="report-meta-item">
          <strong>국내 수입사</strong>
          <span>{report.domestic_importer_label}</span>
        </div>
        <div className="report-meta-item">
          <strong>해외지사</strong>
          <span>{report.overseas_branch_label}</span>
        </div>
        <div className="report-meta-item">
          <strong>수입제품코드</strong>
          <span>{report.product_label}</span>
        </div>
      </div>

      <div className="report-summary-strip">
        <div className="report-stat-box">
          <span>집계 물질 수 / 수입실적</span>
          <strong>
            {report.unique_substance_count}종 ({report.matching_shipment_count}건)
          </strong>
        </div>
        <div className="report-stat-box">
          <span>수입제품 총 중량</span>
          <strong className="mono">
            {report.total_product_weight_kg_formatted} kg (
            {report.total_product_weight_ton_formatted} ton)
          </strong>
        </div>
        <div className="report-stat-box">
          <span>화학물질 환산 총 중량</span>
          <strong className="mono">
            {report.total_substance_kg_formatted} kg (
            {report.total_substance_ton_formatted} ton)
          </strong>
        </div>
      </div>

      {includeBreakdown ? (
        <div className="report-mode-banner report-mode-internal">
          내부 검증용 상세본 (대외비): 각 CAS번호별 기여 수입실적 및 제품 구성성분
          함유율(%) 산출 근거가 함께 출력됩니다.
        </div>
      ) : (
        <div className="report-mode-banner report-mode-external">
          대외 제출용 요약본 (기본값): 제품별 기밀 구성성분 비율 정보는 제외되며,
          CAS번호 기준 연간 총 수입량(kg / ton)만 출력됩니다.
        </div>
      )}

      <table className="report-table">
        <thead>
          <tr>
            <th style={{ width: "48px", textAlign: "center" }}>NO</th>
            <th style={{ width: "130px" }}>CAS번호</th>
            <th>화학물질명</th>
            <th className="num" style={{ width: "155px" }}>
              연간 총 수입량 (kg)
            </th>
            <th className="num" style={{ width: "145px" }}>
              연간 총 수입량 (ton)
            </th>
          </tr>
        </thead>
        <tbody>
          {rows.length === 0 ? (
            <tr>
              <td colSpan={5} className="report-empty-cell">
                선택한 조건에 해당하는 수입실적 데이터가 없습니다.
              </td>
            </tr>
          ) : (
            rows.map((row) => (
              <React.Fragment key={row.cas_number}>
                <tr>
                  <td className="mono" style={{ textAlign: "center" }}>
                    {row.no}
                  </td>
                  <td className="mono">
                    <strong>{row.cas_number}</strong>
                  </td>
                  <td>{row.substance_name ?? "— (미입력)"}</td>
                  <td className="num">
                    <strong>{row.total_substance_kg_formatted} kg</strong>
                  </td>
                  <td className="num">
                    <strong>{row.total_substance_ton_formatted} ton</strong>
                  </td>
                </tr>
                {includeBreakdown && row.breakdown.length > 0 && (
                  <tr className="report-breakdown-row">
                    <td colSpan={5}>
                      <div className="report-breakdown-box">
                        <div className="report-breakdown-title">
                          ↳ CAS번호 <span className="mono">{row.cas_number}</span>{" "}
                          산출 상세 ({row.breakdown.length}건)
                        </div>
                        <table className="report-subtable">
                          <thead>
                            <tr>
                              <th>수입일자</th>
                              <th>국내 수입사</th>
                              <th>해외지사</th>
                              <th>수입제품코드</th>
                              <th className="num">제품 수입량 (kg)</th>
                              <th className="num">함유율 (%)</th>
                              <th className="num">화학물질 환산 중량 (kg / ton)</th>
                            </tr>
                          </thead>
                          <tbody>
                            {row.breakdown.map((b) => (
                              <tr key={`${row.cas_number}-${b.shipment_id}`}>
                                <td className="mono">{b.shipment_date}</td>
                                <td>{b.domestic_importer_name}</td>
                                <td>{b.overseas_branch_name}</td>
                                <td className="mono">
                                  {b.product_code}
                                  {b.product_name ? ` (${b.product_name})` : ""}
                                </td>
                                <td className="num">
                                  {b.import_weight_kg_formatted} kg
                                </td>
                                <td className="num">{b.weight_pct_formatted}%</td>
                                <td className="num">
                                  <strong>{b.substance_weight_kg_formatted} kg</strong>{" "}
                                  / {b.substance_weight_ton_formatted} ton
                                </td>
                              </tr>
                            ))}
                          </tbody>
                        </table>
                      </div>
                    </td>
                  </tr>
                )}
              </React.Fragment>
            ))
          )}
        </tbody>
        <tfoot>
          <tr>
            <th colSpan={3} style={{ textAlign: "right" }}>
              총 화학물질 환산 수입량 합계
            </th>
            <th className="num">{report.total_substance_kg_formatted} kg</th>
            <th className="num">{report.total_substance_ton_formatted} ton</th>
          </tr>
        </tfoot>
      </table>

      <footer className="report-footer-note">
        <span>
          ※ 고정소수점 정밀 연산: 물질 환산 중량(kg) = 제품 수입량(kg) × 함유율(%)
          ÷ 100 (중간 반올림 없이 합산 후 최종 표기 반올림)
        </span>
        <span>ton 환산 = kg ÷ 1,000</span>
      </footer>
    </section>
  );
}

export function AnnualReportWorkspace() {
  const client = useMemo(() => createReportClient(), []);
  const [options, setOptions] = useState<ReportFilterOptions>({
    years: [2026],
    domestic_importers: [],
    overseas_branches: [],
    imported_products: [],
  });
  const [filter, setFilter] = useState<AnnualReportFilter>(() =>
    createDefaultReportFilter([2026]),
  );
  const [mode, setMode] = useState<ReportMode>("external_summary");
  const [report, setReport] = useState<AnnualSubstanceReport | null>(null);
  const [loading, setLoading] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [feedbackMessage, setFeedbackMessage] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    client
      .getFilterOptions()
      .then((loaded) => {
        if (!active) return;
        setOptions(loaded);
        setFilter((prev) => ({
          ...prev,
          targetYear:
            prev.targetYear !== null && loaded.years.includes(prev.targetYear)
              ? prev.targetYear
              : loaded.years[0] ?? null,
        }));
      })
      .catch((err) => {
        if (active) {
          setErrorMessage(err instanceof Error ? err.message : String(err));
        }
      });
    return () => {
      active = false;
    };
  }, [client]);

  useEffect(() => {
    const validationErr = validateReportFilter(filter);
    if (validationErr) {
      setErrorMessage(validationErr);
      return;
    }

    let active = true;
    setLoading(true);
    setErrorMessage(null);
    client
      .queryAnnualTotals(filter)
      .then((res) => {
        if (active) {
          setReport(res);
          setLoading(false);
        }
      })
      .catch((err) => {
        if (active) {
          setErrorMessage(err instanceof Error ? err.message : String(err));
          setLoading(false);
        }
      });

    return () => {
      active = false;
    };
  }, [client, filter]);

  const handleExportXlsx = async () => {
    const validationErr = validateReportFilter(filter);
    if (validationErr) {
      setErrorMessage(validationErr);
      return;
    }
    setExporting(true);
    setFeedbackMessage(null);
    setErrorMessage(null);
    try {
      const result = await client.exportXlsx(filter, mode);
      setFeedbackMessage(result.message);
    } catch (err) {
      setErrorMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setExporting(false);
    }
  };

  const handlePrintPdf = () => {
    window.print();
  };

  const displayModel = useMemo(
    () => (report ? buildReportDisplayModel(report, mode) : null),
    [report, mode],
  );

  return (
    <div className="report-workspace">
      <section className="card no-print">
        <div className="report-toolbar-top">
          <h2 className="card-title" style={{ marginBottom: 0 }}>
            4. 연간 물질 집계 &amp; A4 보고서
          </h2>
          <div className="report-action-buttons">
            <button
              type="button"
              className="btn"
              onClick={handleExportXlsx}
              disabled={exporting || loading}
            >
              {exporting ? "내보내는 중..." : "엑셀 (.xlsx) 내보내기"}
            </button>
            <button
              type="button"
              className="btn btn-primary"
              onClick={handlePrintPdf}
              disabled={loading || !displayModel}
            >
              A4 PDF 저장 / 인쇄
            </button>
          </div>
        </div>

        <div className="report-filter-grid">
          <label className="report-field">
            <span>기간 방식</span>
            <select
              value={filter.periodMode}
              onChange={(e) =>
                setFilter((prev) => ({
                  ...prev,
                  periodMode: e.target.value as AnnualReportFilter["periodMode"],
                }))
              }
            >
              <option value="year">대상 연도</option>
              <option value="custom_range">시작일 ~ 종료일</option>
            </select>
          </label>

          {filter.periodMode === "year" ? (
            <label className="report-field">
              <span>대상 연도</span>
              <select
                value={filter.targetYear === null ? "ALL" : String(filter.targetYear)}
                onChange={(e) =>
                  setFilter((prev) => ({
                    ...prev,
                    targetYear:
                      e.target.value === "ALL" ? null : Number(e.target.value),
                  }))
                }
              >
                {options.years.map((y) => (
                  <option key={y} value={String(y)}>
                    {y}년
                  </option>
                ))}
                <option value="ALL">전체 기간</option>
              </select>
            </label>
          ) : (
            <>
              <label className="report-field">
                <span>시작일</span>
                <input
                  type="date"
                  value={filter.startDate}
                  onChange={(e) =>
                    setFilter((prev) => ({ ...prev, startDate: e.target.value }))
                  }
                />
              </label>
              <label className="report-field">
                <span>종료일</span>
                <input
                  type="date"
                  value={filter.endDate}
                  onChange={(e) =>
                    setFilter((prev) => ({ ...prev, endDate: e.target.value }))
                  }
                />
              </label>
            </>
          )}

          <label className="report-field">
            <span>국내 수입사</span>
            <select
              value={
                filter.domesticImporterId === null
                  ? "ALL"
                  : String(filter.domesticImporterId)
              }
              onChange={(e) =>
                setFilter((prev) => ({
                  ...prev,
                  domesticImporterId:
                    e.target.value === "ALL" ? null : Number(e.target.value),
                }))
              }
            >
              <option value="ALL">전체 수입사</option>
              {options.domestic_importers.map((imp) => (
                <option key={imp.id} value={String(imp.id)}>
                  {imp.label}
                </option>
              ))}
            </select>
          </label>

          <label className="report-field">
            <span>해외지사</span>
            <select
              value={
                filter.overseasBranchId === null
                  ? "ALL"
                  : String(filter.overseasBranchId)
              }
              onChange={(e) =>
                setFilter((prev) => ({
                  ...prev,
                  overseasBranchId:
                    e.target.value === "ALL" ? null : Number(e.target.value),
                }))
              }
            >
              <option value="ALL">전체</option>
              {options.overseas_branches.map((branch) => (
                <option key={branch.id} value={String(branch.id)}>
                  {branch.label}
                </option>
              ))}
            </select>
          </label>

          <label className="report-field">
            <span>수입제품코드</span>
            <select
              value={filter.productId === null ? "ALL" : String(filter.productId)}
              onChange={(e) =>
                setFilter((prev) => ({
                  ...prev,
                  productId:
                    e.target.value === "ALL" ? null : Number(e.target.value),
                }))
              }
            >
              <option value="ALL">전체</option>
              {options.imported_products.map((prod) => (
                <option key={prod.id} value={String(prod.id)}>
                  {prod.product_code}
                  {prod.product_name ? ` (${prod.product_name})` : ""}
                </option>
              ))}
            </select>
          </label>
        </div>

        <div className="report-mode-toggle-row" role="radiogroup" aria-label="보고서 출력 모드">
          <span className="report-mode-toggle-label">보고서 출력 모드:</span>
          <label className={`mode-pill ${mode === "external_summary" ? "active" : ""}`}>
            <input
              type="radio"
              name="reportMode"
              value="external_summary"
              checked={mode === "external_summary"}
              onChange={() => setMode("external_summary")}
            />
            대외 제출용 요약본 (기본값 · 함유율 제외)
          </label>
          <label className={`mode-pill ${mode === "internal_detail" ? "active" : ""}`}>
            <input
              type="radio"
              name="reportMode"
              value="internal_detail"
              checked={mode === "internal_detail"}
              onChange={() => setMode("internal_detail")}
            />
            내부 검증용 상세본 (수입실적별 함유율 포함)
          </label>
        </div>

        {errorMessage && (
          <div className="report-alert report-alert-error" role="alert">
            {errorMessage}
          </div>
        )}
        {feedbackMessage && (
          <div className="report-alert report-alert-info" role="status">
            {feedbackMessage}
          </div>
        )}
      </section>

      {displayModel ? (
        <AnnualReportSheet model={displayModel} />
      ) : (
        <section className="card no-print">집계 데이터를 불러오는 중입니다...</section>
      )}
    </div>
  );
}
