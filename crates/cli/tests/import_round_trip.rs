//! Acceptance criterion AC11: `pixelcad-cli import <png> --out <pxcproj>`
//! followed by `pixelcad-cli run <pxcproj> --out <png>` reproduces the
//! source image's pixels exactly.
//!
//! This drives the **real compiled binary** for both halves rather than
//! calling library functions, so it also covers argument parsing, file I/O,
//! the project header, and the base64 payload surviving a trip through
//! disk.

use std::path::{Path, PathBuf};
use std::process::Command;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join(format!("pixelcad-cli-{tag}-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run_cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pixelcad-cli"))
        .args(args)
        .output()
        .expect("failed to spawn pixelcad-cli")
}

/// Writes an RGBA8 PNG using the same encoder settings the CLI itself uses.
fn write_png(path: &Path, width: u32, height: u32, pixels: &[u8]) {
    let file = std::fs::File::create(path).unwrap();
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(pixels).unwrap();
}

fn read_png_rgba(path: &Path) -> (u32, u32, Vec<u8>) {
    let file = std::fs::File::open(path).unwrap();
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    buf.truncate(info.buffer_size());
    assert_eq!(info.color_type, png::ColorType::Rgba, "test fixtures are RGBA");
    (info.width, info.height, buf)
}

/// A small but non-trivial source image: every channel varies, and it
/// includes fully transparent, partially transparent and opaque pixels so
/// an alpha-handling mistake cannot hide.
fn fixture_pixels(width: u32, height: u32) -> Vec<u8> {
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            pixels.extend_from_slice(&[
                (x * 7 % 256) as u8,
                (y * 13 % 256) as u8,
                ((x + y) * 3 % 256) as u8,
                match (x + y) % 3 {
                    0 => 0,
                    1 => 128,
                    _ => 255,
                },
            ]);
        }
    }
    pixels
}

#[test]
fn importing_a_png_and_replaying_the_project_reproduces_the_source_pixels() {
    let dir = temp_dir("import-round-trip");
    let source = dir.join("source.png");
    let project = dir.join("imported.pxcproj");
    let rendered = dir.join("rendered.png");

    let (w, h) = (23u32, 17u32); // deliberately not a power of two
    let original = fixture_pixels(w, h);
    write_png(&source, w, h, &original);

    let out = run_cli(&["import", source.to_str().unwrap(), "--out", project.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "import failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let project_text = std::fs::read_to_string(&project).unwrap();
    assert!(
        project_text.contains("pixelcad.project version=1"),
        "the import must emit a versioned project header"
    );
    assert!(project_text.contains("image.import"), "the pixels must travel as a command");

    let out = run_cli(&["run", project.to_str().unwrap(), "--out", rendered.to_str().unwrap()]);
    assert!(out.status.success(), "run failed: {}", String::from_utf8_lossy(&out.stderr));

    let (rw, rh, round_tripped) = read_png_rgba(&rendered);
    assert_eq!((rw, rh), (w, h), "dimensions must survive the round trip");
    assert_eq!(round_tripped, original, "pixel data must survive the round trip exactly");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn replaying_a_project_twice_produces_byte_identical_pngs() {
    // AC12: determinism is preserved for the new container format, not just
    // for bare Phase 0 scripts.
    let dir = temp_dir("project-determinism");
    let source = dir.join("source.png");
    let project = dir.join("p.pxcproj");
    write_png(&source, 9, 9, &fixture_pixels(9, 9));

    assert!(run_cli(&["import", source.to_str().unwrap(), "--out", project.to_str().unwrap()])
        .status
        .success());

    let mut outputs = Vec::new();
    for name in ["a.png", "b.png"] {
        let out_path = dir.join(name);
        assert!(run_cli(&["run", project.to_str().unwrap(), "--out", out_path.to_str().unwrap()])
            .status
            .success());
        outputs.push(std::fs::read(&out_path).unwrap());
    }
    assert_eq!(outputs[0], outputs[1], "the same project must render byte-identically");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_future_version_project_is_refused_and_writes_no_output() {
    // AC10 at the binary level: the error names both versions, the exit
    // code is non-zero, and no partial PNG is left behind.
    let dir = temp_dir("future-version");
    let project = dir.join("future.pxcproj");
    let out_path = dir.join("should-not-exist.png");
    std::fs::write(&project, "pixelcad.project version=99\ncanvas.new width=4 height=4\n")
        .unwrap();

    let out = run_cli(&["run", project.to_str().unwrap(), "--out", out_path.to_str().unwrap()]);
    assert!(!out.status.success(), "a future-version project must fail");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("99"), "error must name the found version: {stderr}");
    assert!(stderr.contains("version 1"), "error must name the supported version: {stderr}");
    assert!(!out_path.exists(), "no output file may be produced");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_phase_0_script_still_runs_through_the_new_loader() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let script = root.join("docs/samples/ship.pxc");
    let dir = temp_dir("legacy-script");
    let out_path = dir.join("ship.png");

    let out = run_cli(&["run", script.to_str().unwrap(), "--out", out_path.to_str().unwrap()]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let (w, h, _) = read_png_rgba(&out_path);
    assert_eq!((w, h), (64, 64));

    let _ = std::fs::remove_dir_all(&dir);
}
