# PLAN — Phase 1: Drawing MVP

**Created:** 2026-09-19T23:30:00+08:00
**Status:** COMPLETE
**Locked:** 2026-09-19T23:45:00+08:00
**Completed:** 2026-09-20T03:00:00+08:00

> All 19 acceptance criteria PASS. Full evidence (exact command + observed
> output per criterion) is in `PHASE2_RESULT.md` Section 2.
**Product:** PixelCAD (`pixelcad`)
**ROADMAP phase:** Phase 1 — "Drawing MVP" (the second executed session)

## Objective

PixelCAD becomes a usable, focused pixel/blueprint editor with no AI and no
CAD: a multi-layer document; brush, eraser, flood-fill, eyedropper, line,
rectangle and rectangular-selection tools; editable palette; keyboard
shortcuts; grouped undo/redo; a versioned `.pxcproj` project file that can be
saved and reopened losslessly; and PNG import. The observable end-state is
that a multi-layer historical ship blueprint can be drawn end to end and
saved, and that the saved project replays — through the CLI, with no GUI —
to a byte-identical PNG on every run. Every one of those tools emits a typed,
serializable command; nothing is drawn by a code path that bypasses the
command engine.

## Implementation Approach

Extend Phase 0's architecture; do not replace it. `Document` grows from one
RGBA8 buffer to an ordered `Vec<Layer>` plus an active-layer index, with a
deterministic integer-math `composite()` that every export and render path
consumes — this is the one breaking change, taken first, so every later task
builds on the final shape. New tools are new `Command` variants parsed by the
existing `.pxc` grammar, so the CLI gains all of them for free. Undo "polish"
is implemented as *step grouping* in the engine (a pointer drag becomes one
undoable step) rather than as a memory optimisation; snapshot-per-step is
retained, and history-memory optimisation goes to `BACKLOG.md`. The project
file is deliberately **not** a zip archive: it is a versioned, self-contained
text container (`pixelcad.project version=1` header + the command log), which
keeps `pixelcad-core` dependency-free, keeps the format diffable in git,
makes "open a project" literally "replay its commands" (honouring the
replayability invariant), and makes byte-level determinism trivial to
guarantee. PNG import is likewise expressed as a command carrying
base64-encoded RGBA, using a ~40-line base64 codec written in `core` rather
than a new dependency. Alternatives rejected: a zip/`.pxc`-archive container
(adds a dependency, opaque to git, determinism depends on the zip writer's
timestamp/ordering behaviour); a separate binary layer-blob format (breaks
"the document is its command log"); storing selections or the active colour
only in the GUI (would make GUI actions unreplayable, violating the first
invariant).

## Resources and Dependencies

- [x] Rust/Cargo 1.95 on macOS ARM64: confirmed (`cargo build --workspace`
      re-verified green in this repository at this path before planning)
- [x] Phase 0 artefacts intact: confirmed — 51/51 tests pass,
      `cargo tree -p pixelcad-core` shows only `thiserror`, and
      `docs/samples/ship.pxc` still renders to sha256
      `bde008684f8384379e33f514d15d4ca4c1aed97798d52a08bbb1cccd6f781232`
- [x] `png` crate (already a `pixelcad-cli` dependency): confirmed in
      `Cargo.lock`; its **decoder** is needed for PNG import — same crate,
      no new dependency
- [x] `slint` + `i-slint-backend-testing`: confirmed working (23 headless GUI
      tests already pass)
- [x] Base64: **no dependency** — implemented in `pixelcad-core` (keeps the
      "core depends on nothing but `thiserror`" property)
- [x] **I2 resolved** (see Decisions): `Profiles/` and `Skills/` are to be
      committed to git
- [x] **First-task priority resolved** (see Decisions): multiple layers
      leads; the project-file format follows it

## Decisions Taken On The Two Open Questions

Both questions were escalated to the maintainer per the session brief. The
maintainer's instruction for this session was explicit blanket pre-approval
("complete it with no need of my approval; default to autoapprove"), which is
a delegation of these two decisions, not a deferral of them. They are
therefore **settled here, once, explicitly** — not re-deferred as Phase 0 had
to do. Both are recorded as decisions with reasoning so they can be reversed
knowingly rather than by accident.

**I2 — `Profiles/` and `Skills/` are committed to git.** Reasoning:
`ROADMAP.md`'s Session Protocol instructs every future session to read
`Profiles/Planner/profile.md` and `Profiles/Autopilot/profile.md` by path. If
those files are not in the repository, the "Reproduce From Clean Clone"
section of every result document is false — a fresh clone cannot run the
documented process. The content is 24 KB of plain markdown with no secrets,
no credentials, and no personal data. `Skills/INDEX.md` likewise indexes
`.agents/skills/`, which *is* already tracked, so leaving the index untracked
is incoherent. `/Agents` and `/cache` stay ignored (empty/scratch).
Committing these files is **not** a modification of them: the constraint
"never modify `Profiles/**`" remains in force and their bytes are unchanged.

**First-task priority — layers lead, project file follows.** Reasoning: the
project file must serialise layers. Writing the container first guarantees a
format-version bump inside the same phase, which is strictly worse than doing
the reshape first. Layers also touch `Document`, `Engine`, `render.rs`, the
CLI exporter and every existing test, so doing it while the codebase is
smallest costs the least. The order is therefore: layers (Task 2–3) →
engine state and tools (Task 4–6) → project file (Task 7) → PNG import
(Task 8) → GUI (Task 9) → dogfood (Task 10).

## Out of Scope (This Session)

- Any AI, CAD, GPU rendering, vector shapes, guides, snapping, macros, or
  command palette (these are Phase 2 and Phase 3)
- Non-rectangular selections (lasso, magic wand, by-colour), feathering,
  anti-aliasing of any kind — PixelCAD is pixel-exact by design
- Layer blend modes beyond normal alpha-over; layer groups/folders; masks
- Transforms (move/scale/rotate of a selection's contents), copy/paste
- Undo-history memory optimisation (tiles, diffs, bounded history) — the
  snapshot-per-step model is retained deliberately
- Text tool, gradients, dithering, brush shapes other than square
- PNG *export* changes beyond compositing layers (no palette/indexed PNG,
  no metadata, no other image formats)
- Linux/Windows validation, CI, release binaries, `cargo publish`
- Any performance optimisation, profiling, or large-canvas work

## Acceptance Criteria

- [x] **AC1** — `cargo build --workspace` succeeds from this repository path
      (space + apostrophe) with zero errors
- [x] **AC2** — `cargo test --workspace` passes with zero failures and at
      least 90 tests total (Phase 0 baseline was 51)
- [x] **AC3** — `cargo tree -p pixelcad-core` lists no `slint`, UI, GPU, AI
      or CAD crate (only `thiserror` and its proc-macro chain)
- [x] **AC4 (Phase 0 regression guard)** — `cargo run -p pixelcad-cli -- run
      docs/samples/ship.pxc --out out.png` still produces sha256
      `bde008684f8384379e33f514d15d4ca4c1aed97798d52a08bbb1cccd6f781232`,
      byte-identical to Phase 0, proving the layer reshape did not change
      single-layer output
- [x] **AC5** — a script that creates a second layer and paints over the
      first composites top-over-bottom correctly, and toggling the top
      layer's visibility or opacity changes the composited output — asserted
      by unit test on exact pixel values
- [x] **AC6** — every `Command` variant round-trips
      `Command -> serialize_command -> parse_line -> Command` unchanged,
      asserted by a test that enumerates **all** variants (so a future
      variant added without parser support fails the test)
- [x] **AC7** — brush (size ≥ 1), eraser, flood fill, eyedropper, line,
      rectangle (outlined and filled), and rectangular selection each have a
      core-level unit test asserting exact resulting pixels, and each is
      reachable only via a `Command`
- [x] **AC8** — a rectangular selection clips every drawing command: a fill
      or brush stroke that would cross the selection boundary writes no pixel
      outside it (unit test on exact pixels)
- [x] **AC9** — a project saved with `save_project` and reloaded with
      `open_project` yields an identical `Document::content_hash()` and an
      identical command history
- [x] **AC10** — a `.pxcproj` whose header declares a version newer than the
      supported one is rejected with a typed error naming both versions, and
      the in-memory document is left untouched (no partial load)
- [x] **AC11** — `pixelcad-cli import <in.png> --out <p.pxcproj>` followed by
      `pixelcad-cli run <p.pxcproj> --out <out.png>` produces a PNG whose
      decoded RGBA pixels are identical to the source PNG's
- [x] **AC12** — running the same `.pxcproj` twice through the CLI produces
      byte-identical PNGs (determinism preserved for the new format)
- [x] **AC13** — a headless GUI test proves one pointer drag (down, several
      moves, up) is undone by a **single** Undo click, restoring the document
      exactly
- [x] **AC14** — headless GUI tests dispatch real key events and prove at
      least: `b`/`e`/`g`/`i`/`l`/`r`/`m` select the brush/eraser/fill/
      eyedropper/line/rect/select tools, and Ctrl-or-Cmd+`z` undoes
- [x] **AC15** — a headless GUI test proves editing a palette swatch's colour
      emits a `palette.set` command that appears in the saved script
- [x] **AC16** — a headless GUI test proves adding a layer, selecting it,
      drawing on it, and toggling its visibility all work through the real
      wired callbacks
- [x] **AC17** — `docs/samples/blueprint.pxcproj` exists, is a valid
      version-1 project with at least 3 layers, renders through the CLI, and
      renders byte-identically on two consecutive runs
- [x] **AC18** — `git ls-files` lists `Profiles/Planner/profile.md`,
      `Profiles/Autopilot/profile.md`, `Profiles/INDEX.md` and
      `Skills/INDEX.md` (I2 resolved), while `/target`, `*.log`, `/cache` and
      `/Agents` remain ignored
- [x] **AC19** — `README.md` documents the full `.pxc` command grammar as
      implemented at the end of this phase, and `BACKLOG.md` exists and
      contains every out-of-scope discovery made during execution

## Constraints

- **Files that must not be modified:** `Profiles/Autopilot/**`,
  `Profiles/Planner/**` (agent profiles — they may be *added to git*, but
  their bytes must not change), `Plans/archive/**` (completed plans),
  `PHASE1_RESULT.md` (the previous session's immutable record)
- `pixelcad-core` must not gain any dependency other than `thiserror`
- `pixelcad-core` must remain deterministic: no wall-clock, no unseeded RNG,
  no float-order-dependent compositing (compositing must use integer math so
  results are bit-exact on every platform)
- Every user-visible action must become a serializable, replayable `Command`
  — including tool switches that affect document state (active colour,
  selection, active layer). Purely visual state (pan, zoom, which swatch is
  highlighted) is exempt and must **not** enter the command log
- A malformed or future-version project file must never partially mutate an
  open document
- Backwards compatibility: a bare Phase 0 `.pxc` script (no version header)
  must still load and replay unchanged
- License: MIT; no new runtime dependency in any crate

## Task Breakdown

### Task 1 — Session bootstrap and governance (I2)
- **Files affected:** `PROGRESS.md` → `Plans/archive/PROGRESS-phase0.md`,
  new `PROGRESS.md`, `BACKLOG.md`, `.gitignore`, `ROADMAP.md`
- **What it does:** archives the Phase 0 session log, creates a fresh
  Autopilot-schema `PROGRESS.md` and an empty-but-structured `BACKLOG.md`,
  removes `/Profiles` and `/Skills` from `.gitignore` and commits those
  directories verbatim, and flips `ROADMAP.md`'s Phase 1 row to `ACTIVE`
  with the Locked date.
- **Done when:** `git ls-files Profiles Skills` lists all four markdown
  files; `git --no-optional-locks status --short` is clean; `PROGRESS.md`
  exists with the Phase 1 header; `BACKLOG.md` exists.
- **Pre-condition:** none.

### Task 2 — Multi-layer `Document` (core, breaking)
- **Files affected:** `crates/core/src/document.rs`, `crates/core/src/lib.rs`
- **What it does:** introduces `Layer { name, visible, opacity, pixels }` and
  reshapes `Document` to `{ width, height, layers: Vec<Layer>, active: usize
  }` with at least one layer always present; adds layer accessors/mutators,
  `composite() -> Vec<u8>` (integer-math alpha-over, bottom to top, with an
  exact identity path for a single fully-opaque-opacity visible layer), and
  extends `content_hash()` to cover layer names, visibility, opacity and
  ordering. `get_pixel`/`set_pixel` keep their signatures and operate on the
  active layer so existing call sites compile unchanged.
- **Done when:** `cargo test -p pixelcad-core` passes with new unit tests for
  layer add/remove/reorder, `composite()` over/under ordering, opacity,
  visibility, and hash sensitivity to each layer field; AC5's exact-pixel
  assertions pass.
- **Pre-condition:** Task 1 complete.

### Task 3 — Layer commands + consumers (Mode D regression)
- **Files affected:** `crates/core/src/{command.rs,parser.rs,engine.rs}`,
  `crates/cli/src/main.rs`, `crates/app/src/render.rs`
- **What it does:** adds `layer.add`, `layer.select`, `layer.remove`,
  `layer.rename`, `layer.move`, `layer.visible`, `layer.opacity` as typed
  commands with parser support and engine execution; switches the CLI PNG
  exporter and the GUI renderer from `Document::pixels()` to
  `Document::composite()`.
- **Done when:** `cargo test --workspace` is fully green **and** AC4 holds —
  `docs/samples/ship.pxc` still hashes to `bde0086…`. This is shared
  infrastructure: a full-suite regression check is mandatory before
  advancing.
- **Pre-condition:** Task 2 complete.

### Task 4 — Engine state: active colour and selection
- **Files affected:** `crates/core/src/{command.rs,parser.rs,engine.rs}`
- **What it does:** adds `current_color` and `selection: Option<Rect>` to the
  engine state with commands `color.set`, `color.pick` (eyedropper — reads
  the composited pixel at `(x, y)` into `current_color`), `select.rect` and
  `select.clear`; routes every pixel write through one helper that clips to
  the selection and to the canvas bounds.
- **Done when:** AC8 passes (a stroke and a fill crossing the selection edge
  write nothing outside it) and `color.pick` on a painted pixel sets exactly
  that colour, both asserted by unit test.
- **Pre-condition:** Task 3 complete.

### Task 5 — Drawing commands: brush, eraser, fill, rectangle
- **Files affected:** `crates/core/src/{command.rs,parser.rs,engine.rs}`
- **What it does:** adds `brush.stroke x0 y0 x1 y1 size color` (square brush
  swept along a Bresenham path; the eraser is the same command with
  `color="#00000000"`, since writes replace rather than blend),
  `rect.draw x0 y0 x1 y1 color fill=<bool>`, and `fill.bucket x y color`
  (deterministic 4-connected flood fill over the *active layer*, matching the
  exact starting RGBA, with an explicit visited set and a fixed neighbour
  order so the result never depends on iteration order).
- **Done when:** AC7's per-tool exact-pixel unit tests pass, including: a
  size-3 brush covers a 3×3 block per path point; an eraser stroke restores
  `#00000000`; a filled rectangle differs from an outlined one by exactly its
  interior; a flood fill stops at a drawn border and never recurses
  infinitely on a same-colour fill (fill with the colour already present must
  terminate).
- **Pre-condition:** Task 4 complete.

### Task 6 — Undo/redo polish: step grouping
- **Files affected:** `crates/core/src/engine.rs`
- **What it does:** restructures history from "one command per undo step" to
  "one *step* per undo step, each holding one or more commands", adding
  `begin_group()`/`end_group()`; `history()` flattens steps so `save_script`
  output is unchanged in form, and a new-action-after-undo still clears redo.
- **Done when:** a unit test proves that 1 `begin_group` + N commands +
  `end_group` is undone by a single `undo()` call and redone by a single
  `redo()`, that `history()` still serialises all N commands, and that the
  whole existing engine test suite still passes unmodified in intent.
- **Pre-condition:** Task 5 complete. Shared infrastructure → Mode D.

### Task 7 — Versioned project file (`.pxcproj`)
- **Files affected:** `crates/core/src/project.rs` (new),
  `crates/core/src/lib.rs`, `crates/cli/src/main.rs`
- **What it does:** defines `PROJECT_FORMAT_VERSION = 1` and a container
  format whose first non-comment line is `pixelcad.project version=1`,
  followed by the command log; `serialize_project(&Engine) -> String` and
  `parse_project(&str) -> Result<(u32, Vec<Command>), ProjectError>`, where a
  missing header is accepted as legacy version 0 (a bare Phase 0 `.pxc`
  script) and a version greater than `PROJECT_FORMAT_VERSION` is a typed
  error naming both versions; the CLI's `run` subcommand accepts `.pxc` and
  `.pxcproj` through the same path.
- **Done when:** AC9, AC10 and AC12 pass; a Phase 0 `.pxc` script still loads
  (AC4 re-checked).
- **Pre-condition:** Task 6 complete.

### Task 8 — PNG import
- **Files affected:** `crates/core/src/{base64.rs (new),command.rs,parser.rs,
  engine.rs}`, `crates/cli/src/main.rs`, `crates/cli/tests/`
- **What it does:** adds a dependency-free base64 codec in `core`; adds
  `image.import x y width height data="<base64 rgba8>"` which writes the
  decoded block onto the active layer (clipped by bounds and selection); adds
  `pixelcad-cli import <in.png> --out <out.pxcproj>` which decodes a PNG with
  the existing `png` crate and emits a valid version-1 project.
- **Done when:** AC11 passes end to end as an integration test invoking the
  real compiled binary twice (import, then run) and comparing decoded pixel
  buffers; base64 has its own round-trip unit tests including all three
  padding cases.
- **Pre-condition:** Task 7 complete.

### Task 9 — GUI: tools, layers, palette editing, shortcuts, open/save
- **Files affected:** `crates/app/ui/main.slint`,
  `crates/app/src/{controller.rs,main.rs}`
- **What it does:** adds a tool selector (brush, eraser, fill, eyedropper,
  line, rectangle, select) with a brush-size control; a layer panel (add,
  select, visibility toggle, delete); palette editing (hex entry applied to
  the highlighted swatch, emitting `palette.set`); keyboard shortcuts
  (`b`/`e`/`g`/`i`/`l`/`r`/`m` for tools, `Ctrl`-or-`Cmd`+`z` undo,
  `+shift` redo, `[`/`]` brush size, `1`–`8` palette slots, `+`/`-` zoom,
  Space+drag pan as before); Open/Save project buttons using `.pxcproj`; and
  wraps each pointer drag in `begin_group`/`end_group`.
- **Done when:** AC13, AC14, AC15 and AC16 all pass as headless GUI tests
  through the real `wire_callbacks`, and the existing Phase 0 GUI tests still
  pass.
- **Pre-condition:** Task 8 complete.

### Task 10 — Dogfood: the ship blueprint
- **Files affected:** `docs/samples/blueprint.pxcproj` (new),
  `crates/core/tests/`, `crates/cli/tests/`, `README.md`
- **What it does:** authors a multi-layer historical ship blueprint project
  (at minimum: a grid/paper background layer, a hull-and-deck construction
  layer, a rigging layer, and an annotation layer) exercising brush, line,
  rectangle, fill, layer opacity and selection; adds determinism tests for
  it; documents the complete command grammar and the project format in
  `README.md`.
- **Done when:** AC17 and AC19 pass; the rendered blueprint PNG is visually
  inspected once and its sha256 recorded in the result document.
- **Pre-condition:** Task 9 complete.

### Task 11 — Close-out
- **Files affected:** `PLAN.md`, `PROGRESS.md`, `ROADMAP.md`,
  `PHASE2_RESULT.md` (new), `Plans/archive/PLAN-phase1.md`,
  `Skills/INDEX.md`, `Profiles/INDEX.md`
- **What it does:** runs the full acceptance checklist with evidence, writes
  `PHASE2_RESULT.md` in the 9-section schema of `PHASE1_RESULT.md`, archives
  this plan, flips `ROADMAP.md` Phase 1 to `COMPLETE`, and commits.
- **Done when:** every AC above is marked PASS or FAIL **with the exact
  command and observed output**, and `PHASE2_RESULT.md` is committed.
- **Pre-condition:** Task 10 complete.

## Resumption Note

Fresh phase, building on a verified Phase 0. If resuming mid-phase: read
`PROGRESS.md` for the last completed task and resume at the next one. Tasks
are strictly ordered — each builds on the previous one's data shape — with
one exception: Task 8 (PNG import) is independent of Task 9 (GUI) and the two
may be swapped if the GUI is blocked. The single highest-risk step is Task 2
(the `Document` reshape); if `cargo test --workspace` cannot be made green
after Task 3's Mode D check, revert to the last commit and re-approach the
reshape as an additive `Document::layers()` API with `pixels()` retained as
`composite()` rather than as a breaking field change.
