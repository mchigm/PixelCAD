# BACKLOG

Out-of-scope discoveries. Nothing here is executed in the phase that found
it; each item is carried forward for a future Planner to schedule or reject.

Format: `- [source phase] **Item** — description.`

## Deferred P1 items from the Paint feature matrix

These are the only rows the maintainer marked **P1** that Phase 1.5 did
**not** ship. Each is deferred for a stated reason, not dropped.

- [Phase 1.5] **Anti-aliased pencil, brush hardness, textured and
  technical-marker brushes** — all four need sub-pixel coverage, which
  directly contradicts the pixel-exact invariant that the byte-level
  determinism guarantee rests on. This needs a product decision first
  (an explicit "anti-aliased layer type"? a separate render mode?), not a
  quiet implementation. **Blocked on a decision, not on effort.**
- [Phase 1.5] **Flow / accumulating opacity** — pixel writes replace rather
  than blend, which is exactly what makes the eraser erase instead of
  painting white. Opacity therefore ships as the alpha written, so painting
  twice at 50 % gives 50 %, not 75 %. True flow needs a blend mode in the
  write path; that is a real change to `write_lenient`/`write_strict` and
  should be designed alongside layer blend modes.
- [Phase 1.5] **OS clipboard interop** (accepting screenshots and images
  from other applications) — needs a platform clipboard crate in
  `pixelcad-app`, i.e. a new dependency and a network fetch. The *internal*
  clipboard (cut/copy/copy-composite/paste/duplicate) is fully implemented.
- [Phase 1.5] **Interactive scale and rotate handles on the marquee** — the
  operations all ship as commands and are reachable from the toolbar,
  numerically and by shortcut. Drag-handles with live preview are a
  separate interaction-design job.
- [Phase 1.5] **Skew** — needs resampling, so it inherits the same
  anti-aliasing decision as the first item.
- [Phase 1.5] **RGB and HSL colour entry** — HEX entry ships; the other two
  are a colour-picker widget rather than a model change.
- [Phase 1.5] **Rulers** — the grid, zoom presets and a live zoom readout
  ship; graduated rulers along the viewport edges did not. Small, purely
  presentational, and worth doing with the Navigator (P2).
- [Phase 1.5] **Shift-constrained drawing** (0/45/90° lines, square/circle
  from a rectangle drag) — needs the modifier state threaded from Slint
  into the drag handler, which is a small but real plumbing change.

## Open (general)

- [Phase 1] **Undo-history memory is unbounded** — the engine keeps a full
  `Document` snapshot per undo step. Correct and simple, but a 1024×1024
  canvas with 200 steps costs ~800 MB. Needs tiles, diffs, or a bounded
  window that recomputes by replay. **Phase 1.5 made this worse in one
  respect:** `canvas.resize` and `selection.scale` can now change the
  document's size mid-history, so any future tiling scheme must cope with
  snapshots of differing dimensions.
- [Phase 1] **`image.import` stores raw base64 RGBA** — a 512×512 import is
  ~1.4 MB of base64 inside the project file. Now more visible, because
  every *paste* is an `image.import`, so a session with many pastes grows
  quickly. A compressed payload or an external blob directory would fix it.
- [Phase 1.5] **No non-rectangular selection beyond the lasso** — magic
  wand and select-by-colour are marked P1 in spirit but were listed under
  "Pixel mode" in the P2 selection-modes table; they are cheap now that a
  coverage mask exists (`Selection::from_mask`).
- [Phase 1] **No layer blend modes beyond normal alpha-over**, no groups,
  no masks, no clipping layers.
- [Phase 1] **No automated visual-regression testing of the GUI** — the
  headless GUI tests prove the logic is correct, but nothing compares
  rendered images.
- [Phase 1] **No CI and no Linux/Windows validation** — blocked on a
  reference machine, per `ROADMAP.md`'s deferred cross-phase item.
- [Phase 1] **No file dialogs** — Open and Save take a path typed into a
  text field.
- [Phase 1.5] **No new-document dialog** — the GUI always starts at 64×64.
  `canvas.resize` and `canvas.crop` now exist, so this is only UI.
- [Phase 1.5] **`text.draw` rasterises immediately** — text is not a
  re-editable object. The matrix marks persistent shapes and text as P2,
  which is really Phase 2's vector model.

## P2–P4 tiers (not yet scheduled)

The maintainer's feature matrix marks these for later phases and they are
recorded here only so they are not rediscovered from scratch: pressure and
tilt input, object eraser, closed-region fill, material picker, document
palette, persistent shapes, labels and dimensions, drawing sheets and
viewports, non-destructive transforms, snapping, document units, navigator,
vector lasso, scope-aware select-all, fine/coarse nudge, numeric move,
exact dimensions, repeat transform, multi-representation clipboard,
selection *modes* (object / node / edge-face / layer / AI), layer locking,
groups, typed layers, and the AI features (object select, generative fill
and erase, background removal, Image Creator).
