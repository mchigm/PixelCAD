//! Acceptance criterion AC17: `docs/samples/blueprint.pxcproj` is the
//! Phase 1 exit criterion made concrete — a historical ship blueprint drawn
//! end to end with nothing but the Phase 1 tool set.
//!
//! These tests assert the properties that make it *evidence* rather than
//! decoration: it is a valid version-1 project, it genuinely uses layers
//! and the full command vocabulary, and it replays deterministically.

use std::collections::BTreeSet;

use pixelcad_core::{open_project, parse_project, Engine, PROJECT_FORMAT_VERSION};

const BLUEPRINT: &str = include_str!("../../../docs/samples/blueprint.pxcproj");

#[test]
fn the_blueprint_is_a_current_version_project() {
    let project = parse_project(BLUEPRINT).expect("blueprint.pxcproj must parse");
    assert_eq!(project.version, PROJECT_FORMAT_VERSION);
    assert!(project.commands.len() > 80, "the blueprint should be substantial drawing work");
}

#[test]
fn the_blueprint_has_at_least_three_named_layers() {
    let engine = open_project(BLUEPRINT).expect("blueprint.pxcproj must replay");
    let doc = engine.document().expect("the blueprint must create a canvas");
    assert_eq!((doc.width(), doc.height()), (128, 96));

    let names: Vec<&str> = doc.layers().iter().map(|l| l.name()).collect();
    assert!(doc.layer_count() >= 3, "AC17 requires at least 3 layers, got {names:?}");
    assert_eq!(names, vec!["Paper", "Grid", "Hull", "Annotation"]);

    // The grid layer is deliberately drawn at reduced layer opacity rather
    // than in a paler colour, which is the feature this exercises.
    assert_eq!(doc.layers()[1].opacity(), 48);
    assert!(doc.layers().iter().all(|l| l.visible()));
}

#[test]
fn the_blueprint_exercises_the_phase_1_tool_set() {
    // If a later change silently stops the sample from covering a tool,
    // this fails — the dogfood artefact is only useful while it dogfoods.
    let project = parse_project(BLUEPRINT).unwrap();
    let used: BTreeSet<&str> = project.commands.iter().map(|c| c.name()).collect();

    for required in [
        "canvas.new",
        "palette.set",
        "layer.add",
        "layer.rename",
        "layer.select",
        "layer.opacity",
        "pixel.set",
        "line.draw",
        "brush.stroke",
        "rect.draw",
        "fill.bucket",
        "select.rect",
        "select.clear",
    ] {
        assert!(used.contains(required), "the blueprint never uses {required:?}; used: {used:?}");
    }
}

#[test]
fn the_blueprint_replays_deterministically() {
    let commands = parse_project(BLUEPRINT).unwrap().commands;

    let mut a = Engine::new();
    a.execute_all(commands.clone()).unwrap();
    let mut b = Engine::new();
    b.execute_all(commands).unwrap();

    assert_eq!(a.document_hash(), b.document_hash());
    assert_eq!(
        a.document().unwrap().composite(),
        b.document().unwrap().composite(),
        "the composited output must be byte-identical between runs"
    );
}

#[test]
fn the_blueprint_survives_a_save_and_reload_cycle() {
    let engine = open_project(BLUEPRINT).unwrap();
    let saved = pixelcad_core::serialize_project(&engine);
    let reloaded = open_project(&saved).unwrap();
    assert_eq!(engine.document_hash(), reloaded.document_hash());
}

#[test]
fn the_selection_in_the_blueprint_actually_masked_the_name_plate_fill() {
    // The name plate is filled through a marquee. If selection clipping
    // regressed, the fill would escape and flood the whole annotation
    // layer, so this is a real behavioural assertion, not a smoke test.
    let engine = open_project(BLUEPRINT).unwrap();
    let doc = engine.document().unwrap();
    let annotation = doc.layer_count() - 1;

    // Inside the plate: filled with the paper colour.
    assert_eq!(doc.get_pixel_on(annotation, 90, 85).unwrap(), [0x0b, 0x2f, 0x6b, 0xff]);
    // Just outside the plate on the same layer: untouched.
    assert_eq!(doc.get_pixel_on(annotation, 70, 85).unwrap(), [0, 0, 0, 0]);
    assert_eq!(doc.get_pixel_on(annotation, 90, 70).unwrap(), [0, 0, 0, 0]);
    // And no selection is left active for whoever opens the file.
    assert_eq!(engine.selection(), None);
}

#[test]
fn hiding_the_grid_layer_visibly_changes_the_composite() {
    // Proves the layers are load-bearing rather than cosmetic.
    let engine = open_project(BLUEPRINT).unwrap();
    let with_grid = engine.document().unwrap().composite();

    let mut hidden = engine.clone();
    hidden.execute(pixelcad_core::Command::LayerVisible { index: 1, value: false }).unwrap();
    let without_grid = hidden.document().unwrap().composite();

    assert_ne!(with_grid, without_grid, "the grid layer must contribute to the image");
}
