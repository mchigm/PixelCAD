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

use pixelcad_core::{base64, open_project, serialize_project, Command, Engine};

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
        [cmd, png_path, flag, out_path] if cmd == "import" && flag == "--out" => {
            import_png(png_path, out_path)
        }
        _ => Err(usage()),
    }
}

fn usage() -> String {
    concat!(
        "usage:\n",
        "  pixelcad-cli run    <script.pxc|project.pxcproj> --out <output.png>\n",
        "  pixelcad-cli import <input.png>                  --out <project.pxcproj>"
    )
    .to_string()
}

/// Converts a PNG into a version-1 project consisting of a `canvas.new`
/// sized to the image plus one `image.import` carrying its pixels.
///
/// Going through commands rather than constructing a `Document` directly is
/// the point: an imported image is ordinary, undoable, replayable document
/// history like anything else the user does.
fn import_png(png_path: &str, out_path: &str) -> Result<(), String> {
    let (width, height, rgba) =
        read_png_rgba8(png_path).map_err(|e| format!("failed to read {png_path}: {e}"))?;

    let mut engine = Engine::new();
    engine
        .execute(Command::CanvasNew { width, height })
        .map_err(|e| format!("cannot create a {width}x{height} canvas: {e}"))?;
    engine
        .execute(Command::ImageImport {
            x: 0,
            y: 0,
            width,
            height,
            data: base64::encode(&rgba),
        })
        .map_err(|e| format!("cannot import image data: {e}"))?;

    fs::write(out_path, serialize_project(&engine))
        .map_err(|e| format!("failed to write {out_path}: {e}"))
}

/// Decodes a PNG to packed RGBA8, converting from whatever colour type the
/// file actually uses. Returns `(width, height, pixels)`.
fn read_png_rgba8(path: &str) -> Result<(u32, u32, Vec<u8>), String> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    // Normalise everything to 8-bit RGBA so the rest of the pipeline only
    // ever sees one pixel layout.
    decoder.set_transformations(png::Transformations::normalize_to_color8());

    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    buf.truncate(info.buffer_size());

    let pixel_count = info.width as usize * info.height as usize;
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => widen(&buf, pixel_count, 3, |p| [p[0], p[1], p[2], 255]),
        png::ColorType::Grayscale => widen(&buf, pixel_count, 1, |p| [p[0], p[0], p[0], 255]),
        png::ColorType::GrayscaleAlpha => {
            widen(&buf, pixel_count, 2, |p| [p[0], p[0], p[0], p[1]])
        }
        png::ColorType::Indexed => {
            return Err("indexed PNGs should have been expanded by the decoder".to_string())
        }
    };
    Ok((info.width, info.height, rgba))
}

fn widen(
    src: &[u8],
    pixel_count: usize,
    stride: usize,
    to_rgba: impl Fn(&[u8]) -> [u8; 4],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(pixel_count * 4);
    for i in 0..pixel_count {
        out.extend_from_slice(&to_rgba(&src[i * stride..(i + 1) * stride]));
    }
    out
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
