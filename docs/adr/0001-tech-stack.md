# Tauri v2, Bundled SQLite, and Pure-Rust Excel Stack for Windows 11 Access Replacement

Status: accepted

We will build the Korea branch Chemical Import Volume Tracker as a single-window **Tauri v2** desktop application (**Rust** backend + Windows 11 built-in **WebView2** with `CoreWebView2MemoryUsageTargetLevel::Low` + **React 19 / TypeScript / Vite / `@tanstack/react-table` & `@tanstack/react-virtual` / Tailwind CSS** frontend), backed by synchronous **`rusqlite`** (`bundled`, `backup`, `functions` in WAL mode) with **`rust_decimal`** fixed-point arithmetic, **WebView2 native `@media print` / `PrintToPdf`** for Korean A4 reports, and **`calamine` + `rust_xlsxwriter` (`unicode-width`)** for Excel I/O, cross-compiled from macOS to Windows 11 (`x86_64-pc-windows-msvc`) via **`cargo-xwin` + `nsis`**.

## Considered Options

- **GUI Framework**:
  - **Tauri v2 + Windows 11 WebView2 (Chosen)**: Uses the OS-bundled Edge WebView2 runtime (3–8 MB installer, ~0.4–0.9s cold start, ~120–250 MB RAM—comparable to or lighter than 64-bit MS Access). Eliminates custom Win32 Korean IME (`TSF`) composition bugs during inline table cell editing, provides mature virtualized grids for shipment and substance tables, and cross-compiles directly on macOS via `cargo-xwin` + `nsis`.
  - **GPUI + `gpui-component` (Rejected)**: Lower RAM (~40–90 MB) and fast DirectX 11 data tables, but custom Win32 IME handling still exhibits Korean composition/focus edge cases and `gpui/build.rs` requires Windows-only `fxc.exe` shader compilation, blocking local macOS cross-builds.
  - **`iced` / `egui` (Rejected)**: Lowest RAM (~20–50 MB) and clean `cargo-xwin` builds, but lacks out-of-the-box Access-grade editable master-detail grids, print layout engines, and flicker-free Korean IME candidate positioning.
- **Embedded DBMS & Arithmetic**:
  - **SQLite via synchronous `rusqlite` + `rust_decimal` (Chosen)**: Adds <1 MB to the binary, runs in single-digit milliseconds for annual substance aggregations, avoids async runtime overhead, prevents IEEE-754 float drift on `Product Import Weight (kg) × Weight Percentage (%)` (and enforces exact `100%` recipe totals), and enables atomic single-file backups on a live WAL database via `VACUUM INTO`.
  - **DuckDB (Rejected)**: 35–60 MB binary overhead and heavy buffer pool footprint for a low-spec PC with <100k shipment rows.
- **PDF Reporting & Excel I/O**:
  - **WebView2 `@media print` + `window.print()` / `PrintToPdf` (Chosen over embedded `Typst`)**: Adds 0 MB to the binary (vs. +15–25 MB for `typst-as-lib`), reuses the exact React report preview component for both interactive printing and PDF export, and leverages Windows 11's native `Malgun Gothic` (`맑은 고딕`) DirectWrite shaping.
  - **`calamine` + `rust_xlsxwriter` with `unicode-width` (Chosen)**: 100% pure Rust with zero COM/Excel or C DLL dependencies; enforces `@` text formatting on `CAS번호` (`50-00-0`) to prevent Excel auto-date corruption, and calculates Hangul column widths at 2 units per syllable.

## Consequences

- All filtering and fixed-point `연간 화학물질 총 수입량` aggregations execute in Rust/SQLite over the customer's directly managed database (without bundling or loading the 47,531-row external NCIS master Excel file), keeping startup, query latency, and RAM footprint minimal on low-spec Windows 11 hardware.
- Because customer `제품 구성성분(BOM)` recipes and shipment history are confidential (`대외비`), the app embeds a pure-Rust self-service importer (`calamine` + `csv`) with downloadable `.xlsx` templates and preview/validation rather than bundling C `mdbtools` DLLs or a JVM runtime.
- Primary packaging runs locally on macOS via `cargo tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis` (producing a standalone `*-setup.exe` and portable `.exe`), backed by a GitHub Actions `windows-latest` workflow for native MSVC verification.
