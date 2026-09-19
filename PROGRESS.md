# PROGRESS — Phase 1: Drawing MVP

**Last updated:** 2026-09-20T03:00:00+08:00
**Current task:** — (session complete)
**Completed tasks:** 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11
**Current mode:** A

> Phase 0's session log was archived to `Plans/archive/PROGRESS-phase0.md`
> at the start of this session. This file covers ROADMAP Phase 1 only.
> The self-contained result document is `PHASE2_RESULT.md`.

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
- Implemented: archived the Phase 0 log; wrote this `PROGRESS.md`; created
  `BACKLOG.md`; removed `/Profiles` and `/Skills` from `.gitignore` and
  committed those four markdown files verbatim (I2 resolved, reasoning
  written into `.gitignore` itself); `ROADMAP.md` Phase 1 → `ACTIVE`.
- Tests: n/a. `git ls-files Profiles Skills` lists all four files.
- Mode used: A.
- Deviations: none in content. Process note: `PLAN.md` was self-locked
  under the maintainer's blanket pre-approval rather than by an explicit
  per-plan sign-off. Recorded openly; see `PHASE2_RESULT.md` Section 6.
- Commit `ffad2d6`.

### 2026-09-20T00:30:00+08:00 — Tasks 2 and 3 complete (committed together)
- Implemented: `document.rs` rewritten around `Layer` and a layer stack,
  with `composite()` in integer math; 7 layer commands across
  `command.rs`/`parser.rs`/`engine.rs`; CLI and GUI switched from
  `Document::pixels()` (removed) to `Document::composite()`;
  `content_hash()` extended over all layer metadata.
- Three deliberate exactness properties keep Phase 0 output bit-identical:
  `scale_u8(v, 255) == v`, `over(src, transparent) == src`, and a fast path
  returning the buffer verbatim for a lone opaque visible layer. Each has
  its own test.
- Added the pinned regression test
  `ship_script_composites_to_the_phase_0_byte_sequence` (FNV-1a
  `9723217798654324057`) so this can never regress quietly.
- Tests: **76 passed, 0 failed**. AC4 re-verified by hand: still
  `bde0086…232`.
- Mode used: D. Deviation: Tasks 2 and 3 merged into one commit because
  Task 2 alone cannot compile the workspace by construction. Commit
  `47c83b8`.

### 2026-09-20T01:05:00+08:00 — Tasks 4 and 5 complete
- Implemented: `current_color` and `selection` in `EngineState` (both
  snapshotted, so undo restores tool state); commands `color.set`,
  `color.pick`, `select.rect`, `select.clear`, `brush.stroke`, `rect.draw`,
  `fill.bucket`; all pixel writes routed through `write_strict` (errors
  off-canvas, preserving Phase 0 behaviour) or `write_lenient` (clips).
- Flood fill is iterative with a `visited` bitmap and a fixed neighbour
  order; the selection is a wall, not just a write mask, so a fill cannot
  leak around the marquee.
- The eraser is deliberately not a separate command — writes replace rather
  than blend, so a transparent `brush.stroke` is the eraser.
- AC6 made fail-closed via an exhaustive `variant_index` match with no
  wildcard arm.
- Tests: **97 passed, 0 failed**. Mode used: A. Commit `2a0dfdd`.

### 2026-09-20T01:30:00+08:00 — Task 6 complete
- Implemented: history restructured to `Vec<Vec<Command>>` (one *step* per
  undo), plus `begin_group`/`end_group`/`group_open`/`step_count`.
  `history()` flattens, so grouping never leaks into the file format.
- Edge cases covered: empty and all-failed groups leave no phantom step;
  `begin_group` closes a dangling group; `end_group` is idempotent; `undo`
  mid-group closes it first; a new group after undo clears redo.
- Tests: **104 passed, 0 failed**. Mode used: D. Commit `6e309a3`.

### 2026-09-20T01:50:00+08:00 — Task 7 complete
- Implemented: `project.rs` with `PROJECT_FORMAT_VERSION = 1`, a
  `pixelcad.project version=N` header, `serialize_project`/`parse_project`/
  `open_project`, typed errors. Parsing completes before execution and
  `open_project` builds a fresh engine, so a bad file cannot half-apply.
  Legacy headerless `.pxc` loads as version 0. Save→load→save is a proven
  byte-identical fixed point. CLI opens both formats through one path.
- Deviation from `ROADMAP.md`'s "archive" wording: a versioned **text**
  container, not a zip. Rationale in the module header and in
  `PHASE2_RESULT.md` Section 6.
- Tests: **117 passed, 0 failed**. Mode used: A. Commit `bf9ece2`.

### 2026-09-20T02:05:00+08:00 — Task 8 complete
- Implemented: `base64.rs` (RFC 4648, no new dependency — re-verified with
  `cargo tree`); `image.import` writing raw RGBA8 onto the active layer,
  rejecting corrupt base64 and length/dimension mismatches; `pixelcad-cli
  import` normalising any PNG colour type to RGBA8.
- New `crates/cli/tests/import_round_trip.rs` drives the real binary for
  AC11, AC12, AC10-at-binary-level, and legacy-script compatibility.
- Tests: **132 passed, 0 failed**. Mode used: A. Commit `61b58e7`.

### 2026-09-20T02:40:00+08:00 — Task 9 complete
- Implemented: `Tool` enum and brush size in `Controller`; a third drag
  mode (`Anchored`) so line/rect/marquee commit only on release; layer
  operations; swatch editing; project open/save. Each pointer drag is
  wrapped in `begin_group`/`end_group`, satisfying AC13.
- `handle_shortcut()` holds the whole shortcut table in Rust so it is
  unit-testable and collapses Ctrl/Cmd into one `accel` flag.
- `open_project_file` swaps the engine only after the replacement is fully
  built, so a failed Open cannot damage open work.
- `.slint` gained a tool column, brush stepper, layer panel (displayed
  top-first, index converted in Rust), hex swatch editor and project
  controls. One fix needed: Slint only auto-converts percentages on size
  properties, not on `x`.
- Tests: **162 passed, 0 failed**, zero warnings. Mode used: A. Commit
  `d0583a2`.

### 2026-09-20T02:55:00+08:00 — Task 10 complete
- Implemented: `docs/samples/blueprint.pxcproj` — 128×96, four layers
  (Paper / Grid / Hull / Annotation), using 13 distinct commands including
  layer opacity and a selection-masked fill. Rendered sha256
  `abdf81edb8e8c52f7aba5c7977bf66bc5bc622eecac54456e099120b21d08f1a`,
  identical across two runs. Visually inspected once via an ASCII-luminance
  dump of the decoded PNG (no desktop screenshot, so no personal data was
  captured — Phase 0 had to delete its screenshot for that reason).
- 7 tests in `core/tests/blueprint_dogfood.rs` plus a CLI byte-identity
  test. One of them asserts the sample still *covers* each required
  command, so the artefact cannot silently stop dogfooding.
- `README.md` rewritten with the complete grammar, both file formats, the
  strict-vs-lenient out-of-bounds rule, and the control table.
- Tests: **170 passed, 0 failed**. Mode used: A. Commit `1b2af1c`.

### 2026-09-20T03:00:00+08:00 — Task 11 complete (close-out)
- Ran the full 19-criterion acceptance checklist; every one PASS with the
  exact command and observed output recorded in `PHASE2_RESULT.md`
  Section 2.
- Wrote `PHASE2_RESULT.md` (9 sections, mirroring `PHASE1_RESULT.md`).
- Archived `PLAN.md` → `Plans/archive/PLAN-phase1.md` with all criteria
  ticked and Status → COMPLETE.
- `ROADMAP.md`: Phase 1 → `COMPLETE` (Locked 2026-09-19, Completed
  2026-09-20). Phase 2 deliberately left `PENDING` — it becomes `ACTIVE`
  only when its own plan is drafted and locked, which is not this
  session's job.
- `Skills/INDEX.md` corrected (its heading said "Profiles") and annotated
  with the I2 decision. `Profiles/INDEX.md` left untouched: it still
  describes reality accurately, and `Profiles/**` is under a
  must-not-modify constraint.
- Verified the constraint held: `git log --follow -- Profiles/Planner/profile.md`
  shows exactly one commit, the one that added it. No profile bytes changed.

## Current Blockers

None.

## Backlog (out-of-scope items discovered during execution)

See `BACKLOG.md` (7 open items). Three further candidates surfaced during
Task 9 and are recorded in `PHASE2_RESULT.md` Section 8 for the next
Planner: no file dialogs; no canvas resize/crop/new-document; no GUI
affordance for the existing `layer.move` command.

## Resumption

Nothing to resume — Phase 1 is complete. A new session should start at
`ROADMAP.md`'s Session Protocol step 1 for Phase 2 ("Concept System"),
reading `PHASE2_RESULT.md` Section 8 first.
