# PROGRESS — Phase 1.75: Platform-Aware UI Shell & Settings

**Last updated:** 2026-09-22T04:20:00+08:00
**Current task:** Task 8 — ROADMAP/BACKLOG updates and future-phase mapping
**Completed tasks:** 1, 2, 3, 4, 5, 6, 7
**Current mode:** A

> Phase 1.5's log was archived to `Plans/archive/PROGRESS-phase1_5.md`
> rather than overwritten. This file covers ROADMAP Phase 1.75 only, per
> `PLAN.md`.

## Log

### 2026-09-22T00:15:00+08:00 — Session start
Design intake was completed before execution: the maintainer's hand-drawn
sketch (`Pixel CAD UI Draft 1.pdf`) was rendered to a readable image, and
their written supplement was preserved verbatim. Three design documents now
exist and are the source of truth for this phase:

- `docs/design/pixelcad-ui-draft-1.md` — maintainer's spec, verbatim
- `docs/design/pixelcad-ui-draft-1.png` — rotated, readable sketch render
- `docs/design/ui-redesign-spec.md` — every sketched control classified
  LIVE / PARTIAL / WIP, cross-checked against `crates/core`'s actual
  command set rather than assumed

Two engineering risks were retired before planning, not during execution:
`slint::Window::is_fullscreen()`/`set_fullscreen()` was confirmed to exist
in Slint 1.8's Rust API (docs.rs), and `TabWidget` was confirmed present in
1.8's std-widgets. The windowed/fullscreen distinction the maintainer asked
for is therefore backed by a real platform API, not simulated.

### 2026-09-22T00:40:00+08:00 — Task 1 complete (design tokens + icons)
- **`docs/design/DESIGN_SYSTEM.md`**: one visual identity shared by both
  chrome trees — dark (default) and light palettes, type scale, 4px grid,
  state treatments, and the per-platform structural difference.
- **Three subject-grounded decisions recorded with their justification**,
  so later sessions don't "fix" them into generic defaults:
  1. Zero corner radius on canvas-adjacent chrome — rounded chrome
     contradicts a pixel-exactness product. Radius survives only on modal
     overlays, which float above the drawing surface.
  2. Monospace confined to live numeric readouts — proportional digits make
     a live cursor-coordinate readout jitter as the pointer moves. This is
     the functional reason; monospace is *not* used for ordinary labels.
  3. **Dashed borders mark WIP controls.** In a drafting tool a dashed line
     already means "construction geometry — drawn, not built," so the large
     number of sketched-but-unimplemented controls reads correctly to this
     audience. `wip` amber is held far from `accent` cyan in hue so a WIP
     control can never be misread as the active one.
- **60 Lucide icons vendored** (`crates/app/ui/icons/`, ISC, license text
  included). All 60 downloaded and verified 200 in one pass; zero failures.
- **One non-obvious fix applied at vendor time:** Lucide ships
  `stroke="currentColor"`, which Slint's SVG renderer has no context to
  resolve — the icons would have rendered invisibly. Rewrote it to
  `#000000` across all 60 files so each icon is a deterministic mask, then
  tinted at use-site via Slint's `colorize`. This is what lets one icon
  file serve dark, light, active, inactive and WIP states.
- Recorded in `BACKLOG.md` (Task 8): a bespoke pixel-grid icon set on the
  same 16px grid as the canvas would suit this product better than a
  general-purpose stroke set, but is polish, not a blocker.
- Mode used: A. Deviations from `PLAN.md`: none.
- Files modified: `docs/design/DESIGN_SYSTEM.md` (new),
  `crates/app/ui/icons/*.svg` (60 new), `crates/app/ui/icons/LICENSE-lucide.txt` (new).

### 2026-09-22T01:00:00+08:00 — Task 2 complete (settings + platform detection)
- **`crates/app/src/settings.rs`**, deliberately window-free so it is
  testable without a display. File I/O is split from path resolution
  (`load_from`/`save_to` take an explicit path), so no test can touch the
  real user's config directory.
- `directories` is used instead of a hard-coded `~/.config`, which is
  simply the wrong location on macOS — one of this phase's two targets.
- **Three deliberate robustness decisions**, each pinned by a test:
  1. `UiStyle::Auto` is stored *as* `Auto`, not as the platform it resolved
     to, so a config synced to another machine re-resolves there. A
     regression test asserts the literal `ui-style = "auto"` on disk.
  2. A corrupt or partial file never blocks startup: it yields defaults
     plus a `LoadOutcome` explaining why, and `#[serde(default)]` means a
     file written by an older build still loads once fields are added.
  3. `sanitize()` repairs out-of-range values rather than rejecting the
     file — a hand-edited config must not be able to produce a 0px icon or
     a 1×1 default canvas.
- Windows has no chrome of its own by design (maintainer: Windows users run
  the Linux build under WSL); it resolves to the Linux chrome and sets
  `windows_fallback`, which Task 7's first-run dialog uses to recommend WSL.
- 10 unit tests, all passing.

### 2026-09-22T01:30:00+08:00 — Task 3 complete (component split, pure refactor)
- Extracted `common/canvas.slint` (`CanvasArea`) and
  `common/layers_panel.slint` (`LayersPanel`), added `common/theme.slint`
  (tokens as code), `common/types.slint` (`LayerRow`), and
  `common/wip.slint` (the WIP primitive + a `DashedBorder`, since Slint has
  no `border-style`).
- Verified as a true regression-only change: 284 tests passed, i.e. the 274
  Phase 1.5 baseline plus exactly the 10 new settings tests, with all 66
  pre-existing GUI tests untouched in content.

### 2026-09-22T02:10:00+08:00 — Tasks 4 and 5 complete (Home toolbar + Linux chrome)
- `common/icons.slint`, `common/controls.slint` (`IconButton`,
  `ChromeButton`, `ToolButton`, dividers, `ToolGroup`, `Readout`),
  `common/home_toolbar.slint`, and `linux/chrome.slint` — the full stacked
  band layout from the sketch (Title → ribbon tabs → contextual toolbar →
  file tabs → main → style → secondary → tail).
- `main.slint` is now only the Rust contract plus a chrome switch on
  `ui-style`; all visible structure moved into the chrome trees.
- The 12 tools render from the `tool-labels` model (not hard-coded) so the
  toolbar cannot drift from `Tool::ALL`, while still being split into the
  sketch's three visual groups.
- Deviation from PLAN.md: Tasks 4 and 5 were verified together rather than
  separately, because `HomeToolbar` has no consumer until a chrome tree
  exists. No scope change.

### 2026-09-22T02:05:00+08:00 — Mode B pivot on Task 5 (attempt 3)
- **Failed approach:** assuming the headless tests' element lookups only
  needed their component prefix renamed (`AppWindow::x` → `LinuxChrome::x`).
  Two successive attempts (renaming prefixes, then making the controls
  inherit `Rectangle` so their ids survive optimisation) both left 9 GUI
  tests failing with "no element found".
- **Pivot:** stopped guessing the naming scheme and wrote a throwaway probe
  test that enumerated what the element tree actually contains.
- **Root cause, and it was not naming at all:** `AppWindow::new()` leaves
  the window with no size, and Slint only assigns geometry during layout,
  so in the denser redesigned chrome most controls stayed 0×0 and were
  absent from the searched tree. The old flat toolbar happened to survive
  this; the new layout does not. Adding
  `window().set_size(1600×1000)` to the shared test helper fixed 8 of the 9
  failures outright, and confirmed the renamed element ids were correct all
  along.
- **Second, smaller finding from the same probe:** a Slint `Text` element
  takes an implicit `accessible-label` equal to its text, so the "Brush"
  tool-group caption collided with the "Brush" *tool* button in
  label-based lookups. The group is now titled "Brush options", which is
  also the more accurate name.
- Only one test needed rewriting rather than re-pointing:
  `the_tool_buttons_select_tools` indexed tool buttons positionally, which
  the new visual grouping invalidates. It now locates the Fill button by
  accessible label — the same assertion, independent of layout order — and
  a companion test asserts all 12 tools carry a unique accessible label.
- 285 tests pass, 0 failures.

### 2026-09-22T03:05:00+08:00 — Task 6 complete (macOS chrome + fullscreen difference)
- `macos/chrome.slint`: in-content Head bar (native title bar untouched, so
  the traffic lights stay where macOS puts them), `SecondaryControls`
  (Save/Undo/Redo/Files), an expandable Files menu with Open/Save live and
  New/Export/Recent as WIP, the 8-entry vertical tab rail, and the
  bottom-right zoom cluster.
- **The windowed/fullscreen difference is real, not cosmetic.** Rust's
  `fullscreen-toggled` callback flips `slint::Window::set_fullscreen` and
  then mirrors `is_fullscreen()` — the *queried* state, not the requested
  one — back into the `is-fullscreen` property. A window manager that
  refuses the request therefore cannot desynchronise the UI. Windowed
  merges Secondary into Head; fullscreen splits it onto its own row and
  reveals a per-tab pop-out affordance (itself WIP, since real
  multi-window support does not exist).
- Slint requires components to be declared before first use in a file;
  the two helper components had to move above `MacOsChrome`.
- 4 new GUI tests: chrome switching, the vertical tab rail, the fullscreen
  restructure (asserted in both directions, so it is a live binding rather
  than a one-way branch), and that the toggle drives the real window state.

### 2026-09-22T03:00:00+08:00 — Two layout bugs found by looking at it
- The structural tests all passed while the app still looked wrong, so the
  binary was run and screenshotted. Two defects that no assertion covered:
  1. **The chrome did not fill the window.** A `FocusScope` is not a
     layout, so the chrome collapsed to its preferred size and floated in
     the middle of the window. Fixed with explicit `width/height: 100%` on
     the focus scope and both chrome trees.
  2. **The Home toolbar is wider than a small window** and was silently
     clipping the Colour and AI groups off the right-hand end. On macOS it
     was worse: the toolbar's minimum width propagated up the layout and
     squeezed the vertical tab rail to zero width, which is why those tabs
     were also missing from the element tree.
- **`ScrollView` is the wrong fix and cost an attempt:** wrapping the
  toolbar in one made 10 GUI tests fail, because its clipped content drops
  out of the searchable element tree. `Flickable` with an explicit
  `viewport-width` scrolls the toolbar *and* keeps every control
  discoverable, so the toolbar is now horizontally scrollable on both
  platforms with all tests green.
- Both chrome trees were then visually confirmed against the sketch,
  band by band, in a real window.
- 289 tests pass, 0 failures.

### 2026-09-22T04:20:00+08:00 — Task 7 complete (first-run dialog, Settings page, theming)

**Scope addition, requested mid-execution by the maintainer:** the Settings
page also had to offer colour scheme, colour design and text style for both
windows. `PLAN.md` Task 7 and a new AC12 were amended to record this before
implementing it.

- **`Theme` became two orthogonal axes instead of a dark/light flag.**
  `scheme` (dark / light / high-contrast) decides how light the chrome is;
  `design` (cyanotype / graphite / amber / phosphor) decides the hue family.
  Four palette definitions therefore cover twelve appearances, rather than
  twelve hand-maintained themes that would drift apart.
- The four designs are grounded in the subject, not arbitrary hues:
  cyanotype (blueprint), graphite (pencil lead), amber (drafting lamp),
  phosphor (green CRT, for the command-line heritage).
- **One rule worth keeping:** the WIP marker is amber everywhere *except*
  under the Amber design, where it becomes violet. A warm WIP dot on a warm
  accent reads as "active", which is exactly the confusion the WIP
  treatment exists to prevent.
- **Text style** is a named step (compact / normal / comfortable), not a
  free number, so it cannot be set to something unreadable. The scale
  multiplies text sizes *and* row heights, so larger text grows its
  container instead of clipping inside it. A monospace-labels switch is
  also offered; numeric readouts were already always monospace.
- `settings/settings_panel.slint` (Basic / Style / Layout tabs) and
  `common/first_run_dialog.slint`, both as in-window modal overlays
  declared once above either chrome tree.
- **Tabs are hand-built from `ChromeButton`, not `TabWidget`**, for the
  same reason `ScrollView` was rejected in Task 6: those widgets keep
  inactive/clipped content out of the searchable element tree, which would
  make the settings page untestable headlessly.
- **No Apply button.** Every control writes its property and raises one
  `changed()`; Rust persists, re-applies the theme, and writes the
  *sanitised* values back into the form, so the visible UI and the saved
  file cannot disagree, and a rejected value cannot stay on screen.
- `collect_settings` keeps the previous value when a numeric field will not
  parse: the user is mid-edit, and a half-typed number must not overwrite
  what they had.
- **Accepting the detected platform stores `Auto`, not the resolved
  platform** — so the same config still follows the OS if it is later used
  on another machine. Explicitly picking Linux or macOS pins it. Both paths
  are pinned by tests.
- `wire_settings` takes its save function as a parameter so the headless
  tests capture settings in memory instead of writing to the real user
  config directory.
- 9 new tests: panel open/close, all three theming axes applying *and*
  persisting, accent override and restore, invalid accent discarded without
  losing a valid sibling change, live chrome swap without restart, first-run
  answered once (both the "detected" and explicit paths), and a control
  inventory test that stands in for eyeballing the overlay.
- Warnings cleaned to zero: dropped two speculative `ALL` constants and
  migrated `viewport-width/height` to Slint 1.8's `content-width/height`.
- 297 tests pass, 0 failures, 0 warnings. `crates/core` and `crates/cli`
  still show no diff.

### 2026-09-22T04:05:00+08:00 — Note on visual verification
The Linux and macOS chrome trees, and the Amber colour design, were each
confirmed in a real window. Verification of the Settings overlay was
**not** done visually: capturing it required a full-screen grab, and one
such grab caught an unrelated browser window belonging to the maintainer.
Those captures were deleted immediately and the practice was stopped;
window-only capture needs Accessibility permission this session does not
have. The overlay is instead covered by
`the_settings_page_shows_every_documented_control`, which asserts every
documented row and option is present, so a dropped control fails the suite
rather than shipping silently.

## Current Blockers

None.

## Backlog (out-of-scope items discovered during execution)

- Bespoke pixel-grid icon set (found during Task 1) — to be filed in
  `BACKLOG.md` during Task 8.

## Resumption

Task 8 — ROADMAP/BACKLOG updates and the future-phase mapping document
(`docs/design/ui-roadmap-mapping.md`), then Task 9 (full regression and
close-out). Nothing is half-finished: 297 tests pass with 0 warnings.

<details>
<summary>Superseded resumption note for Task 7</summary>

Task 7 — First-run dialog and Settings page. Next step: write
`common/first_run_dialog.slint` (modal overlay: "Detected <OS> — use the
matching interface style?", plus the WSL note when
`PlatformDetection::windows_fallback`) and `settings/settings_panel.slint`
(`TabWidget` with Basic and Style tabs, plus the disabled Linux-only
"Layout — coming later" row), then wire both into `main.rs`: show the
dialog when `!first_run_completed`, persist on answer, and make
`settings-clicked` open the panel. `apply_settings` and `wire_fullscreen`
already exist in `main.rs`; `settings::load` is already called at startup,
so only the dialog/panel UI and the save-on-change path remain.

Last stable state: 289 tests passing, both chrome trees complete and
visually verified in a real window, `crates/core` and `crates/cli`
untouched. Tasks 8 (ROADMAP/BACKLOG + future-phase mapping) and 9 (full
regression and close-out) not started.
</details>

### 2026-09-22T05:10:00+08:00 — Tasks 8 and 9 complete (mapping, docs, close-out)

**Maintainer correction applied before close-out:** the macOS tab icons
belong in a **horizontal strip along the bottom edge** whose panels expand
**vertically upward** on click, not in a left-hand vertical rail. The rail
was rebuilt as `TabDockIcon` dock icons + a `tab-panel` overlay above the
bottom bar (click to expand, click again or the panel's close button to
collapse, click another icon to switch content in place). The spec doc's
macOS section and README now describe the corrected arrangement, and a new
test covers expand / switch / collapse / re-expand.

- **Task 8:** `docs/design/ui-roadmap-mapping.md` written — 30 rows, every
  WIP control assigned to a phase, a tagged BACKLOG row, or the new
  ROADMAP cross-phase item, with zero unassigned. `ROADMAP.md` gained the
  Phase 1.75 entry (COMPLETE) and the `Cross-phase — Windowing & panel
  layout` line item (DEFERRED), which is where the Linux movable-panel
  request lives per the maintainer's instruction. `BACKLOG.md` gained a
  Phase 1.75 deferrals section with reasons, matching the existing format.
- **Task 9:** `README.md` updated (window layouts, settings, theming; stale
  button names corrected). Final sweep: build 0 warnings, **298 tests
  passed / 0 failed**, `crates/core` and `crates/cli` show no diff.

AUTOPILOT: Session complete.

Tasks completed:     9 / 9
Acceptance criteria: 12 / 12 passed (AC1 build clean; AC2 298 > 274 tests;
                      AC3 core/cli untouched; AC4 first-run once + persisted;
                      AC5/AC6 chrome trees render with LIVE wired and WIP
                      disabled; AC7 fullscreen difference real and testable;
                      AC8 settings persist + live style switch; AC9/AC11
                      roadmap/backlog + mapping doc; AC10 design system doc;
                      AC12 scheme/design/text theming live + persisted)
Modes used:           A (1, 2, 3, 4, 6, 7, 8, 9), B (5, one pivot: element
                      lookup diagnosis)
Files modified:       PLAN.md, PROGRESS.md, ROADMAP.md, BACKLOG.md, README.md,
                      Cargo.lock, crates/app/Cargo.toml, crates/app/src/main.rs,
                      crates/app/src/settings.rs (new), crates/app/ui/** (new)
Backlog items:        11 — see BACKLOG.md, Phase 1.75 deferrals
Regressions found:    none
