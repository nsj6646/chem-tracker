# Evaluate Lightweight GUI Frameworks for Windows 11 (developed on macOS)

Type: `research`
Label: `wayfinder:research`
Status: resolved

## Question

Which GUI framework and language combination best serves a data-heavy custom business application (forms, filterable/sortable data tables/grids, Korean IME input) targeting a **low-spec Windows 11 PC**, while being **developed on macOS**?

Investigate and compare against primary sources (official docs, GitHub repositories, release notes, issue trackers):

1. **Rust + GPUI (Zed's UI framework)**:
   - Current Windows 11 port maturity, stability, and hardware requirements (DirectX/GPU requirements on low-spec integrated graphics).
   - Korean IME (Input Method Editor) support on Windows.
   - Availability of form controls (text inputs, dropdowns, date pickers) and data table/grid components (`gpui-component` or ecosystem libraries).
   - Cross-compilation or CI build story from macOS to Windows 11.
2. **Other Rust Native GUI Candidates (`iced`, `egui`, `Slint`)**:
   - Memory footprint, CPU/GPU usage on low-spec Windows 11 (including software rendering fallback if GPU is weak).
   - Korean IME support on Windows 11.
   - Maturity of table/grid and form widgets.
   - Cross-compiling from macOS to Windows.
3. **Lightweight Hybrid Candidate (`Tauri v2` + Rust backend + Windows 11 built-in WebView2)**:
   - Actual RAM footprint and startup speed on low-spec Windows 11 compared to MS Access and native Rust GUIs.
   - Trade-offs: instant access to mature HTML/CSS table grids, Korean IME, and print/report layouts vs WebView2 memory overhead.
4. **Other Notable Lightweight Candidates (e.g., Go + Wails / Fyne, Flutter Desktop)**:
   - Brief comparison on binary size, RAM usage, Korean IME, and macOS-to-Windows cross-compilation.

Save the detailed findings with primary source citations to `.scratch/access-replacement/research/01-gui-frameworks.md`.

## Answer

Detailed findings and primary-source citations are recorded in [01-gui-frameworks.md](../research/01-gui-frameworks.md).

### Summary of Findings

1. **`Tauri v2` (Rust + Built-in Windows 11 WebView2) — Top Recommendation for MS Access Replacement**:
   - **Footprint**: Produces a **3–8 MB** installer (WebView2 is pre-installed on Windows 11) with a **~0.4–0.9s** cold start and **~120–250 MB RAM** for a single-window app—roughly comparable to or lighter than 64-bit MS Access (150–300+ MB), though ~100–180 MB heavier than pure native Rust GUIs. Supports `CoreWebView2MemoryUsageTargetLevel::Low` trimming.
   - **Business UI & Korean IME**: Because WebView2 uses Microsoft's native Text Services Framework (`TSF`), **Korean IME (Hangul composition, `한/영` toggle, Hanja conversion, inline grid editing)** works without the composition/focus bugs that plague custom-rendered native toolkits. Instant access to **TanStack Table / AG Grid**, rich HTML form validation, and native `@media print` / PDF reporting.
   - **macOS -> Windows Cross-Build**: Officially supported directly on macOS via `cargo-xwin` + Homebrew `llvm` and `nsis` (`cargo tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc`).

2. **Rust + GPUI + `gpui-component` (Longbridge GPUI Kit)**:
   - **Footprint & Rendering**: Zed's Windows port migrated from Vulkan to **DirectX 11 + DirectWrite** specifically to support low-end Windows integrated GPUs and VMs (~40–90 MB RAM).
   - **Widgets vs Risks**: Longbridge's `gpui-component` (`gpui-kit`) provides the richest native Rust widget set (`DataTable` with 100k+ virtualized rows, `Input`, `DatePicker`, docking, modals). However, custom Win32 IME handling still sees occasional CJK/Korean composition and keybinding edge cases, and **cross-compiling from macOS is blocked locally** because `gpui/build.rs` invokes Windows-only `fxc.exe` to compile HLSL shaders (requiring Wine or GitHub Actions `windows-latest` runners).

3. **Other Native Rust GUIs (`iced`, `egui`, `Slint`)**:
   - **`iced`**: **20–45 MB RAM**, cross-compiles cleanly from macOS via `cargo-xwin`, and includes a built-in CPU software fallback (`iced_tiny_skia`) alongside `wgpu` (DX12). Korean IME works via `winit` + `cosmic-text` (with minor candidate box positioning quirks), but `iced::widget::table` and form widgets require significant custom code for Access-style inline editing and filtering.
   - **`egui` (`eframe`)**: **25–50 MB RAM**, **5–10 MB `.exe`**, easy `cargo-xwin` cross-build, and fast virtualized tables via `egui_extras::TableBuilder`, but immediate-mode focus transitions cause Korean IME preedit/commit edge cases on rapid cell entry, and it lacks native office aesthetics or printing.
   - **`Slint`**: Lowest resource usage (**15–35 MB RAM**) with a fast CPU `SoftwareRenderer`, but `StandardTableView` is limited to string cells, and proprietary desktop apps require displaying a Slint attribution badge under the Royalty-Free license (or paying for a commercial license).

4. **Other Candidates (`Wails`, `Fyne`, `Flutter Desktop`)**:
   - **Go + Wails**: Similar WebView2 pros/cons to Tauri v2 (~130–260 MB RAM), cross-compiles via `mingw-w64`, but CGO + SQLite cross-compilation is less ergonomic than Rust's `cargo-xwin` + `rusqlite` (`bundled`).
   - **Go + Fyne / Flutter Desktop**: Both suffer from CJK/Korean IME composition quirks on Windows, and Flutter Desktop cannot cross-compile to Windows from macOS.
