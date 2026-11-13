# Evaluate PDF/Print Report Generation & Excel Export Stack

Type: `research`
Label: `wayfinder:research`
Status: resolved

## Question

How should the application generate fixed-layout printable reports (PDF & Windows printing with full Korean font/CJK support) and Excel (`.xlsx`) exports rapidly on a low-spec Windows 11 PC?

Investigate against primary sources:

1. **PDF Generation & Printing (with Korean CJK Font Support)**:
   - **Typst** (embedded as a Rust library or CLI, or `typst-as-lib`): binary size, speed, memory footprint, table/page-break layout ergonomics, and Korean font embedding.
   - **Pure Rust PDF libraries** (`genpdf`, `printpdf`, `krilla`, `pdf-writer`): maturity of table layouts, headers/footers, and CJK/Korean glyph shaping.
   - **WebView2 / HTML-to-Print** (if using Tauri v2): CSS `@media print` and native Windows print dialog / Save-as-PDF via WebView2 APIs.
   - **Direct Windows Printing**: how a native Rust app triggers the Windows 11 print dialog or opens a generated PDF in the default viewer (e.g., Edge / SumatraPDF) for one-click printing.
2. **Excel (`.xlsx`) Export**:
   - Evaluate `rust_xlsxwriter` (and `calamine` if reading Excel is needed): performance, memory usage, styling, column widths, and compatibility with Korean Excel on Windows 11.

Save the detailed findings with primary source citations to `.scratch/access-replacement/research/03-reporting-pdf-print-and-excel.md`.

## Answer

Detailed findings and primary source citations are recorded in [03-reporting-pdf-print-and-excel.md](../research/03-reporting-pdf-print-and-excel.md).

### Key Takeaways

1. **PDF Generation & Printing (Korean CJK)**:
   - **WebView2 / HTML-to-Print (in Tauri v2)**: Best zero-bloat default for standard printable views and reports. CSS `@media print` + `@page` + `<table>` (`<thead>`/`<tfoot>` repeat across pages) with `window.print()` (or WebView2 COM `ICoreWebView2_16::ShowPrintUI` / `ICoreWebView2_7::PrintToPdf`) uses Windows 11's native `Malgun Gothic` (`맑은 고딕`), DirectWrite/HarfBuzz CJK shaping, and built-in print preview / Save-as-PDF with **0 MB** added binary size.
   - **Typst (`typst-as-lib` + `typst-pdf`)**: Best engine if deterministic, offline, pixel-exact multi-page PDF generation is required (or if using a non-WebView GUI). Compiles multi-page Korean tables (`table.header(repeat: true)`, `table.footer(repeat: true)`, `counter(page)`) in **10–50 ms** with `rustybuzz` OpenType shaping, automatic CJK font subsetting, and valid `/ToUnicode` CMaps. Adds ~15–25 MB to the binary and can load Windows 11's system `C:\Windows\Fonts\malgun.ttf` at runtime.
   - **Pure-Rust PDF Crates (`pdf-writer`, `krilla`, `printpdf`, `genpdf`)**: Do **not** use directly for multi-page Korean tabular reports—`pdf-writer`, `krilla`, and `printpdf` lack automatic multi-page table layout, while `genpdf` is unmaintained (since 2021) and lacks modern CJK shaping/subsetting.
   - **Direct Windows 11 Printing**: If generating standalone `.pdf` files, open them via `opener::open(&pdf_path)` (`ShellExecuteW` `"open"` verb, which opens in Microsoft Edge's PDF viewer on Windows 11) rather than the broken `"print"` shell verb, or optionally bundle portable `SumatraPDF.exe -print-dialog "report.pdf"` for direct native print dialog invocation.
2. **Excel (`.xlsx`) Export & Import**:
   - **Export (`rust_xlsxwriter`)**: 100% pure Rust (cross-compiles cleanly from macOS to Windows 11), exports 100k+ cells in **< 200 ms**, supports `constant_memory` streaming (~1–2 MB flat RAM), number/currency formats (`₩#,##0`), text formats (`@` to preserve leading zeros like `010-...`), freeze panes, and print page setup. For Korean column widths, pair `set_column_width()` with `unicode-width` (`UnicodeWidthStr::width`, counting Hangul syllables as 2 width units) so full-width Korean text is never clipped (`###`).
   - **Import (`calamine`)**: Pure-Rust, read-only spreadsheet reader (`.xlsx`/`.xls`/`.ods`) with fast lazy sheet-level parsing and `serde` row deserialization.
