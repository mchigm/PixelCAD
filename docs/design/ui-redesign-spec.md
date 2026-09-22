# PixelCAD UI Redesign — Implementation Spec

Synthesizes `docs/design/pixelcad-ui-draft-1.md` (maintainer's sketch +
written supplement) into a build-ready spec. Every sketch element is
classified:

- **LIVE** — backed by an existing `pixelcad-core` command or existing GUI
  state today; the redesign only needs to re-skin/reposition it.
- **WIP** — sketched, but no backing implementation exists yet. Renders as a
  visibly-present, disabled/placeholder control labeled "WIP", per the
  maintainer's instruction to leave unimplemented features blank-with-warning
  rather than fake them or hide them.

This file is the single source of truth for what the redesigned UI should
contain in the MVP. It does not itself decide *how* windowing/native-chrome
work happens on macOS — see "Open engineering decisions" at the bottom.

---

## Linux window

| Region | Item | Status | Notes |
|---|---|---|---|
| Title | App icon → menu | WIP | No app-level menu system yet. Icon present, click shows "Menu — WIP". |
| Title | Save | LIVE | Maps to existing save/save-project. "Save As" on new file: WIP (no file dialog; path is a text field today). |
| Title | Redo | LIVE | RMB "redo to a specific point": WIP (history list UI doesn't exist). |
| Title | Undo | LIVE | RMB "undo to a specific point": WIP. |
| Title | Filename | LIVE | Existing path field, restyled as a label/breadcrumb. |
| Title | Settings | **NEW, LIVE** | Opens the new Settings page (this task builds it). |
| Secondary1 | Tabs: Home / Tool / View / Custom | PARTIAL | Tabs are real and switch panels. Only **Home** has content; Tool/View/Custom show a WIP panel. RMB-expand list: WIP. |
| Home tab | Paste | LIVE | Existing paste. "Paste options" expand: WIP. |
| Home tab | Cut | LIVE | Existing cut (internal clipboard only; OS-clipboard PNG interop is WIP per `BACKLOG.md`). |
| Home tab | Copy / Copy hovered element | PARTIAL | Copy-selection and copy-composite exist. "Copy element under hover with nothing selected" is WIP (no hit-testing model). |
| Home tab | Select (marquee mode) | LIVE | Existing rectangle/lasso select. "Other select options" expand: WIP. |
| Home tab | Default select (pos1/pos2 aim cursor) | LIVE | Existing rectangle select already works this way (drag defines the box); cursor changes to a crosshair: minor polish, LIVE. |
| Home tab | Crop & Resize | LIVE | Existing crop + canvas resize; expose both under one menu button. |
| Home tab | Stationaries: Pen/Fill/Eraser/Text | LIVE | Existing tools. "Other types" expand: WIP. |
| Home tab | Brush (type/style/effect/size) | PARTIAL | Size + shape toggle exist. Type/style/effect: WIP. |
| Home tab | Shapes (line/ellipse/rect/polygon/arrow/polyline) | LIVE | All exist; render as a scrollable/expandable icon grid instead of a flat button list. |
| Home tab | Fill/Outline | PARTIAL | Fill bucket + tolerance exist. The 1D vs 1.7D/2D/2.5D face/outline distinction is a **Phase 2/3 vector concept** — WIP, out of scope until the Concept System / CAD phases. |
| Home tab | Width | PARTIAL | Maps to existing brush size for MVP. Separate outline-vs-fill width: WIP. |
| Home tab | Color & Tools | LIVE | Existing palette + hex entry + recent colors. "Effects": WIP. |
| Home tab | AI | WIP | Phase 4. Icon present, disabled, "AI — WIP". |
| File row | Open canvases/files as tabs | WIP | MVP is single-document. Renders exactly one real tab (current file); `+`/multi-doc: WIP. |
| Main | Cursor X,Y | LIVE | Existing pointer tracking. |
| Main | Canvas + scrollbars | LIVE | Existing pan/zoom; add visible scrollbar affordance bound to existing pan state. |
| Main | Rotate/Angle dial | WIP | Only fixed 90° rotate exists; arbitrary-angle input is WIP. |
| Main | Layers | LIVE | Existing layer panel. |
| Style row | Canvas style (background/backdrop/texture/export style) | WIP | No backing model. Render as a disabled dropdown listing the four sub-items, each "WIP". |
| Style row | Media bay (dir tree/resource finder/app storage/image browser) | WIP | No asset system. Same treatment. |
| Style row | New style tab (+) | WIP | Disabled. |
| Secondary2 | Find (elements/actions/commands/history/conversation) | WIP | No search index. |
| Secondary2 | Command Line (CLI/export/path/AutoCAD-style command) | WIP | The headless `pixelcad-cli` binary exists as a **separate process**, not an in-GUI console. Render the input bar, disabled, "Command line — WIP". |
| Secondary2 | AI Chat | WIP | Phase 4. |
| Secondary2 | Secondary tools (mini-window toolbox) | WIP | No multi-window support yet. |
| Tail | Move (drag/move/rotate) | WIP | No generic move/transform-by-drag tool; only arrow-key nudge and fixed selection commands exist. |
| Tail | Click/cursor (select) | LIVE | Existing Select tool. |
| Tail | Canvas size (size/format) | PARTIAL | Size exists (resize inputs). Unit/format (px/mm/in): WIP. |
| Tail | Zoom/Shrink | LIVE | Existing zoom fit/actual; add a numeric `− 100% +` stepper bound to existing `zoom-level`. |
| Right vertical strip | Rotate angle (precise input) | WIP | Same as Main's rotate dial — one control, not two. |
| Right vertical strip | Layers | LIVE | Same panel as Main's Layers; sketch shows it spanning both rows. |

## macOS window

macOS reuses the Linux content but reorganizes it into a **windowed** layout
(the only one built for MVP — see open decision below):

- **Tab icons sit in a horizontal strip along the bottom edge** (per the
  maintainer's correction to the sketch: "Icons arrange horizontally, when
  clicked expands vertically tabs"). Clicking an icon expands its panel
  **vertically upward** from the strip, dock-style; clicking it again, or
  the panel's close button, collapses it; clicking another icon switches
  the content in place. The eight tabs are File / Main / Style / Secondary /
  Tool / Command / Layers / AI.
- The rest of the window is the canvas column (Home toolbar, canvas,
  bottom bar with the icon strip, cursor readout, select/grid/resize, and
  the bottom-right zoom cluster).

| Region | Item | Status | Notes |
|---|---|---|---|
| Head+Secondary (merged, windowed) | Save / Undo / Redo | LIVE | Same bindings as Linux. |
| Head+Secondary | Files ▾ (file operations, expand) | PARTIAL | Open/Save/Save-project exist. New/Export/Recent-files: WIP entries in the dropdown. |
| Bottom icon strip: File (Media bay, File tree) | — | WIP | Same as Linux Style-row Media bay + single-document file list. |
| Bottom icon strip: Main (Default / Linux Home / Master management) | — | PARTIAL | "Linux Home" content = the same Home-tab tools, reused. "Master management": WIP, meaning unconfirmed — placeholder only. |
| Bottom icon strip: Style | — | WIP | Same as Linux Style row. |
| Bottom icon strip: Secondary (Linux Secondary1, tab picker) | — | PARTIAL | Tab switcher reused; content still Home-only for MVP. |
| Bottom icon strip: Tool (Find + Linux Secondary2) | — | WIP | Same as Linux Find/Command-line/AI-chat/Secondary-tools. |
| Bottom icon strip: Command | — | WIP | Same as Linux Command Line. |
| Bottom icon strip: Layers (layer settings + list) | — | PARTIAL | List exists; "layer settings" (lock/blend/groups) is WIP per `BACKLOG.md`. |
| Bottom icon strip: AI | — | WIP | Phase 4. |
| Bottom-right | Zoom/Shrink | LIVE | Same as Linux. |
| — | Fullscreen-only behaviors (Head/Secondary split, tab-to-miniwindow pop-out) | WIP / deferred | See open decision below. |

---

## Settings page (new)

Two tabs, present on both platforms; a third Linux-only tab is stubbed for
the future layout customization:

**Basic**
- Default new-canvas size (width × height)
- Autosave: on/off + interval
- Confirm before exit/close
- Recent-files list length

**Style**
- UI style: Auto-detect / Linux / macOS (drives which chrome renders)
- Theme: Light / Dark
- Accent color
- Toolbar icon size
- **macOS only:** per-item show/hide toggles ("add/reduce items", per the
  maintainer's note that macOS cannot reposition, only add/remove)

**Layout — Linux only, "coming later"**
- A single disabled row: "Rearrange toolbar items (drag & drop) — planned,
  see ROADMAP.md". No functionality in MVP; exists so the settings page's
  shape doesn't change again when it ships.

---

## Platform detection & first-run flow

1. On first launch (no settings file found), detect OS.
   - **macOS** → offer macOS-style UI.
   - **Linux** → offer Linux-style UI.
   - **Windows** → show a one-time note: "PixelCAD's Windows-native UI isn't
     built yet — consider running the Linux build under WSL. Continuing
     with the Linux-style interface for now." Then proceed with Linux style.
2. Dialog: **"Detected <OS> — use the matching interface style?"** with
   **Use detected** / **Choose manually** (manual shows both options
   regardless of OS, for testing/preference).
3. Persist the choice to a settings file. Never ask again; changeable later
   from Settings → Style → UI style.

---

## Open engineering decisions (need a decision before/while implementing)

1. **macOS window chrome.** The sketch's macOS "Head" bar sits where the
   traffic-light buttons are, with space "reserved" for them — implying a
   **custom-drawn unified title bar** (native title bar hidden, app draws
   its own bar including the reserved gap), like VS Code or Arc. This is
   real platform-integration work in a Slint app (borderless window +
   manual drag-to-move region; the traffic lights themselves still need to
   come from somewhere). The alternative is simpler: keep the native macOS
   title bar as-is, and put the "Head+Secondary" bar as an ordinary in-content
   toolbar directly below it. **Proposal for MVP: use the simple version**
   (native title bar + in-content bar below it) and file the custom unified
   title bar as a ROADMAP/BACKLOG item, since it requires native
   platform code beyond what Slint 1.8 exposes directly.
2. **Fullscreen-specific behavior.** Head/Secondary splitting, and popping
   a vertical-tab panel out into an independent miniwindow, only happens in
   fullscreen per the spec. Detecting fullscreen and spawning independent
   OS windows from Slint is nontrivial. **Proposal for MVP: implement the
   windowed layout only**; fullscreen gets the same layout (no split, no
   pop-out) and the pop-out buttons render as WIP. Deferred to ROADMAP.
3. **Process.** Should this redesign be tracked the way the rest of the
   project is (a `PLAN.md` + `PROGRESS.md` entry, ROADMAP line item), or
   implemented directly in this interactive session? Pending maintainer
   answer.

Proposals 1 and 2 will be assumed correct if not corrected, since they only
affect *how much* gets built now, not what the final design looks like —
everything deferred here is already marked WIP/roadmap in this document.
