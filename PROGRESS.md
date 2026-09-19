# PROGRESS — Phase 1: Drawing MVP

**Last updated:** 2026-09-20T00:40:00+08:00
**Current task:** Task 4 — Engine state: active colour and selection
**Completed tasks:** 1, 2, 3
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
- Mode used: A — no dependencies and no ambiguity, since `PLAN.md` settled
  both open questions before execution started.
- Deviations from PLAN.md: none in content. Process note: `PLAN.md` was
  self-locked under the maintainer's blanket pre-approval for this session
  ("complete it with no need of my approval") rather than by an explicit
  per-plan sign-off. This is recorded openly rather than presented as a
  normal lock.
- Files modified: `.gitignore`, `BACKLOG.md`, `PLAN.md`, `PROGRESS.md`,
  `Plans/archive/PROGRESS-phase0.md`, `ROADMAP.md`, `Profiles/**`,
  `Skills/INDEX.md`. Commit `ffad2d6`.

### 2026-09-20T00:30:00+08:00 — Tasks 2 and 3 complete (committed together)
- Implemented:
  - `crates/core/src/document.rs` — rewritten around `Layer { name, visible,
    opacity, pixels }` and `Document { width, height, layers, active }`.
    Added layer add/select/remove/rename/move/visible/opacity, `in_bounds`,
    `get_pixel_on`/`set_pixel_on`, `composited_pixel`, and `composite()`.
    `get_pixel`/`set_pixel` kept their exact Phase 0 signatures and now act
    on the active layer, which is why no Phase 0 call site needed touching.
  - Compositing is **integer-only** (`scale_u8`, `over`) with two exactness
    guarantees asserted by tests: `scale_u8(v, 255) == v`, and `over(src,
    transparent) == src`. Plus a fast path returning the buffer verbatim
    when exactly one visible layer sits at opacity 255. Together these are
    what keep Phase 0's PNG bit-identical.
  - `content_hash()` extended to cover layer count, order, names,
    visibility, opacity and the active index.
  - `crates/core/src/command.rs` — 7 new variants (`LayerAdd`,
    `LayerSelect`, `LayerRemove`, `LayerRename`, `LayerMove`,
    `LayerVisible`, `LayerOpacity`) plus `sanitize_name`.
  - `crates/core/src/parser.rs` — parse/serialize for all 7, plus
    `parse_usize` and `parse_bool` helpers.
  - `crates/core/src/engine.rs` — execution for all 7.
  - `crates/cli/src/main.rs` and `crates/app/src/render.rs` — switched from
    `Document::pixels()` (now gone) to `Document::composite()`.
  - `crates/core/tests/ship_determinism.rs` — new pinned regression test
    `ship_script_composites_to_the_phase_0_byte_sequence`, asserting the
    composited buffer's FNV-1a is exactly `9723217798654324057`.
- Tests: **76 passed, 0 failed** (Phase 0 baseline 51). Mode D full-suite
  regression run; AC4 re-verified manually as well — the CLI still emits
  sha256 `bde0086…232` for `docs/samples/ship.pxc`, unchanged.
- Mode used: D — `Document` is the most shared piece of infrastructure in
  the workspace; every consumer was re-run before advancing.
- Deviations from PLAN.md: one process deviation. `PLAN.md` lists Task 2 and
  Task 3 as separate tasks, but Task 2 alone deletes `Document::pixels()`
  and therefore *cannot* compile the workspace on its own. Committing a
  non-building tree would violate the "never leave a half-staged tree" rule
  more seriously than merging two task boundaries, so they were verified and
  committed as one atomic change. Both tasks' Done-when criteria were
  checked independently.
- Backlog additions from this task: none.

## Current Blockers

None.

## Backlog (out-of-scope items discovered during execution)

See `BACKLOG.md`.

## Resumption

Task 4 — Engine state: active colour and selection. Next step: add
`current_color` and `selection: Option<Rect>` to `EngineState`, with
`color.set`, `color.pick`, `select.rect`, `select.clear`, and route every
pixel write through one clipping helper. Last stable state: Tasks 1–3
complete, 76/76 tests green.
