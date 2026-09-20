//! AC18: `docs/samples/paint_parity.pxcproj` is the Phase 1.5 dogfood
//! artefact — a tool sampler that must keep exercising every command the
//! phase added, so the sample doubles as executable documentation.

use std::collections::BTreeSet;

use pixelcad_core::{open_project, parse_project, Engine, PROJECT_FORMAT_VERSION};

const SAMPLE: &str = include_str!("../../../docs/samples/paint_parity.pxcproj");

#[test]
fn the_sampler_is_a_current_version_project_with_four_layers() {
    let project = parse_project(SAMPLE).expect("paint_parity.pxcproj must parse");
    assert_eq!(project.version, PROJECT_FORMAT_VERSION);

    let engine = open_project(SAMPLE).expect("must replay");
    let doc = engine.document().expect("must create a canvas");
    assert_eq!((doc.width(), doc.height()), (160, 120));
    assert_eq!(
        doc.layers().iter().map(|l| l.name()).collect::<Vec<_>>(),
        vec!["Sheet", "Shapes", "Transformed", "Labels"]
    );
}

#[test]
fn the_sampler_exercises_every_phase_1_5_command() {
    let project = parse_project(SAMPLE).unwrap();
    let used: BTreeSet<&str> = project.commands.iter().map(|c| c.name()).collect();

    for required in [
        "ellipse.draw",
        "polygon.draw",
        "arrow.draw",
        "polyline.draw",
        "text.draw",
        "select.lasso",
        "selection.flip",
        "selection.rotate",
        "selection.move",
        "layer.duplicate",
        "layer.merge",
        "brush.stroke",
        "fill.bucket",
        "rect.draw",
    ] {
        assert!(
            used.contains(required),
            "the sampler no longer uses {required:?}; it exists to dogfood these. Used: {used:?}"
        );
    }
}

#[test]
fn the_sampler_uses_the_new_optional_fields_rather_than_their_defaults() {
    // A sampler that only ever used defaults would pass the coverage test
    // above while proving nothing about the new parameters.
    assert!(SAMPLE.contains("shape=round"), "must demonstrate the round brush");
    assert!(SAMPLE.contains("radius=8"), "must demonstrate a rounded rectangle");
    assert!(SAMPLE.contains("tolerance=24"), "must demonstrate a tolerance fill");
    assert!(SAMPLE.contains("font=bold"), "must demonstrate the bold face");
    assert!(SAMPLE.contains("align=center"), "must demonstrate centred text");
    assert!(SAMPLE.contains("align=right"), "must demonstrate right-aligned text");
    assert!(SAMPLE.contains("fill=true") && SAMPLE.contains("fill=false"));
}

#[test]
fn the_sampler_replays_deterministically() {
    let commands = parse_project(SAMPLE).unwrap().commands;

    let mut a = Engine::new();
    a.execute_all(commands.clone()).unwrap();
    let mut b = Engine::new();
    b.execute_all(commands).unwrap();

    assert_eq!(a.document_hash(), b.document_hash());
    assert_eq!(a.document().unwrap().composite(), b.document().unwrap().composite());
}

#[test]
fn the_sampler_survives_a_save_and_reload_cycle() {
    let engine = open_project(SAMPLE).unwrap();
    let saved = pixelcad_core::serialize_project(&engine);
    let reloaded = open_project(&saved).unwrap();
    assert_eq!(engine.document_hash(), reloaded.document_hash());
}

#[test]
fn the_sampler_leaves_no_selection_active() {
    // Opening a file with a stale marquee would silently clip the next
    // thing the user draws, which is a nasty surprise to ship in a sample.
    let engine = open_project(SAMPLE).unwrap();
    assert_eq!(engine.selection(), None);
}

#[test]
fn the_sampler_actually_puts_ink_on_every_layer() {
    let engine = open_project(SAMPLE).unwrap();
    let doc = engine.document().unwrap();
    for (index, layer) in doc.layers().iter().enumerate() {
        let inked = layer.pixels().chunks(4).filter(|p| p[3] != 0).count();
        assert!(inked > 0, "layer {index} ({}) is empty", layer.name());
    }
}
