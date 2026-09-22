# PixelCAD — Design System

Tokens for the platform-aware UI shell (ROADMAP Phase 1.75). Both the Linux
and macOS chrome trees use **this one visual identity**; only the structural
arrangement of the chrome differs per platform, the way GIMP, Krita and
Blender look like themselves on every OS.

---

## 1. Design direction

**Subject:** a pixel-first drawing studio for technical and blueprint work,
whose defining promise is that every action is exact and replayable.
**Audience:** drafters and makers drawing precise technical art (the
reference artefact in this repo is a historical ship blueprint).
**The canvas is the hero.** Chrome exists to stay out of the canvas's way,
so the boldness budget is spent in exactly one place: a single blueprint-cyan
accent that marks *what is active right now* and nothing else.

Three choices, each grounded in the subject rather than picked as a default:

- **Zero corner radius on canvas-adjacent chrome.** This tool's entire value
  proposition is pixel-exactness; rounded, blobby chrome would contradict the
  product it frames. Radius appears only on modal overlays (4px), which float
  *above* the drawing surface rather than belonging to it.
- **Monospace for live numeric readouts only** (cursor X,Y, zoom %, canvas
  size, command line). This is functional, not decorative: proportional
  digits make a live-updating coordinate readout jitter as the pointer
  moves. Every other label uses the platform UI sans.
- **Dashed borders mark WIP controls.** In a drafting tool a dashed line
  already means "construction geometry — drawn, not built," so the sketched
  but unimplemented controls read correctly to this audience without needing
  a legend.

---

## 2. Color

Colour is two orthogonal axes, both user-settable in Settings → Style and
both persisted:

- **Colour scheme** decides *how light* the chrome is: `dark` (default),
  `light`, `high-contrast`.
- **Colour design** decides *what hue* the surfaces and accent are:
  `cyanotype` (default), `graphite`, `amber`, `phosphor`.

Keeping them separate means four designs × three schemes = twelve
appearances from four palette definitions, instead of twelve
hand-maintained themes that drift apart.

### The four colour designs

Each is grounded in the subject matter rather than being an arbitrary hue
pick:

| Design | Idea | Dark accent | Light accent |
|---|---|---|---|
| **Cyanotype** | Blueprint cyan on cold near-black — the product's own identity | `#3FA7D6` | `#1C6E96` |
| **Graphite** | Pencil lead and paper; neutral greys, lead-blue accent | `#8FA3B8` | `#4A5B6E` |
| **Amber** | A drafting lamp / amber CRT on warm near-black | `#E0A33E` | `#A4701A` |
| **Phosphor** | Green CRT terminal, a nod to the command-line heritage | `#4FD99B` | `#17795A` |

Users can additionally override the accent with any `#rrggbb` value; an
empty override means "follow the design".

**One non-obvious rule:** the WIP marker must never be mistakable for the
accent, so it is normally amber — except under the Amber design, where it
switches to violet (`#B98CE0` / `#7A4BA8`). A warm WIP dot on a warm accent
would read as "active".

### Reference palette (Cyanotype)

The token *roles* below apply to every design; only the values change.
Dark is the default, because a bright chrome fights the artwork.

### Dark (default)

| Token | Hex | Use |
|---|---|---|
| `surface-base` | `#0F171C` | window chrome background |
| `surface-raised` | `#17232A` | toolbars, side panels, tab strips |
| `surface-sunken` | `#0A1115` | the viewport pit the canvas sits in |
| `line` | `#24343D` | 1px hairlines, dividers, control borders |
| `text-primary` | `#DCE7EC` | labels, values |
| `text-muted` | `#7E949F` | secondary labels, inactive tabs |
| `accent` | `#3FA7D6` | active tool, focus ring, selection marquee |
| `accent-quiet` | `#1E5A78` | active-tab underline, pressed fill |
| `wip` | `#B08A3E` | WIP markers only — never used for active state |

### Light

| Token | Hex | Use |
|---|---|---|
| `surface-base` | `#EDF1F3` | window chrome background |
| `surface-raised` | `#FFFFFF` | toolbars, side panels, tab strips |
| `surface-sunken` | `#DCE3E7` | viewport pit |
| `line` | `#C2CFD6` | hairlines |
| `text-primary` | `#142026` | labels, values |
| `text-muted` | `#5A6C76` | secondary labels |
| `accent` | `#1C6E96` | active tool, focus ring |
| `accent-quiet` | `#A9D3E6` | active-tab underline, pressed fill |
| `wip` | `#8A6A20` | WIP markers |

`accent` and `wip` are deliberately far apart in hue so a WIP control can
never be mistaken for an active one. Both themes keep body text at or above
4.5:1 against their own surfaces.

The accent is user-overridable (Settings → Style → accent color); every
other token is theme-derived.

---

## 3. Typography

One family, two roles — no display/body split, because a tool UI has no
display type.

**Text style** is user-settable (Settings → Style) as a named step rather
than a free number, so it cannot be set to something unreadable:

| Text style | Scale | Effect |
|---|---|---|
| Compact | 0.92× | denser toolbars, more canvas |
| Normal | 1.00× | default |
| Comfortable | 1.15× | larger labels |

The scale multiplies every text size **and** the toolbar/tab/tail row
heights, so larger text grows its container instead of clipping inside it.

A **monospace labels** switch is also offered. Numeric readouts are always
monospace (see below); this extends it to every label, which suits users
coming from CAD and terminal tools.

- **UI sans:** the platform default UI font (Slint's default). Using the
  host platform's own UI face is the correct behavior for a desktop tool,
  not a fallback.
- **Mono:** platform default monospace, used *only* for live numeric
  readouts and the command line, for the tabular-digit reason above.

| Role | Size | Weight | Notes |
|---|---|---|---|
| Section label | 11px | 600 | sentence case — never all-caps |
| Control label / body | 13px | 400 | base size |
| Active / emphasis | 13px | 600 | |
| Filename, dialog title | 15px | 500 | |
| Settings / dialog heading | 20px | 600 | |
| Numeric readout | 12px | 400 | mono, tabular |

Line-height 1.35 throughout (dense tool UI). No tracked-out caps anywhere.

---

## 4. Spacing, grid, borders

- **4px base grid.** Steps: 4, 8, 12, 16, 24, 32.
- **Border radius:** `0` on all chrome; `4px` on modal overlays only.
- **Borders:** 1px solid `line`. Panels are separated by hairlines, not
  shadows — this UI has no drop shadows except the modal scrim.
- **Toolbar row height:** 32px. **Tab strip:** 28px. **Status/tail bar:** 24px.
- **Hit targets:** minimum 24×24px for icon buttons in dense toolbars
  (desktop pointer input), 32px for primary actions and all dialog buttons.

---

## 5. Icons

- **Set:** [Lucide](https://lucide.dev) `lucide-static@0.544.0`, **ISC
  license** — license text vendored at `crates/app/ui/icons/LICENSE-lucide.txt`.
- **Vendored** as 60 local SVGs in `crates/app/ui/icons/`; no runtime
  network access.
- `stroke="currentColor"` was rewritten to `#000000` at vendor time so
  Slint's SVG renderer produces a deterministic mask; icons are then tinted
  at use-site with Slint's `colorize` property, which is how a single icon
  file serves both themes and the active/inactive/WIP states.
- **Default render size:** 16px (Settings → Style offers 14 / 16 / 20).
- A bespoke pixel-grid icon set drawn on the same 16px grid as the canvas
  would suit this product better than a general-purpose stroke set; that is
  recorded as a future polish item in `BACKLOG.md` rather than done now.

---

## 6. State treatments

| State | Treatment |
|---|---|
| Default | `text-primary` icon/label on `surface-raised`, 1px `line` border |
| Hover | background lifts to `accent-quiet` at 25% opacity, 80ms |
| Active / selected | `accent` icon + 2px `accent` bottom rule (tabs) or `accent-quiet` fill (tools) |
| Disabled (real) | 40% opacity, no border change |
| **WIP** | 55% opacity, **1px dashed `line` border**, small `wip` dot at top-right, `accessible-label` suffixed `" — WIP (not implemented yet)"` |
| Focus (keyboard) | 2px `accent` outline, always visible — never removed |

Motion is limited to the 80ms hover fade and state changes that answer a
user action. There are no entrance animations, no scroll-triggered reveals,
and nothing moves unless the user moved it.

---

## 7. Applying this to the two chrome trees

Identical tokens, different structure:

- **Linux** stacks horizontal bands (title → ribbon tabs → contextual
  toolbar → file tabs → canvas row → style row → secondary row → tail),
  which is the GIMP/Krita idiom its users expect.
- **macOS** keeps the native title bar untouched and condenses the same
  content into one in-content top bar plus a vertical tab sidebar and a
  bottom-right zoom cluster, which is the macOS idiom.

Neither tree re-implements the canvas, layers panel or Home toolbar — those
are shared components, so a token change lands in both trees at once.
