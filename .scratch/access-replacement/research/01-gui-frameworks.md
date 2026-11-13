# Research 01: Lightweight GUI Frameworks for Low-Spec Windows 11 (Developed on macOS)

## Context & Evaluation Criteria

We are evaluating GUI frameworks and language combinations to replace an MS Access business application on a **low-spec Windows 11 PC** (weak integrated GPU, constrained RAM/CPU), while **developing on macOS**.

Key requirements:
1. **Low-spec Windows 11 footprint**: Minimal RAM overhead, fast cold start, and graceful handling of weak integrated graphics (DirectX 11/12 or CPU software fallback).
2. **Korean IME (Microsoft IME) reliability**: Seamless 2-set/3-set Hangul syllable composition (preedit), `한/영` (`Lang1`/`VK_HANGUL`) toggle, Hanja conversion, and focus/blur commit handling without losing or duplicating trailing syllables.
3. **Business UI widget maturity**: Filterable/sortable virtualized data tables/grids (thousands of rows), form controls (text inputs, dropdowns, date pickers, validation, keyboard/Tab navigation), and print/export workflows.
4. **macOS-to-Windows build story**: Ability to develop on macOS and cross-compile (`cargo-xwin`, NSIS) or build cleanly via CI (`windows-latest` GitHub Actions).

---

## 1. Rust + GPUI (Zed's UI Framework)

### 1.1 Windows 11 Maturity & Hardware Requirements
- **Windows Port & Rendering Pipeline**: Zed officially shipped on Windows (`zed-industries/zed`) after replacing its initial experimental Vulkan backend with a native **DirectX 11 + DirectWrite** backend ([Zed Blog: Zed on Windows](https://zed.dev/blog/zed-on-windows), [Zed GitHub Repository](https://github.com/zed-industries/zed)).
- **Why DirectX 11**: The Zed engineering team moved from Vulkan to DirectX 11 specifically because Vulkan drivers on lower-end Windows laptops, integrated GPUs, and VMs had widespread compatibility issues and high VRAM usage. DirectX 11 is supported across virtually all Windows 11 hardware (and even WARP software rasterization on Windows), with lower VRAM footprint and native DirectWrite text shaping ([Zed on Windows Technical Deep Dive](https://zed.dev/blog/zed-on-windows)).
- **Footprint**: Idle RAM is typically **40–90 MB** (mostly GPU texture/atlas caches and DirectWrite font caches). However, GPUI is architected like a 2D game engine; it does not have a pure CPU non-DirectX software buffer mode like `tiny-skia`, relying instead on Direct3D 11 (or Windows WARP fallback if no hardware GPU is present).

### 1.2 Korean IME Support on Windows 11
- **Custom Win32 IME Pipeline**: Because GPUI renders all text on a GPU canvas via DirectWrite rather than using native Win32 edit controls, it implements `WM_IME_STARTCOMPOSITION`, `WM_IME_COMPOSITION`, `WM_IME_CHAR`, and `ITfThreadMgr` / `ImmGetCompositionStringW` manually in `crates/gpui/src/platform/windows/` ([Zed Issues: `platform:windows` + `area:internationalization`](https://github.com/zed-industries/zed/issues?q=is%3Aissue+label%3Aplatform%3Awindows+IME)).
- **Historical & Ongoing Quirks**:
  - `한/영` toggle (`VK_HANGUL`) and `TranslateMessage` routing bugs have required multiple patches in Zed's Windows event loop.
  - Jamo separation (`ㅇㅏㄴㄴㅕㅇ`) and shortcut interception during active composition (e.g., `Enter`, `Backspace`, or `Esc` firing before the IME commits the preedit string) have been recurring edge cases in both Zed and downstream `gpui-component` inputs ([gpui-component Issues](https://github.com/longbridge/gpui-component/issues)).

### 1.3 Widget Ecosystem (`gpui-component` / Longbridge GPUI Kit)
- **GPUI Core**: `gpui` itself is a low-level hybrid immediate/retained UI framework with Flexbox/Tailwind-style layout (`taffy`) and no built-in high-level business widgets ([GPUI crate README](https://github.com/zed-industries/zed/tree/main/crates/gpui)).
- **Longbridge `gpui-component` (GPUI Kit)**: Built by Longbridge for their financial trading desktop terminal ([longbridge/gpui-component](https://github.com/longbridge/gpui-component), [gpui-kit.com](https://gpui-kit.com)). It provides **60–75+ high-density desktop components**:
  - **Tables/Grids**: Includes both a stateless `Table` and a virtualized `DataTable` capable of rendering hundreds of thousands of rows with fixed/resizable columns, column sorting, cell selection, and `Tab`/`Shift+Tab` focus navigation.
  - **Form Controls**: `Input` (with validation masks, prefix/suffix groups, `NumberInput`), `DatePicker` (single date and date-range calendar grids), `Select`/Dropdown, `Checkbox`, `Radio`, `Switch`, `Form` layout helpers, and docking/modal/toast systems.
- **Ecosystem Risk**: `gpui` is still tightly coupled to Zed's rapid internal release cycle; `gpui-component` pins specific `gpui` revisions/versions and frequently introduces breaking API changes across minor releases.

### 1.4 Cross-Compilation from macOS to Windows 11
- **The `fxc.exe` Blocker on macOS**: While pure Rust + Win32 crates cross-compile cleanly from macOS via `cargo-xwin`, **`gpui`'s `crates/gpui/build.rs` invokes Microsoft's `fxc.exe` (Effect-Compiler Tool from the Windows SDK)** at build time to compile `shaders.hlsl` into DirectX bytecode ([zed-industries/zed `crates/gpui/build.rs`](https://github.com/zed-industries/zed/blob/main/crates/gpui/build.rs)).
- Because `fxc.exe` is a Windows binary (`C:\Program Files (x86)\Windows Kits\10\bin\<sdk>\x64\fxc.exe`), running `cargo xwin build --target x86_64-pc-windows-msvc` directly on macOS panics (`Failed to find fxc.exe`) unless you wrap `fxc.exe` via Wine (`GPUI_FXC_PATH`) or build Windows binaries on a Windows VM / GitHub Actions `windows-latest` runner.

---

## 2. Other Rust Native GUI Candidates (`iced`, `egui`, `Slint`)

### 2.1 `iced` (v0.13 / v0.14+)
- **Architecture & Low-Spec Footprint**: Elm-inspired reactive architecture ([iced-rs/iced](https://github.com/iced-rs/iced)).
  - **Dual Renderer Support**: Uses `iced_wgpu` (DirectX 12 / Vulkan / Metal) by default, and includes **`iced_tiny_skia` as a built-in CPU software fallback** ([Iced Architecture](https://github.com/iced-rs/iced#overview)). On a low-spec Windows 11 PC with broken or weak integrated GPU drivers, `tiny-skia` guarantees rendering with **~20–45 MB RAM** and near-zero idle CPU usage (retained-style reactive redraws only on input events).
- **Korean IME on Windows 11**: Uses `winit` IME events (`Ime::Enabled`, `Ime::Preedit`, `Ime::Commit`) paired with `cosmic-text` for font shaping and fallback ([iced-rs/iced PRs & Issues on IME](https://github.com/iced-rs/iced/issues?q=is%3Aissue+IME)). Supports Korean `한/영` (`Named::Lang1` / `HangulMode`) and inline preedit rendering, though candidate window positioning and focus transitions between multiple `text_input` cells occasionally exhibit minor quirks.
- **Table/Grid & Form Maturity**:
  - Recent versions added `iced::widget::table` ([docs.rs `iced::widget::table`](https://docs.rs/iced/latest/iced/widget/table/index.html)) and community crates like `iced_table` / `iced_aw` (date pickers, modals, tabs).
  - **Gap for MS Access replacement**: `iced`'s table and form ecosystem is still relatively primitive compared to web data grids or `gpui-component`—implementing inline cell editing, multi-column filter popovers, keyboard cell navigation, and complex master-detail forms requires substantial custom boilerplate.
- **Cross-Compilation from macOS**: Works out of the box with **`cargo-xwin`** (`cargo xwin build --release --target x86_64-pc-windows-msvc`) because `wgpu` compiles WGSL shaders in pure Rust (`naga`) without needing `fxc.exe` ([cargo-xwin](https://github.com/rust-cross/cargo-xwin)).

### 2.2 `egui` / `eframe` (v0.29–v0.31+)
- **Architecture & Low-Spec Footprint**: Pure immediate-mode GUI ([emilk/egui](https://github.com/emilk/egui)).
  - Runs via `eframe` with either **`glow` (OpenGL)** or **`wgpu` (DirectX 12/Vulkan)**. `glow` (via Windows OpenGL / ANGLE) or `wgpu` DX12 uses **~25–50 MB RAM** and produces a tiny **5–10 MB `.exe`**.
  - Because `egui` now defaults to reactive repainting (only redrawing when input events arrive or animations run), idle CPU on low-spec hardware is very low.
- **Korean IME on Windows 11**: Handled via `egui-winit` translating `winit::event::WindowEvent::Ime` into `egui::Event::Ime` ([emilk/egui IME Issues](https://github.com/emilk/egui/issues?q=is%3Aissue+IME)).
  - **Known Friction**: Because `egui` is immediate-mode, widget focus and IME preedit state can desynchronize when pressing `Enter`, `Tab`, or `Backspace` at the end of a Hangul syllable composition, or when clicking directly from one `TextEdit` cell to another (sometimes dropping or duplicating the uncommitted syllable).
- **Table/Grid & Form Maturity**:
  - `egui_extras::TableBuilder` ([docs.rs `egui_extras::TableBuilder`](https://docs.rs/egui_extras/latest/egui_extras/struct.TableBuilder.html)) supports virtualized scrolling (100k+ rows), striped rows, resizable columns, and custom cell widgets.
  - Community crates (`egui_ltable`, `egui-datepicker`) exist, but `egui`'s default visual aesthetic looks like a developer/game tool rather than a standard Windows office/business application, and printing/reporting layouts are non-existent.
- **Cross-Compilation from macOS**: Compiles cleanly to Windows `.exe` directly on macOS via **`cargo-xwin`** (no Windows-only build tools required).

### 2.3 `Slint` (v1.8–v1.10+)
- **Architecture & Low-Spec Footprint**: Declarative `.slint` DSL compiled ahead-of-time to Rust code ([slint-ui/slint](https://github.com/slint-ui/slint), [slint.dev](https://slint.dev)).
  - **Best-in-class Low-Spec Performance**: Features a dedicated CPU **`SoftwareRenderer`** (`i-slint-renderer-software`, selectable via `SLINT_BACKEND=winit-software`) with dirty-region partial rendering, alongside Skia and FemtoVG GPU renderers ([Slint Backends & Renderers](https://docs.slint.dev/latest/docs/slint/guide/platforms/desktop/)). Memory footprint is tiny (**15–35 MB RAM**).
- **Korean IME on Windows 11**: Supported in `LineEdit` and `TextEdit` via `winit`, though edge cases have been tracked when losing window/widget focus mid-composition (e.g., [slint-ui/slint#10861](https://github.com/slint-ui/slint/issues/10861)).
- **Table/Grid & Form Maturity**: Includes `StandardTableView` (with column sorting and row selection) and native-styled `fluent` (Windows 11) widgets (`LineEdit`, `ComboBox`, `CheckBox`, `DatePickerPopup`) ([Slint Built-in Widgets](https://docs.slint.dev/latest/docs/slint/src/language/builtins/widgets/#standardtableview)). However, `StandardTableView` only renders string cells (`[[StandardListViewItem]]`); rich custom cell widgets, inline table editing, and complex filtering require building custom list/repeater layouts in `.slint`.
- **Cross-Compilation from macOS**: Pure-Rust renderers (`femtovg`, `software`) cross-compile from macOS to Windows with **`cargo-xwin`** (Skia backend is harder to cross-compile due to C++ build scripts).
- **Licensing Constraint**: Triple-licensed under **GPLv3**, **Slint Royalty-Free Desktop License** (free for proprietary desktop apps *only* if you include an `AboutSlint` attribution badge/dialog), or a paid Commercial License ([Slint Licensing](https://slint.dev/pricing), [LICENSE.md](https://github.com/slint-ui/slint/blob/master/LICENSE.md)).

---

## 3. Lightweight Hybrid Candidate (`Tauri v2` + Rust Backend + WebView2)

### 3.1 Actual RAM Footprint & Startup Speed on Low-Spec Windows 11
- **Built-in WebView2 Advantage**: Windows 11 ships with the Microsoft Edge **WebView2 runtime pre-installed** as part of the OS (`C:\Program Files (x86)\Microsoft\EdgeWebView\Application`), meaning Tauri v2 installer sizes are tiny (**2–8 MB `.exe`/`.msi`**) and share OS-cached Chromium DLLs in physical memory ([Tauri v2 Windows Installer Docs](https://v2.tauri.app/distribute/windows-installer/)).
- **RAM Footprint Reality**:
  - The Rust backend process itself uses **~10–20 MB RAM** (plus SQLite page cache).
  - However, WebView2 spawns a multi-process Chromium tree (Browser process, GPU process, Renderer process, Network/Utility processes) consuming **~120–250 MB working set RAM** for a single-window business app ([Tauri Discussions & Benchmarks on Memory Usage](https://github.com/tauri-apps/tauri/discussions)).
  - **Comparison vs MS Access**: 64-bit Microsoft Access (Office 365/2021) + ACE Database Engine typically consumes **150–300+ MB RAM** when loading forms and datasheets. Thus, a single-window Tauri v2 app uses roughly the **same or slightly less RAM than MS Access**, and 3–4x less than Electron, while still using ~100–180 MB more than a pure native Rust GUI (`iced`/`egui`/`Slint`).
  - **Low-Memory Tuning**: WebView2 exposes `ICoreWebView2ProcessExtendedInfo` and `CoreWebView2MemoryUsageTargetLevel::Low` via `wry`/`webview2-com` to aggressively trim Chromium caches when idle or minimized ([Microsoft Learn: CoreWebView2.MemoryUsageTargetLevel](https://learn.microsoft.com/en-us/dotnet/api/microsoft.web.webview2.core.corewebview2.memoryusagetargetlevel)).
- **Startup Speed**: Cold start on low-spec Windows 11 is **~0.4–0.9 seconds** (the Rust binary starts in <50ms; initializing the WebView2 controller takes ~300–700ms).

### 3.2 Trade-offs for an MS Access Replacement (Data-Heavy Business App)
- **Korean IME (Decisive Advantage)**: Because WebView2 is Microsoft's own Chromium-based Edge engine integrated with Windows 11's Text Services Framework (`TSF`), **Korean IME works 100% natively**. Every edge case—`한/영` key, Hanja conversion (`한자` key), inline cell editing inside data grids, rapid `Tab`/`Enter` cell navigation during composition, and IME composition events (`compositionstart` / `compositionend`)—is battle-tested for millions of Korean Windows users.
- **Data Tables, Forms & Filtering**: Gives instant access to mature data grids (**TanStack Table**, **AG Grid Community**, **Svelte/Solid/React data grids**) with virtual scrolling, multi-column filtering, column pinning, copy-paste to Excel/TSV, keyboard navigation, and accessible form validation.
- **Printing & Report Generation**: MS Access replacements frequently need invoice/report printing or PDF export. WebView2 provides native `@media print` CSS pagination and `window.print()` / PDF generation out of the box—something every native Rust GUI lacks entirely.
- **Cross-Compilation from macOS**: Tauri v2 **officially documents and supports cross-compiling Windows NSIS installers (`.exe`) directly from macOS** using `cargo-xwin` + Homebrew `llvm` and `nsis` (`cargo tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc`) ([Tauri v2 Docs: Cross-compile Windows Apps from Linux and macOS](https://v2.tauri.app/distribute/windows-installer/#build-windows-apps-on-linux-and-macos)).

---

## 4. Other Notable Lightweight Candidates

### 4.1 Go + Wails (v2 / v3)
- **Overview**: Equivalent to Tauri, pairing a Go backend with Windows 11's WebView2 ([wails.io](https://wails.io), [wailsapp/wails](https://github.com/wailsapp/wails)).
- **Footprint & IME**: Binary size is **~8–15 MB**; RAM footprint is **~130–260 MB** (Go runtime GC heap + WebView2). Korean IME is flawless via WebView2.
- **Cross-Compilation from macOS**: Wails supports cross-compiling to Windows from macOS (`wails build -platform windows/amd64`) using `mingw-w64` (`brew install mingw-w64`) and `nsis` ([Wails Cross-Compiling Guide](https://wails.io/docs/guides/cross-compiling)). However, if using CGO-based `mattn/go-sqlite3`, cross-CGO compilation adds friction compared to pure-Go SQLite (`modernc.org/sqlite`) or Rust's `rusqlite` (`bundled` via `cargo-xwin`).

### 4.2 Go + Fyne
- **Overview**: Pure Go native GUI using OpenGL ([fyne.io](https://fyne.io), [fyne-io/fyne](https://github.com/fyne-io/fyne)).
- **Footprint**: **~30–60 MB RAM**, **~15–25 MB binary**. Requires OpenGL 2.0+ (can struggle on low-end Windows systems without proper OpenGL drivers unless Mesa `opengl32.dll` software rendering is bundled).
- **Korean IME & Widgets**: Has a built-in `widget.Table`, but CJK/Korean IME composition via `go-gl/glfw` on Windows has historically suffered from candidate window mispositioning and composition bugs ([fyne-io/fyne IME issues](https://github.com/fyne-io/fyne/issues?q=is%3Aissue+IME)). Cross-compilation requires CGO + Docker (`fyne-cross`).

### 4.3 Flutter Desktop (Windows)
- **Overview**: Dart AOT + C++ Engine (ANGLE/DirectX 11 or Impeller) ([Flutter Windows Docs](https://docs.flutter.dev/platform-integration/windows/building)).
- **Footprint**: **~60–100 MB RAM** on Windows 11, **~20–30 MB release bundle** (requires distributing multiple DLLs alongside the `.exe`).
- **Korean IME & Cross-Compilation**: Does **not** support cross-compiling to Windows from macOS (`flutter build windows` must run on Windows/MSVC). Korean IME on Flutter Windows Desktop has had long-standing edge cases with rapid Hangul typing and cursor jumps ([flutter/flutter Windows IME issues](https://github.com/flutter/flutter/issues?q=is%3Aissue+korean+IME+windows)).

---

## 5. Summary Comparison Matrix

| Candidate | Idle RAM (Win 11) | Binary Size | Low-Spec GPU Fallback | Korean IME (Win 11) | Data Table & Form Maturity | Print / Report Layout | macOS -> Win Cross-Compile |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Tauri v2 (Rust + WebView2)** | **120–250 MB** *(shared OS WebView2)* | **3–8 MB** | Chromium DX11 / SwiftShader CPU fallback | **Excellent (Native OS TSF)** | **Best-in-class** (TanStack Table, AG Grid, HTML forms) | **Built-in** (`@media print` / PDF) | **Official (`cargo-xwin` + `nsis`)** |
| **Rust + GPUI (`gpui-component`)** | **40–90 MB** | **15–25 MB** | DirectX 11 (Windows WARP fallback) | **Fair / Improving** (Custom Win32 IME, edge cases) | **High** (`DataTable`, `Input`, `DatePicker` in GPUI Kit) | None (requires external PDF crate) | **Blocked on macOS** (`build.rs` needs `fxc.exe` via Wine or CI) |
| **Rust + `iced` (v0.13/0.14)** | **20–45 MB** | **10–18 MB** | **Built-in CPU (`tiny-skia`)** + DX12 (`wgpu`) | **Good** (`winit` + `cosmic-text`, minor popup quirks) | **Basic** (`widget::table`, custom boilerplate for editing/filtering) | None | **Easy (`cargo-xwin`)** |
| **Rust + `egui` (`eframe`)** | **25–50 MB** | **5–10 MB** | OpenGL (`glow`) or DX12 (`wgpu` / WARP) | **Fair** (Immediate-mode focus/commit edge cases) | **Moderate** (`egui_extras::TableBuilder`, game-tool look) | None | **Easy (`cargo-xwin`)** |
| **Rust + `Slint`** | **15–35 MB** | **6–15 MB** | **Built-in CPU (`SoftwareRenderer`)** | **Good** (Minor focus-loss commit quirks) | **Moderate** (`StandardTableView` is string-only; attribution license) | None | **Easy (`cargo-xwin` with software/femtovg)** |
| **Go + Wails** | **130–260 MB** | **8–15 MB** | Chromium DX11 / SwiftShader | **Excellent (WebView2)** | **Best-in-class** (Web frontend grids/forms) | **Built-in** | **Easy (`mingw-w64` + `nsis`)** |
| **Flutter Desktop** | **60–100 MB** | **20–30 MB** *(multi-DLL)* | ANGLE DX11 | **Fair** (Hangul composition cursor bugs) | **Good** (`DataTable`, `pluto_grid`) | Via Dart PDF packages | **Not supported** (Needs Windows host/CI) |

---

## 6. Recommendation for an MS Access Replacement

1. **Primary Recommendation — `Tauri v2` (Rust Backend + SQLite + Lightweight Frontend such as SolidJS/Svelte + TanStack Table)**:
   - **Why it wins for an MS Access replacement**: Data-heavy business applications live or die by **Korean IME reliability during rapid data entry**, **spreadsheet-like keyboard navigation/filtering/sorting in tables**, **date/number form validation**, and **printing/exporting**. WebView2 handles all of these natively with zero custom IME debugging.
   - **Low-Spec Windows 11 Viability**: As long as the app uses a single window, avoids heavy SPA bloat (e.g., using Svelte 5 or SolidJS instead of a bloated React bundle), and pushes SQLite querying/filtering/pagination to the Rust backend, total RAM stays around **130–200 MB**—comparable to or lighter than MS Access itself—even on a 4 GB / 8 GB RAM Windows 11 PC.
   - **macOS Developer Experience**: Develop and test the full UI and Rust+SQLite backend natively on macOS, then run `cargo tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc` locally (or on GitHub Actions) to produce a 3–5 MB Windows `.exe` NSIS installer.

2. **Best Pure-Native Rust Alternative (if ~150 MB WebView2 RAM is strictly unacceptable)**:
   - **If widget completeness matters most**: **Rust + GPUI + `gpui-component`** gives the closest native equivalent to a rich business UI (`DataTable` with 100k+ virtualized rows, `DatePicker`, `Input` masks) and runs on DirectX 11 (~50 MB RAM). However, you must accept building Windows releases via GitHub Actions (due to `fxc.exe`) and testing Korean IME edge cases carefully on real Windows 11 hardware.
   - **If weak GPU / CPU software rendering & stability matter most**: **Rust + `iced`** (with `wgpu` + `tiny-skia` software fallback) Cross-compiles effortlessly from macOS via `cargo-xwin` and uses ~30 MB RAM, at the cost of writing custom Rust view logic for editable data grids and date pickers.
