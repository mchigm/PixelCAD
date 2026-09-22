# PixelCAD

An open-source, lightweight, pixel-first design studio for technical and
blueprint work. Every action is a replayable command, usable from the GUI or
the CLI, growing toward simple CAD and AI assistance.

See `ROADMAP.md` for the phased development plan and `PLAN.md` (or
`Plans/archive/`) for the plan driving the current or most recently completed
phase.

## Workspace layout

- `crates/core` (`pixelcad-core`) — headless document model and typed command
  engine. No UI, GPU, or platform dependency; depends only on `thiserror`.
- `crates/app` (`pixelcad-app`) — Slint desktop shell; dispatches commands to
  `pixelcad-core` and renders the composited document.
- `crates/cli` (`pixelcad-cli`) — headless CLI that replays command scripts
  and project files through `pixelcad-core`, exports PNG, and imports PNG.

## Building

```sh
cargo build --workspace
cargo test --workspace
```

## Running the CLI

```sh
# Render a script or a project to PNG
cargo run -p pixelcad-cli -- run docs/samples/blueprint.pxcproj --out blueprint.png
cargo run -p pixelcad-cli -- run docs/samples/ship.pxc --out ship.png

# Turn a PNG into an editable project
cargo run -p pixelcad-cli -- import photo.png --out photo.pxcproj
```

Rendering the same input twice always produces byte-identical output. That
is a hard invariant, enforced by tests, not an incidental property.

## Running the GUI

```sh
cargo run -p pixelcad-app
```

On first launch the app detects your operating system and asks once whether
to use the matching window layout; the choice is saved and can be changed
later in **Settings → Style**.

### The window

Two layouts share one codebase:

- **Linux** stacks horizontal bands (title bar, ribbon tabs with the **Home**
  toolbar, file tabs, canvas, style row, command strip, status tail), in the
  GIMP/Krita idiom.
- **macOS** keeps the native title bar, condenses the file controls into an
  in-content top bar, and docks the eight panel icons (**File / Main / Style /
  Secondary / Tool / Command / Layers / AI**) in a horizontal strip along the
  bottom edge. Clicking an icon expands its panel vertically upward;
  clicking again collapses it. Fullscreen splits the top bar into two rows
  and reveals (WIP) per-panel pop-out markers.

Features from the design that are not implemented yet render as dashed,
amber-marked **WIP** controls rather than being hidden. The full
LIVE/PARTIAL/WIP classification is in `docs/design/ui-redesign-spec.md`,
and each WIP item's destination phase is in
`docs/design/ui-roadmap-mapping.md`.

Settings (**gear icon**) persist to `settings.toml` in the platform config
directory (e.g. `~/Library/Application Support/dev.PixelCAD.PixelCAD/settings.toml`
on macOS, `~/.config/pixelcad/settings.toml` on Linux). Style offers interface
style (Auto / Linux / macOS), colour scheme (dark / light / high contrast),
colour design (cyanotype / graphite / amber / phosphor), text style
(compact / normal / comfortable) plus a monospace-labels switch, icon size,
and an accent-colour override. Everything applies live and saves
immediately — there is no Apply button.

### Controls

Shortcuts use `Cmd` on macOS and `Ctrl` on Linux/Windows; both are written
below as **accel**.

| Action | How |
|---|---|
| Pick a tool | `b` brush, `e` eraser, `g` fill, `i` eyedropper, `l` line, `r` rectangle, `o` ellipse, `p` polyline, `a` arrow, `t` text, `m` marquee, `f` lasso |
| Draw / erase | Left-click-drag with the Brush or Eraser |
| Brush size / shape | `[` / `]`, or the − / + buttons; the brush-shape button toggles the tip |
| Fill tolerance | **tol− / tol+** (0 = exact match, like Phase 1) |
| Stroke opacity | **op− / op+** (sets the alpha written; see the note below) |
| Pan | Right-click-drag, or hold `Space` and left-click-drag |
| Zoom | Scroll, or `+` / `-`; **Fit** = accel+`9`, **1:1** = accel+`0` |
| Grid | **Grid on/off**, or `G` (appears at 8× and above) |
| Palette slot | `1`–`8`, or click a swatch; **Recent** keeps the last 8 colours |
| Edit a swatch | Select it, type a hex value, press **Set** |
| Select all | accel+`A`, or **Select all** |
| Clear selection | `Shift`+accel+`D`, or **Clear selection** |
| Delete selection | `Delete` / `Backspace`, or **Delete** |
| Nudge selection | Arrow keys (moves the *pixels*, not just the marquee) |
| Cut / copy / paste | accel+`X` / `C` / `V` |
| Copy visible layers | `Shift`+accel+`C`, or **Copy visible** |
| Duplicate selection | accel+`D` |
| Flip / rotate | accel+`H` / accel+`U` / accel+`R`, or the flip and rotate buttons in the Select group |
| Crop to selection | **Crop** |
| Resize canvas | Type width and height, press **Resize**; **Scale sel** resamples the selection (bottom bar) |
| Undo / redo | accel+`Z`, `Shift`+accel+`Z`, or the buttons |
| Layers | **Add** (accel+`N`), **Duplicate** (accel+`J`), **Merge down** (accel+`E`), **Raise** / **Lower**, **Delete**; click a row to select it, its checkbox to hide it |
| Save / open | **Save project** and **Open** write and read `.pxcproj`; **Save script** writes plain `.pxc` |

One pointer drag is one undo step, no matter how many commands it emits.

**A note on opacity.** Pixel writes *replace* rather than blend — that is
what makes the eraser erase instead of painting white. Opacity is therefore
the alpha actually written, so painting twice at 50 % gives 50 %, not 75 %.
Accumulating "flow" needs a blend mode in the write path and is in
`BACKLOG.md`.

## File formats

### `.pxc` — command script

Plain text, one command per line. Blank lines and lines whose first
non-whitespace character is `#` are comments. Values containing whitespace
must be double-quoted; the serializer always quotes colours and names.
Colours are `#rrggbb` (implicit `ff` alpha) or `#rrggbbaa`.

```text
# Canvas and palette
canvas.new     width=<u32> height=<u32>
palette.set    index=<0..15> color=<hex>
color.set      color=<hex>                     # active drawing colour
color.pick     x=<i64> y=<i64>                 # eyedropper (reads the composite)

# Layers (index 0 is the bottom of the stack)
layer.add      name="<text>"                   # inserted above the active layer
layer.select   index=<usize>
layer.remove   index=<usize>                   # never the last remaining layer
layer.rename   index=<usize> name="<text>"
layer.move     from=<usize> to=<usize>
layer.visible  index=<usize> value=<true|false>
layer.opacity  index=<usize> value=<0..255>

# Selection — clips every subsequent pixel write
select.rect    x=<i64> y=<i64> width=<u32> height=<u32>
select.clear

layer.duplicate index=<usize>                  # independent copy, above
layer.merge    index=<usize>                   # composite down into index-1

# Selection — clips every subsequent pixel write
select.all
select.rect    x=<i64> y=<i64> width=<u32> height=<u32>
select.lasso   points="x,y x,y ..."            # closed polygon, boundary included
select.clear

# Selection contents (all need an active selection)
selection.delete
selection.move   dx=<i64> dy=<i64>             # moves the pixels, not just the marquee
selection.flip   axis=<horizontal|vertical>
selection.rotate degrees=<90|180|270>
selection.scale  width=<u32> height=<u32>      # nearest-neighbour

# Canvas
canvas.crop    x=<i64> y=<i64> width=<u32> height=<u32>
canvas.resize  width=<u32> height=<u32>        # nearest-neighbour

# Drawing (all act on the active layer)
pixel.set      x=<i64> y=<i64> color=<hex>
line.draw      x0=<i64> y0=<i64> x1=<i64> y1=<i64> color=<hex>
brush.stroke   x0=<i64> y0=<i64> x1=<i64> y1=<i64> size=<u32> [shape=<square|round>] color=<hex>
rect.draw      x0=<i64> y0=<i64> x1=<i64> y1=<i64> [radius=<u32>] fill=<bool> color=<hex>
ellipse.draw   x0=<i64> y0=<i64> x1=<i64> y1=<i64> fill=<bool> color=<hex>
polygon.draw   x=<i64> y=<i64> radius=<u32> sides=<u32> [rotation=<deg>] fill=<bool> color=<hex>
arrow.draw     x0=<i64> y0=<i64> x1=<i64> y1=<i64> [head=<u32>] color=<hex>
polyline.draw  points="x,y x,y ..." color=<hex>
fill.bucket    x=<i64> y=<i64> [tolerance=<0-255>] color=<hex>
text.draw      x=<i64> y=<i64> text="..." [font=<small|bold>] [scale=<u32>]
               [align=<left|center|right>] color=<hex>
image.import   x=<i64> y=<i64> width=<u32> height=<u32> data="<base64 rgba8>"
```

Fields in `[brackets]` are optional. Every one of them defaults to the
pre-1.5 behaviour (`shape=square`, `radius=0`, `tolerance=0`,
`font=small`, `scale=1`, `align=left`, `rotation=0`), which is why scripts
written before those parameters existed still render byte-identically.

Two things worth knowing:

- **There is no eraser command.** Pixel writes replace rather than blend, so
  `brush.stroke … color="#00000000"` *is* the eraser.
- **A selection is a shape, not just a box.** `select.lasso` carries a
  coverage mask, so drawing, deleting and moving all respect the traced
  outline rather than its bounding rectangle.
- **Text is a bitmap operation.** It uses the in-repo 5x7 typeface
  (`crates/core/src/font.rs`), not a system font — a system font would be
  floating point, platform-dependent, and would drag a shaping dependency
  into `pixelcad-core`.
- **Geometry is integer-only**, including a fixed-point sine table for
  `polygon.draw` and an integer square root for `arrow.draw`, so shapes are
  bit-identical on every platform rather than merely similar.
- `pixel.set` and `line.draw` **error** on an out-of-canvas coordinate;
  `brush.stroke`, `rect.draw`, `fill.bucket` and `image.import` clip
  silently, because a wide brush at the border must not abort a stroke.

### `.pxcproj` — versioned project file

A `.pxc` command log with a version header as its first non-comment line:

```text
pixelcad.project version=1
canvas.new width=128 height=96
...
```

It is a text container, not a zip. The reasoning is in
`crates/core/src/project.rs`: no dependency, no timestamps or compressor
behaviour to threaten byte-determinism, git-diffable, and it makes "open a
project" literally "replay its commands".

A file with no header loads as legacy version `0` — that is exactly a
pre-1.0 `.pxc` script, and those keep working. A file declaring a version
newer than this build supports is rejected outright, before any command
executes, so it can never half-apply to an open document.

## Samples

- `docs/samples/ship.pxc` — the Phase 0 artefact: a 64×64 single-layer ship
  outline. Its rendered sha256 is pinned by a test as a regression guard.
- `docs/samples/blueprint.pxcproj` — the Phase 1 artefact: a 128×96
  four-layer historical ship blueprint (paper, grid, hull, annotation)
  exercising every tool, layer opacity, and a selection-masked fill.
- `docs/samples/paint_parity.pxcproj` — the Phase 1.5 artefact: a 160×120
  four-layer tool sampler covering ellipses, rounded rectangles, polygons,
  arrows, polylines, bitmap text, lasso selection, flip/rotate/move, a
  tolerance fill, and layer duplicate/merge.

A test asserts each sample still *uses* the commands it exists to
demonstrate, so a sample cannot quietly stop dogfooding them.

## License

MIT — see `LICENSE`.
