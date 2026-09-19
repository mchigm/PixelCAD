# PROGRESS — Phase 1: Drawing MVP

**Last updated:** 2026-09-20T02:10:00+08:00
**Current task:** Task 9 — GUI: tools, layers, palette editing, shortcuts
**Completed tasks:** 1, 2, 3, 4, 5, 6, 7, 8
**Current mode:** A

> Phase 0's session log was archived to `Plans/archive/PROGRESS-phase0.md`
> at the start of this session. This file covers ROADMAP Phase 1 only.

## Log

### 2026-09-19T23:20:00+08:00 — Phase 0 re-verification (pre-plan)
Before planning, every Phase 0 acceptance criterion was independently
re-checked in the repository as it exists now (not trusted from
`PHASE1_RESULT.md`):
- `cargo build --workspace` → `Finished dev profile ... in 0.41s`, 0 errors
- `cargo test --workspace` → 23 + 1 + 26 + 1 = **51 passed, 0 failed**
- `cargo tree -p pixelcad-core` → only `thiserror` → `thiserror-impl` →
  `{proc-macro2, quote, syn, unicode-ident}`; no UI/GPU crate
- `cargo run -p pixelcad-cli -- run docs/samples/ship.pxc` twice →
  both sha256 `bde008684f8384379e33f514d15d4ca4c1aed97798d52a08bbb1cccd6f781232`,
  matching the value recorded in `PHASE1_RESULT.md` exactly
- Criterion 8 confirmed still PARTIAL at session start: `Profiles/` and
  `Skills/` were still gitignored. Resolved this session by Task 1.

### 2026-09-19T23:55:00+08:00 — Task 1 complete
- Implemented: archived the Phase 0 log to
  `Plans/archive/PROGRESS-phase0.md`; wrote this Phase 1 `PROGRESS.md`;
  created `BACKLOG.md` seeded with everything `PLAN.md` put out of scope;
  removed `/Profiles` and `/Skills` from `.gitignore` and committed those
  four markdown files verbatim (I2 resolved, reasoning written into
  `.gitignore` itself and into `PLAN.md`); `ROADMAP.md` Phase 1 → `ACTIVE`.
- Tests: n/a (no code changed). `git ls-files Profiles Skills` lists all
  four files; working tree clean after commit.
- Mode used: A.
- Deviations from PLAN.md: none in content. Process note: `PLAN.md` was
  self-locked under the maintainer's blanket pre-approval for this session
  ("complete it with no need of my approval") rather than by an explicit
  per-plan sign-off. Recorded openly rather than presented as a normal lock.
- Files modified: `.gitignore`, `BACKLOG.md`, `PLAN.md`, `PROGRESS.md`,
  `Plans/archive/PROGRESS-phase0.md`, `ROADMAP.md`, `Profiles/**`,
  `Skills/INDEX.md`. Commit `ffad2d6`.

### 2026-09-20T00:30:00+08:00 — Tasks 2 and 3 complete (committed together)
- Implemented:
  - `crates/core/src/document.rs` rewritten around `Layer { name, visible,
    opacity, pixels }` and `Document { width, height, layers, active }`,
    with layer add/select/remove/rename/move/visible/opacity, `in_bounds`,
    `get_pixel_on`/`set_pixel_on`, `composited_pixel`, and `composite()`.
    `get_pixel`/`set_pixel` kept their Phase 0 signatures and now act on the
    active layer, which is why no Phase 0 call site needed touching.
  - Compositing is **integer-only** (`scale_u8`, `over`) with two exactness
    guarantees asserted by tests — `scale_u8(v, 255) == v` and `over(src,
    transparent) == src` — plus a fast path returning the buffer verbatim
    when exactly one visible layer sits at opacity 255. Together these keep
    Phase 0's PNG bit-identical.
  - `content_hash()` extended to cover layer count, order, names,
    visibility, opacity and the active index.
  - 7 new layer commands across `command.rs`, `parser.rs`, `engine.rs`.
  - `crates/cli/src/main.rs` and `crates/app/src/render.rs` switched from
    `Document::pixels()` (now gone) to `Document::composite()`.
  - New pinned regression test
    `ship_script_composites_to_the_phase_0_byte_sequence`, asserting the
    composited buffer's FNV-1a is exactly `9723217798654324057`.
- Tests: **76 passed, 0 failed** (baseline 51). Mode D full-suite
  regression; AC4 also re-verified by hand — the CLI still emits sha256
  `bde0086…232` for `docs/samples/ship.pxc`.
- Mode used: D — `Document` is the most shared piece of infrastructure here.
- Deviations from PLAN.md: one process deviation. Task 2 alone deletes
  `Document::pixels()` and therefore cannot compile the workspace on its
  own; committing a non-building tree would be a worse violation than
  merging two task boundaries, so both were verified independently and
  committed as one atomic change. Commit `47c83b8`.

### 2026-09-20T01:05:00+08:00 — Tasks 4 and 5 complete
- Implemented: `EngineState` gained `current_color` and
  `selection: Option<SelectionRect>`, both snapshotted so undo restores tool
  state (asserted by test). New commands `color.set`, `color.pick`
  (eyedropper, reads the *composite* not the active layer), `select.rect`,
  `select.clear`, `brush.stroke`, `rect.draw`, `fill.bucket`.
- Every pixel write now goes through one of two helpers: `write_strict`
  (precise commands — still errors off-canvas, preserving Phase 0
  behaviour) or `write_lenient` (area commands — clips silently, because a
  size-5 brush at the border must not abort the whole stroke). Both honour
  the selection.
- Flood fill is iterative (explicit LIFO stack, `visited` bitmap, fixed
  neighbour order) so it is deterministic, cannot overflow the stack, and
  terminates even when filling with the colour already present. The
  selection is treated as a **wall**, not just a write mask, so a fill
  cannot leak around the marquee and reappear inside it.
- The eraser is deliberately not its own command: writes replace rather
  than blend, so `brush.stroke` with `color="#00000000"` *is* the eraser.
  Documented on the variant and asserted by test.
- AC6 made fail-closed: `parser::tests::variant_index` is an exhaustive
  match with no wildcard arm, so adding a `Command` variant breaks the
  build until it is given a round-trip sample.
- Tests: **97 passed, 0 failed**. Mode used: A. Commit `2a0dfdd`.

### 2026-09-20T01:30:00+08:00 — Task 6 complete
- Implemented: history restructured from `Vec<Command>` (one command per
  undo step) to `Vec<Vec<Command>>` (one *step* per undo step), plus
  `begin_group`/`end_group`/`group_open`/`step_count`. `history()` now
  returns a flattened `Vec<Command>`, so `save_script` output is unchanged
  in form — grouping never leaks into the file format (asserted by test).
- Edge cases covered: an empty group and an all-failed group leave no
  phantom undo step; `begin_group` closes a dangling group; `end_group` is
  idempotent; `undo` during an open group closes it first; a new group
  after undo clears the redo branch.
- Tests: **104 passed, 0 failed**. Mode used: D (shared infrastructure —
  every crate depends on `Engine`). Commit `6e309a3`.

### 2026-09-20T01:50:00+08:00 — Task 7 complete
- Implemented: `crates/core/src/project.rs` with `PROJECT_FORMAT_VERSION =
  1`, a `pixelcad.project version=N` header, `serialize_project`,
  `parse_project`, `open_project`, and typed `ProjectError`/`OpenError`.
  Parsing completes **before** any command executes and `open_project`
  builds a *fresh* engine, so a malformed or future-version file can never
  partially mutate an open document (asserted by test). A bare Phase 0
  `.pxc` script loads as legacy version 0. Save → load → save is a proven
  byte-identical fixed point. The CLI opens both formats through one path.
- Deviation from `ROADMAP.md`'s wording: the project file is a versioned
  **text container**, not a zip archive. Rationale written into the module
  header — dependency-free, no timestamps/compressor variance to threaten
  determinism, git-diffable, and it makes "open a project" literally
  "replay its commands", which is the project's first invariant.
- Tests: **117 passed, 0 failed**; manual check confirmed a version-99 file
  exits non-zero with a message naming both versions and writes no output.
  Mode used: A. Commit `bf9ece2`.

### 2026-09-20T02:05:00+08:00 — Task 8 complete
- Implemented: `crates/core/src/base64.rs`, a ~100-line RFC 4648 codec with
  no new dependency (verified: `cargo tree -p pixelcad-core` still shows
  only `thiserror`). New `image.import x y width height data="<b64>"`
  command writing raw RGBA8 onto the active layer, clipped by canvas and
  selection, rejecting both corrupt base64 and a payload whose length
  contradicts the declared dimensions. `pixelcad-cli import <png> --out
  <pxcproj>` decodes any PNG colour type (RGB/RGBA/grey/grey+alpha/indexed)
  to RGBA8 and emits a version-1 project.
- New integration file `crates/cli/tests/import_round_trip.rs` drives the
  real compiled binary for: import→run pixel-exact round trip on a 23×17
  image with transparent/semi/opaque pixels (AC11); the same project
  rendering byte-identically twice (AC12); a version-99 project refused
  with no output file (AC10 at the binary level); a Phase 0 script still
  running through the new loader.
- Tests: **132 passed, 0 failed**. Mode used: A. Commit `61b58e7`.

## Current Blockers

None.

## Backlog (out-of-scope items discovered during execution)

See `BACKLOG.md`. No new items beyond those seeded in Task 1.

## Resumption

Task 9 — GUI. Next step: extend `Controller` with a tool enum, brush size,
layer operations and palette editing; extend `ui/main.slint` with the tool
row, layer panel, hex entry and Open/Save project buttons; add keyboard
shortcuts through the existing `FocusScope`; wrap each pointer drag in
`begin_group`/`end_group`. Last stable state: Tasks 1–8 complete, 132/132
tests green at commit `61b58e7`.
