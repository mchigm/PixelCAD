# PLAN — Phase 0: Bootstrap & Command Engine

**Created:** 2026-09-19T00:00:00+08:00
**Status:** COMPLETE
**Completed:** 2026-09-19T02:55:00+08:00
**Product:** PixelCAD (`pixelcad`)

## Objective

A buildable Cargo workspace in this repository containing a headless document
and command engine, a minimal Slint desktop shell, and a CLI — such that the
same command script produces byte-identical PNG output whether executed by the
CLI or the test harness, and pixels drawn in the GUI are recorded as a
replayable command script. This proves the platform doctrine (one command
engine, many surfaces) on day one, at the smallest possible scale.

## Implementation Approach

Rust workspace with three crates. `core` owns the document model (single RGBA
layer, CPU-authoritative) and a typed command enum with a text-command parser;
undo is implemented as document snapshots (simple and correct now; optimized
in a later phase). `app` is a thin Slint shell that only dispatches commands
and displays the document buffer. `cli` replays `.pxc` command scripts
headlessly and exports PNG. Alternatives rejected: C++/Qt (path hazard with
apostrophe in repo path, heavy day-one setup); egui (user chose Slint);
GPU-authoritative canvas (breaks determinism and headless execution).

## Resources and Dependencies

- [x] Rust toolchain 1.95 + Cargo: confirmed on this machine
- [x] Git 2.54: confirmed
- [x] crates.io reachable: confirmed (name `pixelcad` verified available)
- [ ] `slint` crate builds on macOS ARM64: expected supported — ⚠ UNCONFIRMED
      until first build (note: first compile takes several minutes)
- [ ] `png`/`image` crate: standard, assumed available via crates.io

## Out of Scope (This Session)

- Multiple layers in the UI, brush engine, eraser, fill, selections, transforms
- Project-file archive format (raw `.pxc` command script is the only save format)
- Linux build/validation, CI, release binaries, `cargo publish`
- CAD, AI, GPU rendering, ray tracing, tablet/pressure input
- Any performance optimization (tiling, dirty rectangles, GPU caching)

## Acceptance Criteria

Full evidence for each criterion is in `PHASE1_RESULT.md` ("Acceptance
Criteria Results" section). Summary:

- [x] `cargo build` succeeds from the current repository path (apostrophe/space
      path hazard check)
- [x] `cargo test` passes, including a determinism test: replaying
      `docs/samples/ship.pxc` twice yields identical document hashes
- [x] `cargo run -p pixelcad-cli -- run docs/samples/ship.pxc --out out.png`
      produces a PNG; running it twice produces byte-identical files
- [x] GUI launches; user can pan, zoom (nearest-neighbour with pixel grid at
      high zoom), draw with a 1-pixel pencil, and pick a color from a palette
- [x] GUI actions are recorded as commands; "Save script" writes a `.pxc` file
      that the CLI replays to reproduce the drawing exactly
- [x] `core` has no dependency on `slint` or any UI/GPU crate (verified via
      `cargo tree`)
- [x] Undo/redo works in the GUI for pencil strokes
- [ ] **PARTIAL** — Repository is a git repo with an initial commit (true);
      planning docs are versioned (true); `/target` and `*.log` are ignored
      (true); but `Profiles/` and `Skills/` are **not** versioned — I2
      (Profiles/Skills versioning contradiction, see PHASE1_RESULT.md) was
      never answered by the maintainer, so the documented fallback applied:
      leave them untracked/ignored, as `.gitignore` already did at session
      start.

## Constraints

- Files that must not be modified: `Profiles/Autopilot/**`,`Profiles/Planner/**` (agent profiles)
- `core` must remain headless and deterministic (no wall-clock, no RNG without
  seed, no float-order nondeterminism in compositing)
- Command schema is namespaced text (`canvas.new width=64 height=64`),
  parsed into typed Rust structs — no stringly-typed execution
- License: MIT

## Task Breakdown

### Task 1 — Repository bootstrap
- **Files affected:** `.gitignore`, `LICENSE`, `README.md`,
  `Cargo.toml` (workspace), `Plans/archive/` (empty dir)
- **What it does:** fixes `.gitignore` (docs;
  ignore `/target`, `*.log`, `out.png`), initializes git, creates the Cargo
  workspace with `crates/core`, `crates/app`, `crates/cli`, stub README.
- **Done when:** `git log` shows an initial commit and `cargo build` succeeds
  on the empty workspace.

### Task 2 — Document model (`pixelcad-core`)
- **Files affected:** `crates/core/src/{lib.rs,document.rs}`
- **What it does:** `Document` with width/height and one RGBA8 pixel buffer;
  get/set pixel; blake3 or fnv content hash for determinism tests.
- **Done when:** unit tests for pixel set/get and hashing pass.

### Task 3 — Command engine (`pixelcad-core`)
- **Files affected:** `crates/core/src/{command.rs,parser.rs,engine.rs}`
- **What it does:** typed `Command` enum — `CanvasNew`, `PixelSet`,
  `LineDraw` (Bresenham), `PaletteSet` — plus text parser
  (`pixel.set x=3 y=4 color="#1d1d1f"`), executor with snapshot-based
  undo/redo stack, and command-list serialization to/from `.pxc` text.
- **Done when:** parser round-trips all commands; determinism test (replay
  sample script twice → identical hash) passes.

### Task 4 — Headless CLI (`pixelcad-cli`)
- **Files affected:** `crates/cli/src/main.rs`, `docs/samples/ship.pxc`
- **What it does:** `pixelcad-cli run <script> --out <png>` replays a script
  through `core` and writes a PNG; sample script draws a simple ship hull
  outline (lines + pixels) as the dogfood artifact.
- **Done when:** CLI acceptance criterion above passes.
- **Pre-condition:** Tasks 2–3 complete.

### Task 5 — Slint shell (`pixelcad-app`)
- **Files affected:** `crates/app/src/main.rs`, `crates/app/ui/main.slint`
- **What it does:** window with canvas view (document buffer → Slint image,
  nearest-neighbour scaling), pan (drag/space), zoom (scroll), pixel grid at
  zoom ≥ 8×, pencil tool emitting `PixelSet`/`LineDraw` commands through the
  engine, fixed 16-swatch palette, undo/redo buttons, "Save script" action.
- **Done when:** GUI acceptance criteria above pass manually.
- **Pre-condition:** Tasks 2–3 complete. ⚠ First `slint` compile is slow.

### Task 6 — Session infrastructure (side quest)
- **Files affected:** `.agents/skills/next-phase/SKILL.md`, `ROADMAP.md`,
  `Skills/INDEX.md`
- **What it does:** project-local skill that instructs the agent to read
  `ROADMAP.md`, archive the completed `PLAN.md`, and invoke the Planner
  profile for the next `PENDING` phase; updates `Skills/INDEX.md`.
- **Done when:** skill file exists and `Skills/INDEX.md` references it.

### Task 7 — Close-out
- **Files affected:** `ROADMAP.md`, `PROGRESS.md`, `PLAN.md`
- **What it does:** runs the full acceptance checklist, commits, flips this
  plan's status to COMPLETE.
- **Done when:** every acceptance criterion is checked PASS with evidence.

## Resumption Note

Fresh phase — no prior state. If resuming mid-phase: read `PROGRESS.md` for
the last completed task; tasks are strictly ordered except Task 5 and Task 4,
which are independent of each other after Task 3.
