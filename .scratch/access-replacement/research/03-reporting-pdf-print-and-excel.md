# Research 03: PDF/Print Report Generation & Excel (`.xlsx`) Export Stack

**Ticket**: [03-reporting-pdf-print-and-excel.md](../issues/03-reporting-pdf-print-and-excel.md)  
**Target Environment**: Single non-developer user on a low-spec Windows 11 PC; cross-built or CI-built from macOS; full Korean (Hangul/CJK) typography required.

---

## Executive Summary & Recommendations

1. **PDF Generation & Printable Fixed-Layout Reports**:
   - **If the GUI is Tauri v2 (WebView2)**:
     - **Simplest Path (Zero extra binary bloat)**: Render report templates as HTML/CSS using `@media print` and `@page`, and invoke `window.print()` (for interactive print preview + "Save as PDF" via Windows 11 WebView2's built-in Chromium print dialog) or WebView2 COM `ICoreWebView2_16::ShowPrintUI` / `ICoreWebView2_7::PrintToPdfAsync` (via `webview2-com`). Because WebView2 uses the Windows 11 system font stack (`Malgun Gothic` / `맑은 고딕`) and HarfBuzz/DirectWrite, Korean CJK glyph shaping, line breaking, and font subsetting work out of the box with **0 MB** additional binary size.
     - **Deterministic Offline PDF Path (Exact multi-page invoices/ledgers independent of browser print margins)**: Embed **Typst** (`typst` + `typst-pdf` via `typst-as-lib` or `typst-bake`) or bundle the `typst.exe` CLI binary. Typst compiles multi-page Korean tables with repeating `table.header(repeat: true)` and `table.footer(repeat: true)`, page counters (`counter(page)`), and CJK line-breaking rules in **10–50 ms** in-memory.
   - **If the GUI is Native Rust (`egui` / `iced` / `Slint`)**:
     - Use **Typst** (`typst-as-lib` with `typst-pdf`) as the primary PDF report engine. Do **not** build multi-page Korean business tables directly in low-level pure-Rust PDF crates (`pdf-writer`, `krilla`, `printpdf`, `genpdf`), which either lack automatic table/page-break layout (`krilla`, `pdf-writer`, `printpdf`) or lack modern OpenType CJK shaping and maintained table ergonomics (`genpdf`).
2. **Windows 11 Printing Workflow**:
   - **Tauri v2 / WebView2**: `window.print()` or `ICoreWebView2_16::ShowPrintUI(COREWEBVIEW2_PRINT_DIALOG_KIND_BROWSER)` provides an instant, built-in print preview modal with printer selection, page orientation, and Save-to-PDF.
   - **Native PDF Handoff**: Open the generated `.pdf` file in the Windows 11 default PDF handler (Microsoft Edge by default on Windows 11) via `opener::open(&pdf_path)` (`ShellExecuteW` `"open"` verb) so the user sees the exact PDF and clicks Print (`Ctrl+P`), or optionally bundle **SumatraPDF** (`SumatraPDF.exe -print-dialog "report.pdf"` or `-print-to-default`) if one-click direct print dialog without browser chrome is desired.
3. **Excel (`.xlsx`) Export & Import**:
   - **Export**: Use **`rust_xlsxwriter`** (pure Rust, zero C dependencies, cross-compiles cleanly from macOS to Windows). It generates formatted `.xlsx` files at native speed, supports `constant_memory` row streaming (capped to single-row RAM usage), number/currency/date formatting, freeze panes, and `autofit()`. Caveat: when calculating column widths for Korean (`UTF-8`) text, account for full-width East Asian character metrics (~1.8x–2.0x ASCII width) or use `set_column_width()` explicitly.
   - **Import (One-Time Migration or Excel Upload)**: Use **`calamine`** (pure Rust, read-only `.xlsx`/`.xls`/`.ods` parser with lazy sheet-level parsing).

---

## 1. PDF Generation & Printing (with Korean CJK Font Support)

### 1.1 Typst (Embedded Library via `typst` / `typst-as-lib` or Bundled CLI)

Typst is a modern typesetting system written in Rust, backed by `pdf-writer` and `krilla`/HarfBuzz-grade shaping (`rustybuzz`, `ttf-parser`, `subsetter`).

| Criterion | Evaluation | Primary Source |
| :--- | :--- | :--- |
| **Integration Options** | **1. Embedded Rust crate**: Use `typst` + `typst-pdf` directly by implementing the `typst::World` trait, or use wrapper crates [`typst-as-lib`](https://github.com/Relacibo/typst-as-lib) / [`typst-bake`](https://crates.io/crates/typst-bake) to pass Rust structs (`serde` / `sys.inputs` / `Dict`) into `.typ` templates in memory.<br>**2. Bundled CLI sidecar**: Ship official `typst-x86_64-pc-windows-msvc` binary alongside the app and pass JSON data via `typst compile --input data=report.json template.typ report.pdf`. | [typst/typst GitHub](https://github.com/typst/typst), [Relacibo/typst-as-lib](https://github.com/Relacibo/typst-as-lib), [docs.rs/typst](https://docs.rs/typst/latest/typst/) |
| **Speed & Memory Footprint** | Routine 1-to-20-page business reports (invoices, ledgers, summary tables) compile to PDF in **10–50 ms** in-memory once fonts are indexed. Incremental caching (`comemo`) keeps repeated generations in the single-digit milliseconds. Memory usage during compilation of standard business reports is typically **20–50 MB** (mostly font index + AST). | [typst/typst GitHub](https://github.com/typst/typst), [docs.rs/comemo](https://docs.rs/comemo/latest/comemo/) |
| **Binary Size Impact** | Embedding `typst` + `typst-pdf` adds **~15–25 MB** to the release `.exe` (with `lto = true`, `strip = true`). If embedding a Korean `.ttf`/`.otf` font via `include_bytes!`, a single weight of `Pretendard` or `NanumGothic` adds ~2–4 MB, or Typst can load Windows 11's pre-installed `C:\Windows\Fonts\malgun.ttf` (`Malgun Gothic` regular + bold) at runtime via `typst-kit` (`FontSearcher`) for **0 MB** font bundle overhead. | [docs.rs/typst-kit](https://docs.rs/typst-kit/latest/typst_kit/fonts/index.html) |
| **Table, Header, Footer & Page-Break Layout** | First-class support specifically designed to replace LaTeX `longtable` / Access reports:<br>- **Tables**: `#table(columns: (1fr, 2fr, auto), align: ..., stroke: ...)` with cell spanning (`table.cell(colspan: 2, rowspan: 2)`).<br>- **Repeating Headers/Footers**: `table.header(repeat: true)[...]` and `table.footer(repeat: true)[...]` automatically repeat at the top/bottom of every page split.<br>- **Page Headers/Footers & Counters**: `#set page(paper: "a4", header: [...], footer: context [ #counter(page).display("1 / 1", both: true) ])`.<br>- **Page Breaks**: `#pagebreak(weak: true)` and `block(breakable: false)` to keep signature blocks or group totals together on one page. | [Typst Table Docs](https://typst.app/docs/reference/model/table/), [Typst Page Docs](https://typst.app/docs/reference/layout/page/), [Typst Pagebreak Docs](https://typst.app/docs/reference/layout/pagebreak/) |
| **Korean (CJK) Font & Typography Support** | - Uses `rustybuzz` (pure-Rust HarfBuzz port) for full OpenType glyph shaping and `subsetter` to embed only used Hangul/Hanja glyphs into the PDF (keeping output PDFs around **20–100 KB** instead of 10 MB).<br>- Generates valid `/ToUnicode` CMaps so Korean text in the output PDF is 100% searchable and copy-pasteable.<br>- Supports `#set text(font: "Malgun Gothic", lang: "ko", region: "kr")` for proper Korean line-breaking rules (Hangul word/syllable wrapping and punctuation hanging). | [Typst Text Docs](https://typst.app/docs/reference/text/text/), [docs.rs/typst-pdf](https://docs.rs/typst-pdf/latest/typst_pdf/) |

---

### 1.2 Pure Rust PDF Libraries (`pdf-writer`, `krilla`, `printpdf`, `genpdf`)

We evaluated the four primary pure-Rust PDF crates on `crates.io` against the requirements of fixed-layout Korean business reports (multi-page tables, headers/footers, and CJK glyph shaping):

| Crate | Abstraction Level | Table, Header/Footer & Page-Break Ergonomics | Korean (CJK) Glyph Shaping & Font Subsetting | Verdict for Business Reports |
| :--- | :--- | :--- | :--- | :--- |
| **`pdf-writer`** ([docs.rs](https://docs.rs/pdf-writer/latest/pdf_writer/), [GitHub](https://github.com/typst/pdf-writer)) | Low-level PDF syntax writer (maintained by the Typst team) | **None.** Writes raw PDF objects, streams, and dictionaries. Zero text wrapping, zero coordinate calculation, zero table layout. | **Manual.** Caller must parse fonts (`ttf-parser`), shape glyphs (`rustybuzz`), subset fonts (`subsetter`), and manually emit `CIDFontType2` dictionaries and `/ToUnicode` CMap streams. | Too low-level to use directly; use Typst (which uses `pdf-writer` under the hood). |
| **`krilla`** ([docs.rs](https://docs.rs/krilla/latest/krilla/), [GitHub](https://github.com/LaurenzV/krilla)) | Mid-level 2D vector/PDF canvas (built on `pdf-writer` by Typst contributor LaurenzV) | **None.** Provides a 2D drawing surface (`Surface::draw_text`, paths, fills, strokes, PDF/A compliance) at explicit `(x, y)` coordinates. Does **not** provide multi-page flow layout, automatic line wrapping, or table layout. | **Excellent.** Built-in OpenType shaping, TrueType/CFF font subsetting, and automatic `/ToUnicode` CMap generation for CJK characters. | Great for custom vector drawing or single-page canvas badges, but requires writing a custom layout engine for multi-page tables. |
| **`printpdf`** ([docs.rs](https://docs.rs/printpdf/latest/printpdf/), [crates.io](https://crates.io/crates/printpdf)) | Low/Mid-level coordinate PDF builder | Primarily coordinate-based (`Mm`/`Pt` placement). Recent versions (`0.8+`) introduced experimental HTML/XML layout via `azul-layout`, but it lacks mature repeating table headers, group footers, and predictable multi-page breaking. | Supports TrueType font embedding and subsetting, but has historical issues with CJK line-breaking rules and complex OpenType shaping without manual positioning. | Unsuitable for multi-page tabular reports. |
| **`genpdf`** / **`genpdfi`** ([docs.rs](https://docs.rs/genpdf/latest/genpdf/), [crates.io](https://crates.io/crates/genpdf)) | High-level document tree (`TableLayout`, `Paragraph`, `LinearLayout`) | Has built-in `TableLayout`, margins, and page headers. However, `genpdf` (last official release `0.2.0` in 2021) is largely unmaintained, and its table layout lacks repeating header/footer rows across page breaks, cell rowspans/colspans, and per-cell background fills. | Relies on older `rusttype` + `printpdf` stacks. It does **not** perform full OpenType shaping (`HarfBuzz`/`rustybuzz`) and often embeds entire un-subsetted CJK font files (resulting in **5–15 MB** per PDF) or fails to generate proper CJK metrics. | **Avoid.** Unmaintained upstream and poor CJK subsetting/table ergonomics. |

---

### 1.3 WebView2 / HTML-to-Print (If Using Tauri v2)

If the application uses **Tauri v2**, the Windows 11 frontend runs inside **Microsoft Edge WebView2** (Evergreen Chromium runtime pre-installed on Windows 11).

- **CSS `@media print` & `@page` Layout**:
  - Standard HTML `<table>` elements with `<thead>` and `<tfoot>` automatically repeat `<thead>` and `<tfoot>` across page breaks in Chromium/WebView2 when `break-inside: avoid` is applied to `<tr>`.
  - CSS `@page { size: A4 portrait; margin: 15mm; }` controls paper size and margins.
  - Korean fonts (`font-family: "Malgun Gothic", "Pretendard", sans-serif; word-break: keep-all;`) render with full DirectWrite/HarfBuzz shaping and zero bundled font weight.
- **Triggering Native Print Preview (`window.print()` / `ShowPrintUI`)**:
  - Calling `window.print()` from JavaScript (or `webviewWindow.print()` in Tauri v2) opens WebView2's built-in Chromium Print Preview UI (`COREWEBVIEW2_PRINT_DIALOG_KIND_BROWSER`), allowing the user to preview pages, select any installed Windows printer, or choose **"Save as PDF"** with one click.
  - Calling `ICoreWebView2_16::ShowPrintUI(COREWEBVIEW2_PRINT_DIALOG_KIND_SYSTEM)` opens the native Windows 11 OS print dialog directly ([Microsoft Learn: `ICoreWebView2_16::ShowPrintUI`](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2_16#showprintui)).
- **Silent Programmatic PDF Generation (`PrintToPdfAsync`)**:
  - WebView2 exposes `ICoreWebView2_7::PrintToPdf` (projected as `PrintToPdfAsync` in .NET and accessible in Rust via `webview2-com` inside `WebviewWindow::with_webview`) together with `ICoreWebView2PrintSettings` (page size, margins, orientation, scale factor, header/footer toggle) ([Microsoft Learn: `ICoreWebView2_7::PrintToPdf`](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2_7#printtopdf)).
  - **Constraint**: Only one print/PDF operation can run at a time per `CoreWebView2` instance (`E_ABORT` if concurrent). To export a PDF in the background without flashing print styles on the main form window, either render the report inside a hidden `<iframe style="display:none">` and call `iframe.contentWindow.print()`, or use **Typst** in the Rust backend for silent file export.

---

### 1.4 Direct Windows 11 Printing Workflows

How should a native Windows 11 app hand off a generated report to a physical printer or print preview?

| Workflow | Mechanism | UX for Non-Technical User | Pros & Cons |
| :--- | :--- | :--- | :--- |
| **1. WebView2 Built-in Print Modal** *(Tauri v2 only)* | `window.print()` or `ICoreWebView2_16::ShowPrintUI` | Instant in-app print preview modal (identical to Edge/Chrome print preview) with Printer dropdown, Copies, Orientation, and "Save as PDF". | **Best UX if using Tauri v2.** Zero external processes; preview and print happen inside the app window. |
| **2. Open PDF in Default Windows 11 Viewer (Edge)** | Generate `.pdf` via Typst, then call `opener::open(&pdf_path)` (invokes Win32 `ShellExecuteW` with `"open"` verb). | Opens the PDF immediately in Microsoft Edge (built into Windows 11) or Adobe/SumatraPDF; user clicks the Print icon (`Ctrl+P`). | **Zero extra dependencies.** Note: Do **not** use `ShellExecuteW` with the `"print"` verb on Windows 11—when Edge is the default `.pdf` handler, the legacy `"print"` shell verb is unreliable or ignored. Always use `"open"` so the user sees the preview first, or use Option 3. |
| **3. Bundled SumatraPDF Sidecar** *(Optional for 1-click native print)* | Bundle portable `SumatraPDF.exe` (~7–8 MB single executable, GPLv3 / BSD utilities) and run:<br>`SumatraPDF.exe -print-dialog -exit-when-done "report.pdf"` (or `-print-to-default`). | Immediately pops up the native Windows Print Dialog (or prints silently to the default printer) without opening a full browser window. | Ideal if the customer prints dozens of receipts/invoices per day and wants direct-to-printer (`-print-to-default`) or immediate native print dialog (`-print-dialog`). ([SumatraPDF Command-Line Docs](https://www.sumatrapdfreader.org/docs/Command-line-arguments)) |

---

## 2. Excel (`.xlsx`) Export & Import

### 2.1 Excel Export: `rust_xlsxwriter`

[`rust_xlsxwriter`](https://docs.rs/rust_xlsxwriter/latest/rust_xlsxwriter/) ([GitHub: `jmcnamara/rust_xlsxwriter`](https://github.com/jmcnamara/rust_xlsxwriter)) is a pure-Rust port of the author's widely used C/Python `libxlsxwriter` / `XlsxWriter` libraries.

| Dimension | Findings | Primary Source |
| :--- | :--- | :--- |
| **Cross-Compilation & Dependencies** | **100% pure Rust.** Zero C/C++ dependencies (unlike `xlsxwriter` crate which binds `libxlsxwriter` in C). Cross-compiles effortlessly from macOS to `x86_64-pc-windows-msvc` or `x86_64-pc-windows-gnu`. | [docs.rs/rust_xlsxwriter](https://docs.rs/rust_xlsxwriter/latest/rust_xlsxwriter/), [crates.io/crates/rust_xlsxwriter](https://crates.io/crates/rust_xlsxwriter) |
| **Speed & Performance** | Writes 100,000+ formatted cells in **< 200 ms**. Optional `zmij` feature flag speeds up floating-point formatting by an additional ~10%, and `ryu` or `zlib` (native C zlib if desired, though default pure-Rust `flate2`/`miniz_oxide` is already very fast) can be toggled. | [rust_xlsxwriter Performance Docs](https://docs.rs/rust_xlsxwriter/latest/rust_xlsxwriter/performance/index.html) |
| **Memory Footprint (`constant_memory`)** | - **Standard mode**: Holds the active worksheet XML model in memory (~5–20 MB for typical 1,000–20,000 row business exports).<br>- **Constant memory mode** (`features = ["constant_memory"]`, `workbook.add_worksheet_with_constant_memory()`): Streams each row to a temporary disk file as soon as the next row starts, keeping RAM usage flat (~1–2 MB) even for **1,000,000 rows**. Requires writing rows sequentially (`row 0, 1, 2...`) and uses inline strings instead of the shared string table. | [docs.rs: `add_worksheet_with_constant_memory`](https://docs.rs/rust_xlsxwriter/latest/rust_xlsxwriter/struct.Workbook.html#method.add_worksheet_with_constant_memory) |
| **Cell Formatting & Access Report Equivalents** | Full support for business report styling:<br>- Fonts (`Malgun Gothic` / `맑은 고딕`), sizes, bold/italic, colors.<br>- Number formats (`#,##0`, `₩#,##0`, `yyyy-mm-dd`, `@` for text-formatted IDs/phone numbers so leading zeros `010-...` are preserved).<br>- Borders, header background fills, merged title cells (`merge_range`), freeze panes (`set_freeze_panes(1, 0)`), auto-filters (`autofilter`), and Excel Tables (`Table::new()`).<br>- Print setup (`set_landscape()`, `set_paper_size(9)` for A4, `set_fit_to_pages(1, 0)`). | [docs.rs/rust_xlsxwriter/Format](https://docs.rs/rust_xlsxwriter/latest/rust_xlsxwriter/struct.Format.html), [Worksheet Docs](https://docs.rs/rust_xlsxwriter/latest/rust_xlsxwriter/struct.Worksheet.html) |
| **Column Auto-Fit & Korean (UTF-8) Compatibility** | - **Encoding**: Writes standard OpenXML UTF-8 with XML entity escaping; opens cleanly in Korean Microsoft Excel on Windows 11, Hancom Cell (한셀), and LibreOffice Calc without BOM/CP949 mojibake issues (unlike `.csv` which requires UTF-8 BOM `\u{FEFF}` for Excel on Korean Windows).<br>- **Column Width / Autofit Nuance for Korean**: `worksheet.autofit()` (and optional `features = ["enhanced_autofit"]` for formatted numbers/dates) estimates pixel width using Calibri 11pt metrics. However, Hangul syllables (`U+AC00..=U+D7A3`) are **full-width (East Asian Wide)** glyphs (~2x the width of ASCII characters), and `autofit()` does not work on rows already flushed in `constant_memory` mode.<br>- **Best Practice for Korean Columns**: Either set explicit column widths (`worksheet.set_column_width(col, width)`) or compute maximum display width using Unicode East Asian Width (`unicode-width` crate `UnicodeWidthStr::width(s)`, where each Hangul syllable counts as `2` units, plus `4` units of cell padding) and pass that to `set_column_width()`. | [docs.rs: `Worksheet::autofit`](https://docs.rs/rust_xlsxwriter/latest/rust_xlsxwriter/struct.Worksheet.html#method.autofit), [docs.rs/unicode-width](https://docs.rs/unicode-width/latest/unicode_width/) |

---

### 2.2 Excel Import (If Needed): `calamine`

If the application needs to import `.xlsx` / `.xls` spreadsheets (either during the one-time legacy data migration or as an ongoing user import feature):

- **Crate**: [`calamine`](https://docs.rs/calamine/latest/calamine/) ([GitHub: `tafia/calamine`](https://github.com/tafia/calamine)).
- **Capabilities**: Pure-Rust, read-only reader supporting `.xlsx`, `.xlsm`, `.xlsb`, `.xls` (legacy BIFF8), and `.ods`.
- **Performance & Memory**: Uses lazy **sheet-level** parsing—opening a workbook only reads metadata and `sharedStrings.xml`, and `workbook.worksheet_range("Sheet1")` decodes only the requested worksheet into a contiguous `Range<Data>` buffer. Typical 10,000-row sheets parse in **< 30 ms** with minimal RAM (< 15 MB). Includes `serde` deserialization (`RangeDeserializerBuilder`) for mapping Korean header columns directly into typed Rust structs.

---

## 3. Recommended Architecture for This Project

| Use Case | Recommended Stack | Why |
| :--- | :--- | :--- |
| **Quick Table / Form Print & Preview** *(in Tauri v2)* | **HTML + CSS `@media print` + `window.print()`** | Zero binary overhead, uses Windows 11 `Malgun Gothic` natively, repeating `<thead>`/`<tfoot>` works out of the box, and provides an interactive print preview + "Save as PDF" dialog inside the app. |
| **Pixel-Exact Multi-Page PDF Reports / Invoices** *(or non-WebView GUI)* | **Typst (`typst-as-lib` + `typst-pdf`)** loading `C:\Windows\Fonts\malgun.ttf` (with embedded fallback font) | Deterministic pagination, repeating headers/footers, running subtotals, 10–50 ms compilation speed, automatic CJK font subsetting, and valid `/ToUnicode` CMaps. |
| **Excel (`.xlsx`) Export** | **`rust_xlsxwriter`** + **`unicode-width`** (for Korean column width calculation) | Pure Rust, < 200 ms export speed, native Korean Excel compatibility, preserves leading zeros (`010-...`) and currency (`₩#,##0`), and cross-compiles cleanly from macOS to Windows 11. |
| **Excel (`.xlsx`/`.xls`) Import** | **`calamine`** | Pure Rust, fast sheet-level lazy reader with typed `serde` row deserialization. |

---

## Primary Sources Cited

1. **Typst Core & PDF Export**: [https://github.com/typst/typst](https://github.com/typst/typst) | [https://docs.rs/typst/latest/typst/](https://docs.rs/typst/latest/typst/) | [https://docs.rs/typst-pdf/latest/typst_pdf/](https://docs.rs/typst-pdf/latest/typst_pdf/) | [https://docs.rs/typst-kit/latest/typst_kit/fonts/index.html](https://docs.rs/typst-kit/latest/typst_kit/fonts/index.html)
2. **Typst Layout & CJK Reference**: [Tables (`table.header` / `repeat`)](https://typst.app/docs/reference/model/table/) | [Page Setup (`page`)](https://typst.app/docs/reference/layout/page/) | [Page Breaks (`pagebreak`)](https://typst.app/docs/reference/layout/pagebreak/) | [Text & CJK Fonts (`text`)](https://typst.app/docs/reference/text/text/)
3. **`typst-as-lib` Wrapper**: [https://github.com/Relacibo/typst-as-lib](https://github.com/Relacibo/typst-as-lib) | [https://docs.rs/typst-as-lib/latest/typst_as_lib/](https://docs.rs/typst-as-lib/latest/typst_as_lib/)
4. **Pure-Rust PDF Crates**: [`pdf-writer` (docs.rs)](https://docs.rs/pdf-writer/latest/pdf_writer/) | [`krilla` (GitHub)](https://github.com/LaurenzV/krilla) | [`printpdf` (docs.rs)](https://docs.rs/printpdf/latest/printpdf/) | [`genpdf` (docs.rs)](https://docs.rs/genpdf/latest/genpdf/)
5. **Microsoft Edge WebView2 Printing APIs**: [`ICoreWebView2_16::ShowPrintUI` & `Print`](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2_16) | [`ICoreWebView2_7::PrintToPdf`](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2_7#printtopdf)
6. **SumatraPDF CLI Printing**: [https://www.sumatrapdfreader.org/docs/Command-line-arguments](https://www.sumatrapdfreader.org/docs/Command-line-arguments)
7. **`rust_xlsxwriter`**: [https://github.com/jmcnamara/rust_xlsxwriter](https://github.com/jmcnamara/rust_xlsxwriter) | [https://docs.rs/rust_xlsxwriter/latest/rust_xlsxwriter/](https://docs.rs/rust_xlsxwriter/latest/rust_xlsxwriter/) | [Performance & `constant_memory`](https://docs.rs/rust_xlsxwriter/latest/rust_xlsxwriter/performance/index.html) | [`Worksheet::autofit`](https://docs.rs/rust_xlsxwriter/latest/rust_xlsxwriter/struct.Worksheet.html#method.autofit)
8. **`calamine`**: [https://github.com/tafia/calamine](https://github.com/tafia/calamine) | [https://docs.rs/calamine/latest/calamine/](https://docs.rs/calamine/latest/calamine/)
