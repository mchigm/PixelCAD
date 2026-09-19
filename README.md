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

### Controls

| Action | How |
|---|---|
| Draw / erase | Left-click-drag with the Brush or Eraser tool |
| Pan | Right-click-drag, or hold Space and left-click-drag |
| Zoom | Scroll (grid appears at 8× and above), or `+` / `-` |
| Pick a tool | `b` brush, `e` eraser, `g` fill, `i` eyedropper, `l` line, `r` rectangle, `m` marquee |
| Brush size | `[` / `]`, or the − / + buttons |
| Palette slot | `1`–`8`, or click a swatch |
| Edit a swatch | Select it, type a hex value, press **Set** |
| Undo / redo | `Ctrl`/`Cmd`+`Z`, `Shift`+`Ctrl`/`Cmd`+`Z`, or the buttons |
| Clear selection | `Ctrl`/`Cmd`+`D`, or **Clear selection** |
| Layers | **Add layer** / **Delete layer**; click a row to select it, its checkbox to hide it |
| Save / open | **Save project** and **Open** write and read `.pxcproj`; **Save script** writes plain `.pxc` |

One pointer drag is one undo step, no matter how many commands it emits.

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

# Drawing (all act on the active layer)
pixel.set      x=<i64> y=<i64> color=<hex>
line.draw      x0=<i64> y0=<i64> x1=<i64> y1=<i64> color=<hex>
brush.stroke   x0=<i64> y0=<i64> x1=<i64> y1=<i64> size=<u32> color=<hex>
rect.draw      x0=<i64> y0=<i64> x1=<i64> y1=<i64> fill=<true|false> color=<hex>
fill.bucket    x=<i64> y=<i64> color=<hex>
image.import   x=<i64> y=<i64> width=<u32> height=<u32> data="<base64 rgba8>"
```

Two things worth knowing:

- **There is no eraser command.** Pixel writes replace rather than blend, so
  `brush.stroke … color="#00000000"` *is* the eraser.
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

## License

MIT — see `LICENSE`.
