//! `pixelcad-cli` — headless replay of `.pxc` command scripts to PNG.
//!
//! This binary contains no drawing logic of its own: it reads a script,
//! hands it to `pixelcad-core::{parse_script, Engine}` verbatim, and encodes
//! whatever `Document` results. Any two runs of the same script, on the same
//! machine or a different one, must produce byte-identical PNG bytes — that
//! is the whole point of a headless surface sharing one command engine with
//! the GUI.

use std::fs;
use std::io::BufWriter;
use std::process::ExitCode;

use pixelcad_core::open_project;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<(), String> {
    match args {
        [cmd, script_path, flag, out_path] if cmd == "run" && flag == "--out" => {
            run_script(script_path, out_path)
        }
        _ => Err(usage()),
    }
}

fn usage() -> String {
    "usage: pixelcad-cli run <script.pxc|project.pxcproj> --out <output.png>".to_string()
}

fn run_script(script_path: &str, out_path: &str) -> Result<(), String> {
    let text = fs::read_to_string(script_path)
        .map_err(|e| format!("failed to read {script_path}: {e}"))?;

    // One code path for both formats: `open_project` accepts a versioned
    // `.pxcproj` and a bare Phase 0 `.pxc` script alike, and refuses to
    // return an engine at all unless the entire file parsed and replayed.
    let engine =
        open_project(&text).map_err(|e| format!("cannot open {script_path}: {e}"))?;

    let document = engine
        .document()
        .ok_or_else(|| "script never ran canvas.new; nothing to export".to_string())?;

    // Export the *composited* stack, not any single layer's buffer: the PNG
    // must show exactly what the GUI shows.
    write_png(out_path, document.width(), document.height(), &document.composite())
        .map_err(|e| format!("failed to write {out_path}: {e}"))
}

/// Encodes an RGBA8 buffer to PNG. Uses no timestamp or platform-dependent
/// metadata, so byte output depends only on `width`, `height`, and `pixels`.
fn write_png(path: &str, width: u32, height: u32, pixels: &[u8]) -> Result<(), String> {
    let file = fs::File::create(path).map_err(|e| e.to_string())?;
    let writer = BufWriter::new(file);

    let mut encoder = png::Encoder::new(writer, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);

    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(pixels).map_err(|e| e.to_string())?;
    Ok(())
}
