# PROGRESS — Phase 1.5: Paint Parity

**Last updated:** 2026-09-20T05:30:00+08:00
**Current task:** — (session complete)
**Completed tasks:** 1, 2, 3, 4, 5, 6, 7
**Current mode:** A

> Phase 1's log is in `Plans/archive/` alongside its plan; `PHASE2_RESULT.md`
> is the self-contained result document for ROADMAP Phase 1. This file
> covers ROADMAP Phase 1.5 only.

## Log

### 2026-09-20T04:00:00+08:00 — Pre-flight: recovering an orphaned module
An interrupted earlier session had left `crates/core/src/font.rs` (783
lines) uncommitted and broken: the suite reported 60 tests instead of 170,
one failure and one hang. Triage found two genuine bugs, **both caught by
the file's own tests**, so the module was fixed rather than discarded:

1. `GLYPHS_5X7` was a `const`, not a `static`. A `const` is substituted at
   every use site, so each `&GLYPHS_5X7[i]` borrowed a freshly materialised
   temporary: two lookups of the same character returned different
   addresses and the 665-byte table was duplicated into every caller.
2. `rasterize` clamped `scale` only at the lower bound, so `scale =
   u32::MAX` was not slow but non-terminating (~10^19 iterations). Added
   `MAX_SCALE = 64`, applied identically in `measure` and `rasterize` so
   the two can never disagree about what will appear on screen.

The one test asserting the old saturating contract was replaced with two
regression tests (clamp behaviour, glyph-lookup identity). 21 font tests
pass. Commit `6a772ac`.

### 2026-09-20T04:10:00+08:00 — Task 1 complete (bootstrap)
- `PLAN.md` written for Phase 1.5, scoped to the **P1 tier only** of the
  maintainer's Paint feature matrix, with every deferred P1 row given an
  explicit reason rather than being dropped silently.
- `ROADMAP.md` gained a Phase 1.5 entry (inserted before Phase 2, because
  this completes the Drawing MVP rather than starting the Concept System).
- Mode used: A.

### 2026-09-20T04:40:00+08:00 — Tasks 2-5 complete (core)
- **New module `selection.rs`.** `SelectionRect` became `Selection`: a
  bounding box plus an *optional* per-cell coverage mask. Lasso and
  rectangle are now one concept rather than two parallel ones. Masks
  normalise (an all-covered mask compares equal to the plain rectangle), so
  `PartialEq` means "selects the same cells" rather than "was built the
  same way".
- **A real semantic decision in polygon scan-conversion.** Pure even-odd
  filling follows the top-left rule and drops the bottom and right
  boundary — correct for *rendering* a polygon, wrong for *selecting* one,
  because a lasso traced around a square would fail to select the pixels
  the user traced over. `from_polygon` therefore fills the interior *and*
  stamps the outline with Bresenham, making the boundary inclusive. A test
  pins the square case at 16 cells, not 9.
- **16 new commands:** `select.all`, `select.lasso`,
  `selection.{delete,move,flip,rotate,scale}`, `canvas.{crop,resize}`,
  `layer.{duplicate,merge}`, `ellipse.draw`, `polygon.draw`, `arrow.draw`,
  `polyline.draw`, `text.draw`.
- **Three extended commands, all backward compatible:** `brush.stroke`
  gained `shape=`, `rect.draw` gained `radius=`, `fill.bucket` gained
  `tolerance=`. Each is optional and defaults to the Phase 1 behaviour,
  which is exactly why `ship.pxc` and `blueprint.pxcproj` still hash to
  `bde0086…232` and `abdf81e…f1a`.
- **Geometry is integer-only throughout**: a 91-entry fixed-point sine
  table for `polygon.draw`, an integer square root for `arrow.draw`, and a
  row-scanning midpoint ellipse. `f64::sin` cannot promise bit-identical
  results across targets; a table can.
- **`selection.move` lifts before it clears.** Clearing first would make a
  one-pixel nudge erase the pixels it had just written — a test
  (`ac6_an_overlapping_nudge_does_not_smear`) pins this.
- 39 behavioural tests added in `crates/core/tests/paint_parity.rs`, all
  passing on the first run. Commits `1da8e94`, plus the test commit.

### 2026-09-20T05:05:00+08:00 — Task 6 complete (GUI)
- 12 tools (adds ellipse, polyline, arrow, text, lasso); internal clipboard
  with cut / copy / copy-composite / paste / duplicate; select-all; delete;
  arrow-key nudge; flip / rotate / crop; brush-shape toggle; fill
  tolerance; stroke opacity; recent colours; grid toggle; zoom fit and 1:1;
  canvas resize and selection scale; layer duplicate / merge / raise /
  lower.
- **A real bug the new tests caught:** `image.import` is selection-clipped
  like every other write, so pasting while an old marquee was still active
  silently discarded everything outside it. `paste_at` now emits
  `select.rect` *before* `image.import`, which makes the clip a no-op and
  leaves the pasted region selected — which is what the user wants to drag
  next anyway.
- Clipboard content is deliberately session state and never enters the
  command log (copying mutates nothing). The *paste* is recorded, so a
  session still replays exactly.
- 12 new headless GUI tests. Commit `251c188`.
- Mode used: A. Two Slint-specific corrections were needed, both caught by
  the build: `opacity` is a reserved element property (renamed to
  `stroke-opacity`), and percentages auto-convert only on size properties.

### 2026-09-20T05:30:00+08:00 — Task 7 complete (sample, docs, close-out)
- `docs/samples/paint_parity.pxcproj`: a 160x120 four-layer tool sampler
  (Sheet / Shapes / Transformed / Labels) exercising every new command.
  Renders deterministically to sha256 `c056856314cd3b45…`. Visually checked
  once via an ASCII-luminance dump of the decoded PNG.
- 7 tests assert the sampler keeps using the commands it exists to
  demonstrate, uses the new *optional fields* rather than only their
  defaults, leaves no stale selection active, and puts ink on every layer.
- `README.md`: the full grammar including all 35 commands, the optional
  fields and their defaults, the expanded shortcut table, and the note on
  replace-vs-blend opacity.
- `BACKLOG.md` rewritten: the 8 deferred **P1** rows each with a reason,
  plus the P2–P4 tiers recorded so they are not rediscovered from scratch.

## Acceptance Criteria Results

| # | Result | Evidence |
|---|---|---|
| AC1 | **PASS** | `cargo build --workspace` → `Finished dev profile`, 0 errors, **0 warnings** |
| AC2 | **PASS** | `cargo test --workspace` → **274 passed, 0 failed** (target was ≥ 260; Phase 1 baseline 191) |
| AC3 | **PASS** | `cargo tree -p pixelcad-core` → only `thiserror` + its proc-macro chain. Both new core modules (`selection.rs`, and the recovered `font.rs`) are dependency-free by design |
| AC4 | **PASS** | `ship.pxc` → `bde008684f838437…`, `blueprint.pxcproj` → `abdf81edb8e8c52f…` — both unchanged from Phase 1 |
| AC5 | **PASS** | `ac5_a_lasso_selects_a_non_rectangular_region`, `ac5_drawing_through_a_lasso_is_clipped_to_its_shape` |
| AC6 | **PASS** | `ac6_selection_move_carries_the_pixels_and_the_marquee`, `ac6_an_overlapping_nudge_does_not_smear`, `moving_a_lasso_selection_moves_only_its_masked_pixels` |
| AC7 | **PASS** | `ac7_flipping_twice_is_the_identity`, `ac7_four_quarter_turns_are_the_identity`, `rotate_90_transposes_a_non_square_selection` |
| AC8 | **PASS** | `cut_copy_and_paste_round_trip_through_the_toolbar` — cut then paste elsewhere reproduces the pixels and empties the source |
| AC9 | **PASS** | `ac9_fill_tolerance_spreads_across_near_colours_and_stops_outside_it` |
| AC10 | **PASS** | `ac10_a_round_brush_is_not_a_square_one` — 25 px square vs fewer for the disc, corners empty |
| AC11 | **PASS** | `ac11_*` for ellipse (hollow vs filled, plus symmetry), rounded rectangle, polygon, arrow, polyline |
| AC12 | **PASS** | `ac12_text_stamps_glyphs_that_match_the_font_table` compares against `font::rasterize` directly |
| AC13 | **PASS** | `ac13_crop_and_resize_run_through_the_engine_and_clear_the_selection`, plus 8 `document.rs` tests |
| AC14 | **PASS** | `ac14_layer_duplicate_and_merge_through_the_engine` — merging is asserted to be *visually invisible* |
| AC15 | **PASS** | `every_command_variant_is_covered_by_the_round_trip_test` still fail-closed at `VARIANT_COUNT = 35` |
| AC16 | **PASS** | `ac16_*` tests for absent `shape`, `radius` and `tolerance`; AC4 is the end-to-end proof |
| AC17 | **PASS** | 12 new headless GUI tests (select-all, delete, nudge, cut/copy/paste, copy-composite, duplicate/merge, flip/rotate/crop, grid and zoom presets, all 12 tool shortcuts, recent colours, text, lasso) |
| AC18 | **PASS** | `docs/samples/paint_parity.pxcproj`, 7 tests, byte-identical across runs |
| AC19 | **PASS** | `README.md` documents all 35 commands, optional-field defaults, and the full shortcut table |

## Current Blockers

None.

## Backlog

See `BACKLOG.md`. Eight **P1** rows are deferred with explicit reasons; the
three most likely to be missed are anti-aliasing (blocked on a product
decision, not effort), OS-clipboard interop (needs a new dependency), and
Shift-constrained drawing (needs modifier state threaded from Slint).

## Resumption

Nothing to resume — Phase 1.5 is complete. A new session should start at
`ROADMAP.md`'s Session Protocol step 1 for Phase 2 ("Concept System"),
reading `PHASE2_RESULT.md` Section 8 and then `BACKLOG.md`.
