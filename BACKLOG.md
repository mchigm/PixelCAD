# BACKLOG

Out-of-scope discoveries. Nothing here is executed in the phase that found
it; each item is carried forward for a future Planner to schedule or reject.

Format: `- [source phase] **Item** — description.`

## Open

- [Phase 1] **Undo-history memory is unbounded** — the engine keeps a full
  `Document` snapshot per undo step. Correct and simple, but a 1024×1024
  canvas with 200 steps costs ~800 MB. Needs tiles, diffs, or a bounded
  window that recomputes by replay. Explicitly deferred by `PLAN.md`
  (Phase 1, Out of Scope).
- [Phase 1] **`image.import` stores raw base64 RGBA** — a 512×512 import is
  ~1.4 MB of base64 inside the project file. Works and stays deterministic,
  but a compressed (e.g. PNG-in-base64) payload or an external blob
  directory would be far smaller. Deferred to keep `pixelcad-core`
  dependency-free.
- [Phase 1] **No transforms, copy/paste, or moving a selection's contents** —
  a rectangular selection currently only *clips* drawing. Moving pixels is a
  natural expectation and a natural Phase 2 item.
- [Phase 1] **No non-rectangular selection** (lasso, by-colour, magic wand).
- [Phase 1] **No layer blend modes beyond normal alpha-over**, no groups, no
  masks, no clipping layers.
- [Phase 1] **No automated visual-regression testing of the GUI** — carried
  forward unchanged from Phase 0's Known Limitations.
- [Phase 1] **No CI and no Linux/Windows validation** — blocked on a
  reference machine, per `ROADMAP.md`'s deferred cross-phase item.
