# PixelCAD — Roadmap

**Product:** PixelCAD (`pixelcad`) — an open-source, lightweight, pixel-first design
studio for technical/blueprint work. Every action is a replayable command, usable
from GUI or CLI, growing toward simple CAD and AI assistance.

**Platforms:** macOS ARM64 (primary), Linux ARM64 (deferred until a reference
machine exists). Distribution goal: one-line install (`cargo install pixelcad`,
later GitHub release binaries + install script).

**Stack:** Rust workspace. `core` (document + command engine, headless),
`app` (Slint shell), `cli` (headless automation). CAD kernel and AI runtimes
enter as isolated modules in their own phases.

---

## Session Protocol (one phase per session)

Each development session follows this cycle:

1. **Plan** — invoke the Planner profile (`Profiles/Planner/planner.md`).
   It reads this file, drafts `PLAN.md` for the next `PENDING` phase,
   and you lock it.
2. **Execute** — invoke the Autopilot profile (`Profiles/Autopilot/profile.md`).
   It reads `PLAN.md` + `PROGRESS.md` and executes until acceptance criteria pass.
3. **Close** — when all criteria pass:
   - Move `PLAN.md` → `Plans/archive/PLAN-phase<N>.md`
   - Update this file: phase status → `COMPLETE`, record completion date
   - Carry unresolved `BACKLOG.md` items forward
   - This step (plus starting step 1 for the next phase) is automated by the
     project-local `next-phase` skill at `.agents/skills/next-phase/SKILL.md`.

Resuming a half-finished phase: skip step 1; Autopilot resumes from `PROGRESS.md`.

---

## Phases

### Phase 0 — Bootstrap & Command Engine        [COMPLETE]
Repo bootstrap, Cargo workspace, typed command engine, single-layer pixel
document, minimal Slint canvas (pencil, palette, pan/zoom), headless CLI replay
to PNG.
**Exit criterion:** the same command script produces byte-identical PNG output
via CLI and test harness; GUI drawing is recorded as a replayable script.

### Phase 1 — Drawing MVP                       [COMPLETE]
Brush, eraser, fill, eyedropper, rectangle/line tools, selections, multiple
layers, undo/redo polish, palette management, keyboard shortcuts, versioned
project file (.pxc archive), PNG import.
**Exit criterion:** PixelCAD is a usable, focused pixel/blueprint editor with
no AI or CAD — sufficient to draw a historical ship blueprint end to end.

### Phase 1.5 — Paint Parity                    [COMPLETE]
The P1 tier of the Microsoft Paint feature matrix: mask-based selections
(lasso, select-all, move, nudge, delete, flip, rotate, scale), clipboard
(cut/copy/copy-composite/paste/duplicate), brush shapes, fill tolerance,
ellipse/rounded-rectangle/polygon/arrow/polyline, bitmap text, canvas crop
and resize, layer duplicate and merge, rulers, grid toggle, fit/actual-size
zoom, recent colours.
**Exit criterion:** anything a user can do in Microsoft Paint they can do in
PixelCAD, and can replay from a script. P2–P4 tiers stay in `BACKLOG.md`.

### Phase 1.75 — Platform-Aware UI Shell & Settings  [COMPLETE]
The maintainer-designed window chrome for Linux and macOS (sketch +
spec in `docs/design/`): platform detection with a one-time confirmation
dialog, a persisted Settings page (Basic / Style / Layout), colour scheme
× colour design × text style theming, a real windowed/fullscreen layout
difference, and every sketched-but-unimplemented feature rendered visibly
as a disabled WIP control. The full LIVE/PARTIAL/WIP classification lives
in `docs/design/ui-redesign-spec.md`, and the assignment of each WIP item
to a future phase lives in `docs/design/ui-roadmap-mapping.md`.
**Exit criterion:** both chrome trees render from one shared component set;
all 297 tests pass with zero warnings; `crates/core` and `crates/cli` are
untouched.

### Phase 2 — Concept System                    [PENDING]
Vector shapes, guides, snapping, grid/symmetry tools, annotations, command
palette, textual command editor, macros, local automation API.
**Exit criterion:** a design can be created reproducibly through GUI or CLI.

### Phase 3 — Simple CAD                        [PENDING]
2D sketch entities, dimensions, constraints; extrude/revolve/booleans;
feature history; STEP/STL/OBJ/glTF export. CAD kernel isolated behind a
service interface (module may be C++/OCCT sidecar).
**Exit criterion:** simple manufacturable concept parts export reliably.

### Phase 4 — AI & Acceleration                 [PENDING]
ONNX Runtime adapter with provider discovery (Core ML on macOS; CUDA only on
supported Linux/NVIDIA hardware; CPU fallback). AI proposes commands with
preview/approval/rollback. Background job scheduler.
**Exit criterion:** AI features degrade cleanly to CPU or become unavailable
without affecting editing.

### Phase 5 — Advanced Rendering: Metal first   [PENDING]
PBR materials, progressive preview, Metal ray tracing on supported Macs,
EXR output. GPU code written against `wgpu` (Metal backend) with native Metal
escape hatches only where required (ray-tracing acceleration structures).
**Exit criterion:** rendering remains optional and cannot block or corrupt
authoring; **the Metal/macOS version is publishable on its own.**

### Phase 5b — Vulkan rendering (Linux)         [DEFERRED]
Activates after the Phase 5 macOS release. Enables the `wgpu` Vulkan backend
on the Linux ARM64 reference machine; Vulkan ray tracing on compatible GPUs.
OpenGL/GLES remains a display-only fallback, never a rendering target.
**Exit criterion:** feature parity with the Metal version except where
hardware capabilities differ (capability query reports the difference).

### Phase 6 — Windows (limited capability)      [DEFERRED]
Opportunistic port via `wgpu` DX12 backend. Explicitly limited scope:
core editing + export; advanced rendering/AI acceleration best-effort only.
No Windows-specific code outside the adapter layer.
**Exit criterion:** core drawing workflow passes the determinism test on
Windows; unsupported features degrade cleanly per the invariants.

### Phase 7 — Colorful Update                  [PENDING]
A Minecraft 1.12 "World of Color"-style color-system update: a systematic
16-color (and beyond) palette architecture, per-project custom palettes,
full color pickers (RGB/HSV/hex), color-adjustment and recolor commands
(hue/saturation/value shift, replace-by-color), gradients and dithering, and
named color libraries consistent across GUI, CLI, and scripts.
**Exit criterion:** every color a user can choose is expressible and
replayable as a command; a whole design's color scheme can be transformed
from a single script and survives save/reload byte-identically.

### Cross-phase — Linux ARM64 validation        [DEFERRED]
Activates when a reference Linux ARM64 machine (or UTM VM) exists. Adds CI
and the determinism criterion across both platforms. Pre-requisite for
Phase 5b.

### Cross-phase — Windowing & panel layout      [DEFERRED]
The chrome customisation work Phase 1.75 deliberately did not build:
Linux users can move dockable panels and toolbars to new locations
("box" arrangement, GIMP/Krita-style, via Settings → Layout); macOS users
can add or remove items but not reposition them; both platforms gain real
multi-window support so the macOS fullscreen tab pop-out and the
Secondary-tools miniwindow toolbox stop being WIP. See
`docs/design/ui-roadmap-mapping.md` rows 20–24.
**Exit criterion:** a panel layout can be changed, persisted and restored
without restarting the document session.

---

## Invariants (apply to every phase)

- Every user action becomes a serializable, replayable command.
- `core` never depends on UI, GPU, AI, or CAD-kernel crates.
- Failure of an optional subsystem (AI, GPU, CAD) must never corrupt the
  document or disable basic editing.
- The platform must never block the drawing tool — scope gates are enforced
  per phase; discoveries go to `BACKLOG.md`, not into the current phase.

## Status Log

| Phase | Status | Locked | Completed |
|-------|--------|--------|-----------|
| 0 | COMPLETE | 2026-09-19 | 2026-09-19 |
| 1 | COMPLETE | 2026-09-19 | 2026-09-20 |
| 1.5 | COMPLETE | 2026-09-20 | 2026-09-20 |
| 1.75 | COMPLETE | 2026-09-22 | 2026-09-22 |
| 2 | PENDING | — | — |
| 3 | PENDING | — | — |
| 4 | PENDING | — | — |
| 5 (Metal) | PENDING | — | — |
| 5b (Vulkan) | DEFERRED | — | — |
| 6 (Windows) | DEFERRED | — | — |
| 7 (Colorful) | PENDING | — | — |
