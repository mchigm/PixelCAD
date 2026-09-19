//! Acceptance criterion: `pixelcad-cli run docs/samples/ship.pxc --out out.png`
//! run twice produces byte-identical PNG files.

use std::path::PathBuf;
use std::process::Command;

fn workspace_root() -> PathBuf {
    // crates/cli/tests -> crates/cli -> crates -> workspace root
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn replaying_ship_script_twice_produces_byte_identical_png() {
    let root = workspace_root();
    let script = root.join("docs/samples/ship.pxc");
    assert!(script.exists(), "sample script missing at {}", script.display());

    let out_dir = std::env::temp_dir()
        .join(format!("pixelcad-cli-determinism-test-{}", std::process::id()));
    std::fs::create_dir_all(&out_dir).unwrap();
    let out_a = out_dir.join("a.png");
    let out_b = out_dir.join("b.png");

    let bin = env!("CARGO_BIN_EXE_pixelcad-cli");

    for out in [&out_a, &out_b] {
        let status = Command::new(bin)
            .args(["run", script.to_str().unwrap(), "--out", out.to_str().unwrap()])
            .status()
            .expect("failed to run pixelcad-cli");
        assert!(status.success(), "pixelcad-cli exited with failure for {}", out.display());
    }

    let bytes_a = std::fs::read(&out_a).unwrap();
    let bytes_b = std::fs::read(&out_b).unwrap();
    assert_eq!(bytes_a, bytes_b, "PNG bytes differ between two runs of the same script");

    let _ = std::fs::remove_dir_all(&out_dir);
}

/// AC17, binary level: the Phase 1 dogfood blueprint renders, and renders
/// byte-identically on two consecutive runs.
#[test]
fn rendering_the_blueprint_project_twice_produces_byte_identical_png() {
    let root = workspace_root();
    let project = root.join("docs/samples/blueprint.pxcproj");
    assert!(project.exists(), "blueprint missing at {}", project.display());

    let out_dir =
        std::env::temp_dir().join(format!("pixelcad-cli-blueprint-test-{}", std::process::id()));
    std::fs::create_dir_all(&out_dir).unwrap();
    let bin = env!("CARGO_BIN_EXE_pixelcad-cli");

    let mut renders = Vec::new();
    for name in ["a.png", "b.png"] {
        let out = out_dir.join(name);
        let output = Command::new(bin)
            .args(["run", project.to_str().unwrap(), "--out", out.to_str().unwrap()])
            .output()
            .expect("failed to run pixelcad-cli");
        assert!(
            output.status.success(),
            "pixelcad-cli failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        renders.push(std::fs::read(&out).unwrap());
    }

    assert_eq!(renders[0], renders[1], "the blueprint must render deterministically");
    assert!(renders[0].len() > 200, "the render should not be a trivial image");

    let _ = std::fs::remove_dir_all(&out_dir);
}
