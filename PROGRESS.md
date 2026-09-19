# PROGRESS — PixelCAD Phase 0: Bootstrap & Command Engine

**Last updated:** 2026-09-19T01:00:00+08:00
**Current task:** Task 4 — Headless CLI (pixelcad-cli)
**Completed tasks:** 1, 2, 3
**Current mode:** A

## Log

### 2026-09-19T00:00:00+08:00 — Session start
- Fresh session. No prior PROGRESS.md existed.
- Read PLAN.md (Status: LOCKED, Product: PixelCAD), ROADMAP.md, Profiles/Autopilot/profile.md in full.
- Repo already has an initial commit `183dbbe "init REPO"` tracking `.gitignore`, `PLAN.md`, `ROADMAP.md`.
- `.gitignore` currently ignores `/Profiles`, `/Skills`, `/Agents`, `/cache`, and has a malformed `.log` pattern (should be `*.log`). Working tree has an uncommitted addition of `/cache` to `.gitignore`.
- Known issue I2 (Profiles/Skills versioning contradiction) will be posed to the maintainer as a question; proceeding with Tasks 2-7 while awaiting an answer, per session instructions. Fallback if unanswered: leave `Profiles/`/`Skills/` untracked, mark acceptance criterion 8 PARTIAL.

### 2026-09-19T00:05:00+08:00 — Dependency gate resolved: slint on macOS ARM64
- Built a throwaway probe crate at `cache/slint_probe` (gitignored, deleted after probe) with `slint = "1.8"` as its only dependency, at this exact repository path (contains a space and an apostrophe).
- `cargo build` succeeded in 55.21s (first compile, cold cache); resulting binary ran and printed expected output.
- Conclusion: slint tolerates this path. No Mode B pivot needed for the GUI surface. Proceeding with Task 5 design as planned in PLAN.md.
- Probe crate deleted; no trace left in the tree.

### 2026-09-19T00:15:00+08:00 — Task 1 complete
- Implemented: `.gitignore` fixed per I1 (`*.log`, `/target`, `out.png` added; `/Profiles`, `/Skills`, `/Agents`, `/cache` left as-is pending I2 answer); `LICENSE` (MIT); `README.md` (workspace overview + usage); `Cargo.toml` workspace with members `crates/core`, `crates/app`, `crates/cli`; each crate stubbed with a minimal `Cargo.toml` and placeholder `src/{lib,main}.rs`; `Plans/archive/.gitkeep` to version the empty archive dir.
- Tests: no tests yet at this stage; `cargo build` succeeded (0.40s) on the stub workspace — this is Task 1's own Done-when criterion.
- Mode used: A (sequential; no blockers).
- Deviations from PLAN.md: none. I2 (Profiles/Skills versioning contradiction) not yet resolved — see "Current Blockers" / question posed to maintainer in session output. Proceeding per instructions with Tasks 2-7 while awaiting an answer; fallback (leave untracked, AC8 = PARTIAL) will apply if none arrives before close-out.
- Files modified: `.gitignore`, `LICENSE`, `README.md`, `Cargo.toml`, `Cargo.lock`, `Plans/archive/.gitkeep`, `crates/core/Cargo.toml`, `crates/core/src/lib.rs`, `crates/cli/Cargo.toml`, `crates/cli/src/main.rs`, `crates/app/Cargo.toml`, `crates/app/src/main.rs`, `PROGRESS.md`.
- Committed: `84400ce` "Task 1: repository bootstrap".

### 2026-09-19T00:30:00+08:00 — Task 2 complete
- Implemented: `crates/core/src/document.rs` — `Document` (width/height + packed RGBA8 `Vec<u8>` buffer), `get_pixel`/`set_pixel` with bounds checking, `content_hash()` (hand-rolled FNV-1a, 64-bit, zero-dependency, deterministic across platforms/Rust versions), `DocumentError` via `thiserror`. Re-exported from `crates/core/src/lib.rs`.
- Tests: 6 passed / 0 failed (`cargo test -p pixelcad-core`) — construction/default-transparency, zero-size rejection, set/get round-trip incl. neighbour isolation, out-of-bounds errors, hash determinism, hash sensitivity to both pixels and dimensions.
- Mode used: A (sequential; no blockers). Fixed one `unused_mut` warning during implementation (not a failed attempt, not a Mode B trigger).
- Deviations from PLAN.md: PLAN.md suggested "blake3 or fnv content hash"; implemented a minimal hand-rolled FNV-1a instead of pulling in the `fnv` crate, to keep `core`'s dependency surface as small as possible. Same algorithm family, same determinism guarantee.
- Mode D regression check: `cargo build` (workspace) and `cargo test` (workspace) both pass after this change to shared infrastructure (`core` is depended on by `app` and `cli`).
- Files modified: `crates/core/Cargo.toml` (added `thiserror`), `crates/core/src/document.rs` (new), `crates/core/src/lib.rs`.
- Committed: `bcd8edf` "Task 2: document model (pixelcad-core)".

### 2026-09-19T01:00:00+08:00 — Task 3 complete
- Implemented: `crates/core/src/command.rs` (typed `Command` enum: `CanvasNew`, `PixelSet`, `LineDraw`, `PaletteSet`; hex color parse/format helpers); `crates/core/src/parser.rs` (tokenizer respecting quoted values, `parse_line`/`parse_script`/`serialize_command`/`serialize_script`, line-numbered `ParseError`, `#`-prefixed full-line comments); `crates/core/src/engine.rs` (`Engine` with snapshot-based undo/redo via a `states`/`commands`/`cursor` history model, fixed 16-slot palette with `DEFAULT_PALETTE`, Bresenham `line.draw`, `save_script()` producing `.pxc` text from active history only).
- Tests: 26 passed / 0 failed (`cargo test -p pixelcad-core`) — includes parser round-trip for all 4 command kinds and a full script, comment/blank-line skipping, line-numbered error reporting, engine undo/redo semantics (including "new action after undo clears redo"), Bresenham correctness, and the Task 3 determinism test: parsing an inline sample script and replaying it through two independent `Engine`s yields identical `document_hash()`.
- Mode used: A (sequential). One self-inflicted syntax snag: initial raw-string literal `r#"..."#` for the embedded sample script terminated early because the script body itself contains the two-character sequence `"#` (from `color="#1d1d1f"`); fixed by using `r##"..."##`. Caught immediately by the compiler, not a Mode B pivot.
- Deviations from PLAN.md: none in scope. Added `PaletteSet` state to the engine's snapshot (not just the document) so palette changes are also undoable/redoable and round-trip through `save_script`; PLAN.md's Command enum already listed `PaletteSet`, this just decides where its state lives.
- Mode D regression check: `cargo build` (workspace) and `cargo test --workspace` both pass. `cargo tree -p pixelcad-core` shows only `thiserror` (+ its proc-macro build deps) — confirms criterion 6 (no UI/GPU/slint dependency in `core`) holds after this change.
- Files modified: `crates/core/src/{command.rs,parser.rs,engine.rs}` (new), `crates/core/src/lib.rs` (module wiring + re-exports).
- Committed: (see next commit in this session).

## Current Blockers
[Empty — I2 is a non-blocking open question, see Log; Tasks 2-7 proceed regardless.]

## Backlog (out-of-scope items discovered during execution)
[none yet]

## Resumption
Task 1 — Repository bootstrap. Next step: resolve slint ARM64 dependency probe, then fix .gitignore per I1, then scaffold Cargo workspace. Last stable state: repo has only planning docs committed.
