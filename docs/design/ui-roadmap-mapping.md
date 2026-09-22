# UI → Roadmap Mapping

Every control the redesign renders as **WIP** is assigned here to exactly one
future destination, so a Planner session opening any ROADMAP phase can find
"which UI elements are waiting for this phase" without re-deriving it from
the sketch. No WIP item is left unassigned.

Destinations:

- **Phase N** — already covered by that ROADMAP phase's description.
- **BACKLOG (Phase N)** — new row in `BACKLOG.md`, tagged with the phase it
  most plausibly belongs to; the phase's own Planner may reschedule it.
- **ROADMAP cross-phase** — cross-cutting platform/windowing work that is
  not an editing capability; lives as a ROADMAP line item.
- **General** — no existing phase fits; kept in `BACKLOG.md` untagged.

| # | UI item (from `docs/design/ui-redesign-spec.md`) | Destination | Rationale |
|---|---|---|---|
| 1 | AI agent / AI chat / AI chat miniwindow / command completion | **Phase 4** | Phase 4 is "AI & Acceleration": proposals, preview/approval, scheduler. |
| 2 | In-GUI Command Line (CLI console, export/save, PATH via Linux commands, AutoCAD-style command) | **Phase 2** | Phase 2 already lists "command palette, textual command editor, macros". |
| 3 | Rotate angle (precise input) / Repeat transform | **Phase 2** | Extends the existing "exact dimensions / numeric move" BACKLOG rows. |
| 4 | Find (elements, actions, commands, edit history, conversation history) | **BACKLOG (Phase 2)** | Needs the command-history/index infrastructure Phase 2 builds. |
| 5 | Undo-to / redo-to a specific point (RMB on Undo/Redo) | **BACKLOG (Phase 2)** | History list UI + addressing into the undo stack; Phase 2's textual command model is the natural home. |
| 6 | Copy element under cursor with nothing selected | **BACKLOG (Phase 2)** | Requires hit-testing, which arrives with Phase 2's vector model. |
| 7 | Select "other options" expand (magic wand, select-by-colour) | **BACKLOG (Phase 2)** | Already tracked as Phase 1.5 rows; coverage mask exists, waiting on scheduling. |
| 8 | Stationaries "other types" (pen/fill/eraser/text variants) | **BACKLOG (Phase 2)** | Same family as the deferred brush-hardness/anti-aliasing decision. |
| 9 | Brush type / style / effect expand | **BACKLOG (Phase 2)** | Depends on the anti-aliasing / blend-mode product decision. |
| 10 | Fill/Outline 1D vs 1.7/2/2.5-D face-and-outline model | **Phase 3** | That dimensional model is CAD geometry, not raster painting. |
| 11 | Separate fill width vs outline width ("Width" + density) | **BACKLOG (Phase 3)** | Stroke styling of vector-ish shapes belongs with CAD entities. |
| 12 | Colour "effects" | **BACKLOG (Phase 2)** | Palette/colour infrastructure grows with the document palette (P2). |
| 13 | Canvas style (background / backdrop / texture / export style) | **BACKLOG (Phase 3)** | A document-wide visual style system; export style also touches Phase 5 rendering output. |
| 14 | Media bay (directory tree, resource finder, app storage, image browser) | **BACKLOG (Phase 2)** | A project-resource manager; Phase 2 introduces the project-level object model. |
| 15 | New style tab (adds tabs like Canvas style / Media bay) | **BACKLOG (Phase 3)** | Same as #13; tab infrastructure ships with the style system. |
| 16 | Multi-document file tabs / macOS file tree (real multiple documents) | **BACKLOG (General)** | No phase currently covers multi-document sessions; record and leave unscheduled. |
| 17 | Tail "Move" tool (drag/move/rotate by handle) | **BACKLOG (Phase 2)** | Interactive transform handles sit on the vector model; fixed commands exist today. |
| 18 | Canvas size "Format" (units px/mm/in) | **BACKLOG (Phase 2)** | "Document units" is already in the P2–P4 tier list. |
| 19 | App-icon dropdown menu (menu page) | **BACKLOG (Phase 2)** | An app-level menu needs the menu/command infrastructure Phase 2 creates. |
| 20 | Ribbon "Expand: other tabs" / RMB tab list | **ROADMAP cross-phase** | Part of user-customisable chrome, which is the Linux layout feature below. |
| 21 | Linux movable/dockable panels ("box" locations, GIMP/Krita-style) | **ROADMAP cross-phase** | Maintainer explicitly asked for this to live in `ROADMAP.md`; cross-cutting platform chrome work, not an editing capability. |
| 22 | macOS add/remove items (Layout tab) | **ROADMAP cross-phase** | Same item as #21: macOS can only add/reduce, per the spec. |
| 23 | Secondary tools miniwindow toolbox | **ROADMAP cross-phase** | Needs real multi-window support; same item as #21. |
| 24 | macOS fullscreen tab pop-out to miniwindow | **ROADMAP cross-phase** | Same multi-window infrastructure. |
| 25 | macOS Files menu: New / Export / Recent | **BACKLOG (General)** | New-document dialog and file dialogs are already tracked from Phase 1.5; Export/Recent are UI-only once those land. |
| 26 | macOS "Master management" | **BACKLOG (General)** | Meaning unconfirmed in the spec; placeholder preserved, destination undecided by design. |
| 27 | macOS native unified title bar (traffic-light-integrated custom chrome) | **BACKLOG (General)** | Cosmetic native-integration polish; rejected for Phase 1.75 because Slint 1.8 exposes no portable API for it. |
| 28 | Bespoke pixel-grid icon set (16px grid, matching the canvas) | **BACKLOG (General)** | Found during Task 1; Lucide is a stopgap. |
| 29 | Layer settings (lock / blend modes / groups / clipping) | **BACKLOG (Phase 2)** | Blend modes need the write-path change already noted in `BACKLOG.md`; groups/clipping belong with typed layers. |
| 30 | Autosave (actually saving on the timer) | **BACKLOG (General)** | The settings field ships; the timer is UI-side and needs the file-dialog work to be meaningful. |

Row numbers are stable references; if a row moves phase, update both the
destination and the rationale in one edit so this table stays the single
source of truth.
