# PLAN — Phase 1.5: Paint Parity

**Created:** 2026-09-20T04:00:00+08:00
**Status:** COMPLETE
**Completed:** 2026-09-20T05:30:00+08:00

> All 19 acceptance criteria PASS. Evidence is in `PROGRESS.md`'s close-out
> entry and reproducible with the commands in `README.md`.
**Product:** PixelCAD (`pixelcad`)
**ROADMAP phase:** Phase 1.5 — "Paint Parity" (inserted; completes the
Drawing MVP rather than starting Phase 2's Concept System)

## Objective

PixelCAD gains the **P1 tier** of the Microsoft Paint feature matrix, so
that anything a user can do in Paint they can do in PixelCAD — and, unlike
Paint, can replay from a script. Concretely: a mask-based selection that can
be moved, nudged, deleted, flipped, rotated, scaled, cut, copied and pasted;
lasso and select-all; brush shapes; fill tolerance; ellipse, rounded
rectangle, polygon, arrow and polyline; bitmap text; canvas crop and resize;
layer duplicate and merge; rulers, grid toggle, fit/actual-size zoom and
recent colours in the GUI. Every one of those remains a serializable,
replayable command, and the two existing sample files must still render
byte-identically.

## Implementation Approach

Extend, do not replace. The single structural change is that `SelectionRect`
becomes `Selection` — a bounding box **plus an optional per-cell coverage
mask** — because lasso, magic-wand-shaped regions and "delete what I
selected" all collapse into one concept once a selection can be an arbitrary
shape. Everything else is additive: new `Command` variants executed by the
existing engine, parsed by the existing grammar. Clipboard is *not* a new
command: copy is session state (it mutates no document), cut is
`selection.delete`, and paste is the existing `image.import` plus a
`select.rect` — so paste is replayable for free and the clipboard needs no
new vocabulary. Text uses the in-repo 5x7 bitmap font rather than a system
font, keeping `pixelcad-core` dependency-free and deterministic. Optional
command fields (`shape`, `tolerance`, `radius`) default to the Phase 1
behaviour when absent, which is what keeps `ship.pxc` and
`blueprint.pxcproj` byte-identical.

## Resources and Dependencies

- [x] Rust 1.95, workspace green at 191 tests, commit `6a772ac`
- [x] `crates/core/src/font.rs` — recovered and fixed this session (two real
      bugs: a `const` glyph table that defeated identity lookups, and an
      unbounded `scale` that made `rasterize` non-terminating). 21 tests.
- [x] No new dependency required for any P1 item **except** OS-clipboard
      interop — see Out of Scope
- [x] `pixelcad-core` must still depend only on `thiserror`

## Out of Scope (This Session)

Everything the user's matrix marks **P2, P3 or P4**, plus these P1 rows,
each deferred for a stated reason rather than silently dropped:

- **Anti-aliased pencil, brush hardness, textured/technical-marker brushes.**
  These require sub-pixel coverage, which contradicts the pixel-exact
  invariant the whole determinism guarantee rests on. This needs a product
  decision (an "AA layer type"?), not a quiet implementation. → `BACKLOG.md`
- **Flow / blended opacity.** Pixel writes *replace* rather than blend (this
  is what makes the eraser work). Opacity is therefore delivered as the
  written alpha, not as accumulating paint. True flow needs a blend mode in
  the write path. → `BACKLOG.md`
- **OS clipboard interop** (accepting screenshots from other apps). Requires
  a platform clipboard crate in `pixelcad-app`, i.e. a new dependency and a
  network fetch. The internal clipboard is fully implemented. → `BACKLOG.md`
- **Interactive scale/rotate handles on the marquee.** The *operations* ship
  as commands and are reachable from the UI numerically and by shortcut;
  drag-handles are a separate interaction-design job. → `BACKLOG.md`
- **Skew.** Needs resampling and therefore the same AA decision as above.
- **RGB/HSL colour entry** (HEX ships; the other two are a picker widget).
- **Persistent/editable shape and text objects** — the matrix marks these
  P2, and they are really Phase 2's vector model.

## Acceptance Criteria

- [x] **AC1** — `cargo build --workspace` succeeds with zero errors and zero
      warnings
- [x] **AC2** — `cargo test --workspace` passes with ≥ 260 tests, 0 failures
- [x] **AC3** — `cargo tree -p pixelcad-core` still shows only `thiserror`
- [x] **AC4** — both existing samples still render byte-identically:
      `ship.pxc` → `bde0086…232`, `blueprint.pxcproj` → `abdf81e…f1a`
- [x] **AC5** — a lasso selection selects a non-rectangular region: a pixel
      inside the polygon is in the selection and one inside the bounding box
      but outside the polygon is not
- [x] **AC6** — `selection.move` moves the selected *pixels*, leaves
      transparency behind, and carries the selection with them
- [x] **AC7** — flip is an involution (twice = identity) and rotate 4×90° is
      the identity, both asserted on exact pixels
- [x] **AC8** — cut → paste elsewhere reproduces the cut pixels exactly and
      leaves the source region empty
- [x] **AC9** — `fill.bucket` with tolerance spreads across near-but-unequal
      colours and stops at a colour outside tolerance
- [x] **AC10** — a round brush of size 5 is not the square one: it paints
      fewer pixels and omits the corners
- [x] **AC11** — ellipse, rounded rectangle, polygon, arrow and polyline each
      have exact-pixel tests, filled and outlined where applicable
- [x] **AC12** — `text.draw` stamps legible glyphs: rendering `"A"` matches
      the font table exactly, and alignment/scale behave as `font.rs` defines
- [x] **AC13** — `canvas.crop` and `canvas.resize` preserve content
      correctly across *all* layers, and resize is nearest-neighbour exact
- [x] **AC14** — `layer.duplicate` produces an independent copy and
      `layer.merge` composites down without changing the visible result
- [x] **AC15** — every new `Command` variant round-trips through the parser
      (the fail-closed `variant_index` test still compiles and passes)
- [x] **AC16** — optional fields are backward compatible: a `brush.stroke`
      with no `shape`, a `rect.draw` with no `radius` and a `fill.bucket`
      with no `tolerance` all parse and behave exactly as in Phase 1
- [x] **AC17** — headless GUI tests cover: select-all, delete, nudge, cut,
      copy, paste, duplicate layer, and the new tool shortcuts
- [x] **AC18** — a new sample `docs/samples/paint_parity.pxcproj` exercises
      every new command and renders deterministically
- [x] **AC19** — `README.md` documents the complete grammar including all new
      commands and the updated shortcut table

## Constraints

- Files that must not be modified: `Profiles/**`, `Plans/archive/**`,
  `PHASE1_RESULT.md`, `PHASE2_RESULT.md`
- `pixelcad-core` gains no dependency
- Determinism: integer arithmetic only; no wall-clock, no unseeded RNG
- Optional command fields must default to Phase 1 behaviour
- Clipboard content is session state and must **not** enter the command log;
  its *effect* (paste) must
- A failed command must leave the document untouched

## Task Breakdown

### Task 1 — Bootstrap
- **Files:** `PROGRESS.md`, `BACKLOG.md`, `ROADMAP.md`, `PLAN.md`
- **Done when:** Phase 1.5 row is `ACTIVE`; deferred P1 items are recorded in
  `BACKLOG.md` with their reasons; the recovered `font.rs` is committed.

### Task 2 — Mask-based `Selection`
- **Files:** `crates/core/src/{engine.rs,lib.rs}`, `crates/app/src/controller.rs`
- **What:** `SelectionRect` → `Selection { x, y, width, height, mask }`;
  `select.all`, `select.lasso` (even-odd scanline fill of a polygon).
- **Done when:** AC5 passes; all existing selection tests still pass.

### Task 3 — Selection operations
- **Files:** `crates/core/src/{command.rs,parser.rs,engine.rs}`
- **What:** `selection.delete`, `selection.move`, `selection.flip`,
  `selection.rotate`, `selection.scale`.
- **Done when:** AC6, AC7 pass.

### Task 4 — Canvas and layer operations
- **What:** `canvas.crop`, `canvas.resize`, `layer.duplicate`, `layer.merge`.
- **Done when:** AC13, AC14 pass.

### Task 5 — Drawing: brush shapes, tolerance, shapes, text
- **What:** `brush.stroke shape=`, `fill.bucket tolerance=`,
  `rect.draw radius=`, `ellipse.draw`, `polygon.draw`, `arrow.draw`,
  `polyline.draw`, `text.draw`.
- **Done when:** AC9, AC10, AC11, AC12, AC16 pass.

### Task 6 — GUI
- **Files:** `crates/app/**`
- **What:** new tools; internal clipboard (cut/copy/copy-composite/paste/
  duplicate); select-all, delete, nudge; rulers; grid toggle; zoom fit and
  actual-size; recent colours; opacity control; crop/resize/text entry.
- **Done when:** AC17 passes and the existing GUI tests still pass.

### Task 7 — Sample, docs, close-out
- **Done when:** AC18, AC19 pass and every criterion is PASS with evidence.

## Resumption Note

Fresh phase on a green tree (191 tests, commit `6a772ac`). Highest-risk task
is Task 2: the `Selection` reshape touches every write path. If it cannot be
made green, fall back to keeping a rectangle plus a separate optional mask
side-table rather than changing the type.
