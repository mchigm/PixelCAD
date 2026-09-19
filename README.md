# PixelCAD

An open-source, lightweight, pixel-first design studio for technical and
blueprint work. Every action is a replayable command, usable from the GUI or
the CLI, growing toward simple CAD and AI assistance.

See `ROADMAP.md` for the phased development plan and `PLAN.md` (or
`Plans/archive/`) for the plan driving the current or most recently completed
phase.

## Workspace layout

- `crates/core` (`pixelcad-core`) — headless document model and typed command
  engine. No UI, GPU, or platform dependency.
- `crates/app` (`pixelcad-app`) — Slint desktop shell; dispatches commands to
  `pixelcad-core` and renders the document buffer.
- `crates/cli` (`pixelcad-cli`) — headless CLI that replays `.pxc` command
  scripts through `pixelcad-core` and exports PNG.

## Building

```sh
cargo build
```

## Running the CLI

```sh
cargo run -p pixelcad-cli -- run docs/samples/ship.pxc --out out.png
```

## Running the GUI

```sh
cargo run -p pixelcad-app
```

## License

MIT — see `LICENSE`.
