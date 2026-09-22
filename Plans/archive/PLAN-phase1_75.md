# PLAN — Platform-Aware UI Shell & Settings (ROADMAP Phase 1.75)

**Created:** 2026-09-22T00:00:00+08:00
**Status:** COMPLETE

## Objective

`pixelcad-app` presents a distinct, sketch-conformant window chrome for Linux
and macOS, chosen automatically by OS detection and confirmed once via a
first-run dialog, changeable afterward from a new in-app Settings page. Every
control shown in `docs/design/pixelcad-ui-draft-1.md` exists visibly in the
rendered UI: controls backed by an existing `pixelcad-core` command or
existing GUI state ("LIVE"/"PARTIAL" in `docs/design/ui-redesign-spec.md`)
are wired to their real behavior; controls with no backing implementation
("WIP") are visible, disabled, and labeled "WIP" rather than hidden or faked.
Toggling fullscreen on macOS produces an observably different, testable
layout (Head/Secondary bars separate; panel pop-out buttons appear) so the
distinction is verifiable, not merely notional. Settings persist across
restarts. `crates/core` and `crates/cli` are untouched.

## Implementation Approach

Two structurally different chrome trees (`LinuxChrome`, `MacOsChrome`) are
built as separate Slint components sharing common sub-components
(`CanvasArea`, `LayersPanel`, `HomeToolbar`, a reusable `WipButton`/`WipGroup`
primitive) so canvas/layer/tool logic is not duplicated — the sketch itself
specifies this reuse ("Style from Linux UI", "Secondary 2 from Linux UI").
`AppWindow` keeps the existing public callback/property contract wherever
possible and switches between the two chrome trees on an `in-out property
<string> ui-style`. Both platforms share one visual identity (palette,
type, icon set) — only structural layout differs — matching how cross-platform
creative tools (GIMP, Krita, Blender) present identical theming with
OS-appropriate chrome arrangement; a platform-specific *visual theme* was
considered and rejected as unnecessary complexity the sketch never asked for.

Settings persist to a TOML file via a new `crates/app/src/settings.rs`
module (pure logic, no Slint dependency, testable headlessly) using the
`directories` crate for the OS-appropriate config path. Fullscreen state
uses Slint's real `slint::Window::set_fullscreen`/`is_fullscreen` Rust API
(confirmed present in 1.8), pushed into the UI as a plain `bool` property so
the layout difference is assertable in a headless GUI test without a real
display server. A native macOS unified title bar (traffic-light-integrated
custom chrome) and independent OS-level miniwindow pop-out are rejected for
this phase as requiring platform code beyond Slint's public API — both are
deferred to `BACKLOG.md`, per the maintainer's decision.

## Resources and Dependencies

- [x] `directories` crate (OS-appropriate settings path): yes — new dep, `crates/app/Cargo.toml` only.
- [x] `serde` + `serde_derive` (settings (de)serialization): yes — new dep.
- [x] `toml` crate (settings file format): yes — new dep.
- [x] `slint::Window::is_fullscreen()` / `set_fullscreen()` (Rust API): yes — confirmed on docs.rs for slint 1.8.0.
- [x] Slint 1.8 `TabWidget` std-widget (Settings page tabs): yes — confirmed in the 1.8.0 language reference.
- [x] Open-license icon set for toolbar icons: yes — sourced during Task 1 via the `ui-ux-pro-max` skill's icon domain, vendored as local SVGs (no runtime network dependency).
- [x] Design source of truth: `docs/design/pixelcad-ui-draft-1.md`, `.png`, and `docs/design/ui-redesign-spec.md` — already written this session.

No unconfirmed items.

## Out of Scope (This Session)

- Native macOS unified/custom title bar with integrated traffic lights — deferred to `BACKLOG.md`. Native title bar is kept as-is.
- Independent OS-level miniwindow pop-out for panels — buttons render, visible-but-WIP; no real multi-window spawning.
- Linux drag-and-drop / repositionable toolbar item layout ("box" locations, GIMP/Krita-style) — new `ROADMAP.md` entry only, no implementation.
- Any AI feature, in-GUI CLI command execution, Media bay asset browsing, Find/search indexing, arbitrary-angle rotate, OS-clipboard PNG interop, multi-document/file-tab support — all render as WIP placeholders per `docs/design/ui-redesign-spec.md`; no new backing logic.
- Any change to `crates/core` or `crates/cli`.

## Future-Phase Storage (not built now, but must be findable later)

Not every sketched feature belongs in this phase. Every WIP control must
resolve to exactly one of:

1. **An existing ROADMAP phase** — e.g. AI Agent/AI Chat/command-completion
   → **Phase 4 (AI & Acceleration)**; in-GUI Command Line/AutoCAD-style
   command entry/macros → **Phase 2 (Concept System)**, which already lists
   "command palette, textual command editor, macros"; arbitrary-angle
   Rotate/precise input → **Phase 2**, matching the existing BACKLOG P2–P4
   "exact dimensions"/"numeric move" rows.
2. **A new BACKLOG row tagged with a future phase** when no existing phase
   description covers it (Canvas style, Media bay, Find, multi-document
   tabs, Secondary-tools/AI-chat miniwindow pop-out).
3. **A new ROADMAP line item** when it is a cross-cutting platform feature
   rather than an editing capability (Linux movable/dockable panel layout;
   macOS native unified title bar).

This mapping is written once, as its own document
(`docs/design/ui-roadmap-mapping.md`), in Task 8, so a future Planner
session opening Phase 2 or Phase 4 finds "which UI elements are waiting for
this phase" without re-deriving it from the sketch again.

## Acceptance Criteria

- [ ] AC1: `cargo build --workspace` succeeds, 0 errors, 0 warnings.
- [ ] AC2: `cargo test --workspace` passes, 0 failures, with a strictly greater test count than the pre-redesign baseline of 274.
- [ ] AC3: `git diff --stat` shows zero changes under `crates/core/` and `crates/cli/`.
- [ ] AC4: With no settings file present, launching shows the first-run platform-detection dialog exactly once; an automated test (temp config dir) proves the choice persists to TOML and is honored — not re-prompted — on a simulated next launch.
- [ ] AC5: With `ui-style = "linux"`, every region in the Linux table of `docs/design/ui-redesign-spec.md` is rendered; a headless GUI test asserts every LIVE/PARTIAL control still invokes its pre-existing callback, and every WIP control is present, disabled, and exposes a label/tooltip containing "WIP".
- [ ] AC6: With `ui-style = "macos"`, every region in the macOS table is rendered; native window decorations are untouched (`no-frame` is never set); the same LIVE/WIP rule from AC5 holds.
- [ ] AC7: A headless test sets the pushed `is-fullscreen` property true/false and asserts the macOS layout responds: Secondary bar separates from Head, and pop-out buttons become visible (still disabled/WIP) — proving the windowed/fullscreen difference is real, not cosmetic-only.
- [ ] AC8: The Settings page opens (Basic tab, Style tab, and a disabled Linux-only "Layout — coming later" row), edits persist to the TOML file, and changing "UI style" live-switches the rendered chrome tree without restarting the app.
- [ ] AC9: `ROADMAP.md` gains a new phase entry for this work; `BACKLOG.md` gains entries for every item deferred by this plan (native unified macOS title bar, miniwindow pop-out, Linux layout customization) with stated reasons, in the project's existing format.
- [ ] AC12: Settings → Style offers a **colour scheme** (dark / light / high contrast), a **colour design** (named palette presets), and a **text style** (type scale + optional monospace labels); each applies live to whichever chrome tree is showing, persists across restarts, and is covered by a headless test that changes the setting and asserts the theme tokens changed.
- [ ] AC11: `docs/design/ui-roadmap-mapping.md` exists and assigns every WIP control listed in `docs/design/ui-redesign-spec.md` to exactly one destination: an existing ROADMAP phase, a new phase-tagged BACKLOG row, or a new ROADMAP line item — no WIP item is left unassigned.
- [ ] AC10: `docs/design/DESIGN_SYSTEM.md` exists with concrete color/type/icon tokens used consistently by both chrome trees.

## Constraints

- Files that must not be modified: everything under `crates/core/`, `crates/cli/`.
- Existing `main.slint` callback names/signatures are preserved wherever the control still exists in the new layout; any unavoidable rename is applied to all call sites in the same task, not left dangling.
- No test may be deleted to make the suite pass — only added to or, where genuinely superseded by a renamed/relocated component, replaced 1:1 with an equivalent assertion.
- All new Rust modules follow the existing style: `settings.rs` stays dependency-light and unit-testable without a running window, matching `crates/core`'s own philosophy even though it lives in `crates/app`.
- Vendored icons must carry a license compatible with this repo's MIT license (e.g. MIT, Apache-2.0, CC0, ISC); the chosen set and its license are recorded in `docs/design/DESIGN_SYSTEM.md`.

## Task Breakdown

### Task 1 — Design tokens and icon asset pipeline
- **Files affected:** `docs/design/DESIGN_SYSTEM.md` (new), `crates/app/ui/icons/*.svg` (new, ~25–30 files)
- **What it does:** Produces the shared color/type/spacing token doc (per the `frontend-design` process: plan, review against the brief, note principles) and vendors a single open-license icon set covering every icon referenced in `docs/design/ui-redesign-spec.md` (Save/Undo/Redo/Paste/Cut/Copy/Select/Crop/Pen/Fill/Eraser/Text/Brush/Shapes ×6/Colors/Settings/Layers/Zoom/etc.), sourced via the `ui-ux-pro-max` skill's icon domain search.
- **Done when:** `DESIGN_SYSTEM.md` states concrete hex values, the type scale, and the icon set name + license; every icon named in the redesign spec has a corresponding vendored SVG file.

### Task 2 — Settings persistence and platform detection (Rust, headless)
- **Files affected:** `crates/app/Cargo.toml`, `crates/app/src/settings.rs` (new), `crates/app/src/main.rs`
- **What it does:** Adds `directories`/`serde`/`toml`; implements `AppSettings` (ui_style, theme, accent_color, icon_size, default_canvas_w/h, autosave fields, confirm_before_exit, recent_files_len, first_run_completed, macos_visible_items) with `load()`/`save()`/`config_path()`, and `detect_platform()` (`Linux`/`MacOs`, Windows maps to `Linux` with a flagged WIP note).
- **Done when:** New unit tests cover: default settings round-trip through TOML; missing config file yields defaults with `first_run_completed = false`; `detect_platform()` matches `cfg!(target_os)`. `cargo test -p pixelcad-app settings::` passes.
- **Pre-condition:** Task 1 not required first; independent of it.

### Task 3 — Slint component split (pure refactor, no behavior change)
- **Files affected:** `crates/app/ui/main.slint`, `crates/app/ui/common/canvas.slint` (new), `crates/app/ui/common/layers_panel.slint` (new), `crates/app/ui/common/wip.slint` (new)
- **What it does:** Extracts the existing canvas (image + touch handling + pan/zoom) and layers panel into standalone imported components; adds the reusable `WipButton`/`WipGroup` primitive (disabled control + "WIP" accessible label/tooltip) used by every subsequent task. No new features; existing layout may look unchanged.
- **Done when:** `cargo test --workspace` passes with the *same* test count and content as before this task (regression-only), proving the split preserved behavior exactly.

### Task 4 — HomeToolbar component
- **Files affected:** `crates/app/ui/common/home_toolbar.slint` (new), `crates/app/src/main.rs` (call-site updates only)
- **What it does:** Builds the Home-tab toolbar per the Linux table's Home-tab rows: Paste/Cut/Copy/Select/Default-select/Crop&Resize/Pen/Fill/Eraser/Text/Brush/Shapes/Fill-Outline/Width/Colors wired to existing callbacks; AI rendered via `WipButton`.
- **Done when:** A headless test drives each LIVE/PARTIAL Home-tab control through `HomeToolbar` and asserts the same engine-level effect as the pre-redesign flat toolbar (e.g. clicking Select still sets the select tool); the AI button is present, disabled, labeled "WIP".

### Task 5 — Linux chrome
- **Files affected:** `crates/app/ui/linux/chrome.slint` (new), `crates/app/ui/main.slint`
- **What it does:** Assembles Title (icon-menu WIP, Save/Redo/Undo/Filename/Settings), Secondary1 tabs (Home live via `HomeToolbar`; Tool/View/Custom show a `WipGroup` panel; RMB-expand is WIP), File-tabs row (one real tab bound to the current document, `+` WIP), Main row (cursor X,Y, `CanvasArea`, Rotate/Angle WIP, `LayersPanel`), Style row (Canvas-style/Media-bay/New-style as `WipGroup` dropdowns listing their sketch sub-items), Secondary2 row (Find/Command-Line/AI-Chat/Secondary-tools, centered per the sketch, all `WipGroup`), Tail row (Move WIP, Select live, Canvas-size live + format WIP, Zoom live with a numeric stepper).
- **Done when:** `AppWindow` with `ui-style: "linux"` renders every region in the Linux table of `docs/design/ui-redesign-spec.md`; existing behavioral GUI tests pass unmodified; new tests assert WIP controls are disabled and LIVE controls still fire pre-existing callbacks.

### Task 6 — macOS chrome, including the windowed/fullscreen difference
- **Files affected:** `crates/app/ui/macos/chrome.slint` (new), `crates/app/ui/main.slint`, `crates/app/src/main.rs`
- **What it does:** Builds the in-content Head+Secondary bar (Save/Undo/Redo/Files▾ — Files▾ has Open/Save/Save-project live, New/Export/Recent WIP entries) merged into one row when `is-fullscreen` is false; when true, Secondary visually separates onto its own row and the vertical-tab pop-out buttons become visible (disabled/WIP). Vertical tab sidebar: File (Media-bay/File-tree, WIP), Main (reuses `HomeToolbar`), Style (reuses Linux Style-row `WipGroup`), Secondary (reuses Linux Secondary1 tab-picker), Tool (Find + Linux Secondary2, WIP), Command (WIP), Layers (`LayersPanel` + WIP layer-settings), AI (WIP). Bottom-right Zoom (live). Rust: a "Toggle Fullscreen" callback calls `window().set_fullscreen(!window().is_fullscreen())` and pushes the resulting `is-fullscreen` bool into the UI. Native window decorations are left untouched (`no-frame` never set).
- **Done when:** `AppWindow` with `ui-style: "macos"` renders every region in the macOS table; a headless test sets `is-fullscreen` true then false and asserts the Head/Secondary merge state and pop-out-button visibility differ between the two; native decorations are unmodified (code review: no `no-frame` anywhere in `macos/chrome.slint`).

### Task 7 — First-run dialog, Settings page, and theming

> **Scope addition (maintainer request, mid-execution):** the Settings page
> must also expose colour scheme, colour design and text style for both the
> Linux and macOS windows. This extends `Theme` from a single dark/light
> flag into a scheme × design matrix plus a type scale, and adds the
> matching fields to `AppSettings`. Logged as a deviation in `PROGRESS.md`.
- **Files affected:** `crates/app/ui/common/first_run_dialog.slint` (new), `crates/app/ui/settings/settings_panel.slint` (new), `crates/app/ui/main.slint`, `crates/app/src/main.rs`
- **What it does:** On launch, `main.rs` calls `settings::load()`; if `first_run_completed` is false, the UI shows an in-window modal overlay: "Detected <OS> — use the matching interface style?" with **Use detected**/**Choose manually**; on Windows, an added note recommends the WSL Linux build. The choice is saved and `first_run_completed` set true. The Settings page (opened from the Title/Files▾ area) uses a `TabWidget` with **Basic** (default canvas size, autosave on/off + interval, confirm-before-exit, recent-files count) and **Style** (UI style Auto/Linux/macOS, theme, accent color, icon size; macOS gets per-item show/hide toggles) tabs, plus a disabled Linux-only "Layout — coming later" row linking to the new ROADMAP item. Changing "UI style" updates `ui-style` live and saves immediately.
- **Done when:** An integration test using a temporary config directory proves: first run shows the dialog once; the persisted choice is honored on a simulated second load without re-prompting; opening Settings and changing UI style live-switches the rendered chrome (assert the child tree changes) and persists to disk.

### Task 8 — ROADMAP/BACKLOG updates and future-phase mapping
- **Files affected:** `ROADMAP.md`, `BACKLOG.md`, `docs/design/ui-roadmap-mapping.md` (new)
- **What it does:** Inserts a new phase (e.g. "Phase 1.75 — Platform-Aware UI Shell & Settings") before Phase 2, following the Phase 1.5 precedent. Writes `docs/design/ui-roadmap-mapping.md`: a table assigning every WIP control in `docs/design/ui-redesign-spec.md` to exactly one of (a) an existing ROADMAP phase — AI Agent/AI Chat/command-completion → Phase 4; in-GUI Command Line/AutoCAD-style command/macros → Phase 2; arbitrary-angle Rotate → Phase 2 (matches existing "exact dimensions"/"numeric move" BACKLOG rows) — (b) a new phase-tagged `BACKLOG.md` row when no phase description covers it yet (Canvas style, Media bay, Find, multi-document tabs, Secondary-tools/AI-chat miniwindow pop-out), or (c) a new `ROADMAP.md` line item for cross-cutting platform features (Linux movable/dockable panel layout; macOS native unified title bar). `BACKLOG.md` and `ROADMAP.md` are then updated to match this mapping so the two files and the mapping doc never disagree.
- **Done when:** `ROADMAP.md`'s Status Log table includes the new phase, plus the two new cross-cutting line items; `BACKLOG.md` contains every deferred item tagged with its destination phase (or explicitly untagged/general-polish where no phase applies, e.g. the native macOS title bar); `docs/design/ui-roadmap-mapping.md` has zero unassigned rows (AC11).

### Task 9 — Full regression sweep and close-out
- **Files affected:** `PROGRESS.md` (new/updated), `README.md` (if it documents GUI shortcuts/layout)
- **What it does:** Runs `cargo build --workspace` and `cargo test --workspace`; fixes any regression; updates `README.md` if its GUI description is now stale; writes the final `PROGRESS.md` log per the Autopilot schema; runs the full AC1–AC10 checklist with evidence.
- **Done when:** All Acceptance Criteria in this document are checked PASS with cited evidence (test output, `git diff --stat`, file excerpts).

## Resumption Note

No work started yet. First action on execution start: Task 1 (design tokens
+ icon sourcing), since Tasks 4–7 depend on the icon set and token doc it
produces. Task 2 (settings/platform Rust module) has no dependency on Task 1
and may run in parallel/before it if convenient.
