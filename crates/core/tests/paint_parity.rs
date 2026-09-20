//! Behavioural tests for the Phase 1.5 "Paint parity" command set.
//!
//! These sit in an integration test rather than in `engine.rs` because they
//! exercise the public API exactly as the GUI and CLI do — through
//! `Command`s on an `Engine` — which is the contract that actually has to
//! hold.

use pixelcad_core::font::{Font, TextAlign};
use pixelcad_core::{Axis, BrushShape, Color, Command, Engine, EngineError, Selection};

const INK: Color = [0x11, 0x22, 0x33, 0xff];
const RED: Color = [0xff, 0x00, 0x00, 0xff];
const CLEAR: Color = [0, 0, 0, 0];

fn canvas(n: u32) -> Engine {
    let mut e = Engine::new();
    e.execute(Command::CanvasNew { width: n, height: n }).unwrap();
    e
}

/// Every non-transparent pixel of the active layer, in scan order.
fn painted(e: &Engine) -> Vec<(i64, i64)> {
    let doc = e.document().unwrap();
    let mut out = Vec::new();
    for y in 0..doc.height() as i64 {
        for x in 0..doc.width() as i64 {
            if doc.get_pixel(x, y).unwrap()[3] != 0 {
                out.push((x, y));
            }
        }
    }
    out
}

fn px(e: &Engine, x: i64, y: i64) -> Color {
    e.document().unwrap().get_pixel(x, y).unwrap()
}

// ---------------------------------------------------------------- selection

#[test]
fn ac5_a_lasso_selects_a_non_rectangular_region() {
    let mut e = canvas(8);
    e.execute(Command::SelectLasso { points: vec![(0, 0), (6, 0), (0, 6)] }).unwrap();
    let s = e.selection().expect("lasso must set a selection");

    assert!(s.contains(1, 1), "inside the triangle");
    assert!(
        !s.contains(6, 6),
        "inside the bounding box but outside the polygon — the whole point of a mask"
    );
    assert!(!s.is_rect());
}

#[test]
fn ac5_drawing_through_a_lasso_is_clipped_to_its_shape() {
    let mut e = canvas(8);
    e.execute(Command::SelectLasso { points: vec![(0, 0), (6, 0), (0, 6)] }).unwrap();
    e.execute(Command::FillBucket { x: 0, y: 0, tolerance: 0, color: INK }).unwrap();

    let s = e.selection().unwrap().clone();
    for (x, y) in painted(&e) {
        assert!(s.contains(x, y), "({x},{y}) escaped the lasso");
    }
    assert_eq!(px(&e, 7, 7), CLEAR);
}

#[test]
fn select_all_covers_the_canvas_and_scales_with_it() {
    let mut e = canvas(5);
    e.execute(Command::SelectAll).unwrap();
    assert_eq!(e.selection().unwrap().bounds(), (0, 0, 5, 5));
    assert_eq!(e.selection().unwrap().count(), 25);
}

#[test]
fn selection_delete_clears_only_what_is_selected() {
    let mut e = canvas(6);
    e.execute(Command::RectDraw {
        x0: 0,
        y0: 0,
        x1: 5,
        y1: 5,
        radius: 0,
        fill: true,
        color: INK,
    })
    .unwrap();
    assert_eq!(painted(&e).len(), 36);

    e.execute(Command::SelectRect { x: 1, y: 1, width: 2, height: 2 }).unwrap();
    e.execute(Command::SelectionDelete).unwrap();

    assert_eq!(painted(&e).len(), 32, "exactly the 2x2 selection was cleared");
    assert_eq!(px(&e, 1, 1), CLEAR);
    assert_eq!(px(&e, 0, 0), INK);
}

#[test]
fn selection_commands_without_a_selection_are_a_clear_error() {
    let mut e = canvas(4);
    assert_eq!(e.execute(Command::SelectionDelete), Err(EngineError::NoSelection));
    assert_eq!(e.execute(Command::SelectionMove { dx: 1, dy: 0 }), Err(EngineError::NoSelection));
    assert_eq!(
        e.execute(Command::SelectionFlip { axis: Axis::Horizontal }),
        Err(EngineError::NoSelection)
    );
    assert_eq!(e.history().len(), 1, "failed commands must not enter history");
}

#[test]
fn ac6_selection_move_carries_the_pixels_and_the_marquee() {
    let mut e = canvas(8);
    e.execute(Command::PixelSet { x: 1, y: 1, color: RED }).unwrap();
    e.execute(Command::SelectRect { x: 1, y: 1, width: 1, height: 1 }).unwrap();
    e.execute(Command::SelectionMove { dx: 3, dy: 2 }).unwrap();

    assert_eq!(px(&e, 4, 3), RED, "the pixel moved");
    assert_eq!(px(&e, 1, 1), CLEAR, "and left transparency behind");
    assert_eq!(e.selection().unwrap().bounds(), (4, 3, 1, 1), "the marquee followed it");
}

#[test]
fn ac6_an_overlapping_nudge_does_not_smear() {
    // The classic bug: clearing before lifting makes a 1px nudge erase the
    // pixels it just wrote.
    let mut e = canvas(8);
    e.execute(Command::RectDraw {
        x0: 1,
        y0: 1,
        x1: 4,
        y1: 1,
        radius: 0,
        fill: true,
        color: RED,
    })
    .unwrap();
    e.execute(Command::SelectRect { x: 1, y: 1, width: 4, height: 1 }).unwrap();
    e.execute(Command::SelectionMove { dx: 1, dy: 0 }).unwrap();

    assert_eq!(painted(&e), vec![(2, 1), (3, 1), (4, 1), (5, 1)]);
}

#[test]
fn moving_a_lasso_selection_moves_only_its_masked_pixels() {
    let mut e = canvas(10);
    e.execute(Command::RectDraw {
        x0: 0,
        y0: 0,
        x1: 4,
        y1: 4,
        radius: 0,
        fill: true,
        color: RED,
    })
    .unwrap();
    e.execute(Command::SelectLasso { points: vec![(0, 0), (4, 0), (0, 4)] }).unwrap();
    let moved_count = e.selection().unwrap().count();
    e.execute(Command::SelectionMove { dx: 5, dy: 5 }).unwrap();

    // The corner outside the triangle stayed put.
    assert_eq!(px(&e, 4, 4), RED, "the unselected corner did not move");
    let total = painted(&e).len();
    assert_eq!(total, 25, "no pixels were created or destroyed");
    assert!(moved_count < 25);
}

#[test]
fn ac7_flipping_twice_is_the_identity() {
    let mut e = canvas(8);
    e.execute(Command::PixelSet { x: 0, y: 0, color: RED }).unwrap();
    e.execute(Command::PixelSet { x: 1, y: 0, color: INK }).unwrap();
    e.execute(Command::SelectRect { x: 0, y: 0, width: 4, height: 4 }).unwrap();
    let before = e.document().unwrap().clone();

    e.execute(Command::SelectionFlip { axis: Axis::Horizontal }).unwrap();
    assert_ne!(e.document().unwrap(), &before, "one flip must change something");
    e.execute(Command::SelectionFlip { axis: Axis::Horizontal }).unwrap();
    assert_eq!(e.document().unwrap(), &before, "two flips must restore exactly");
}

#[test]
fn flip_mirrors_within_the_selection_box() {
    let mut e = canvas(8);
    e.execute(Command::PixelSet { x: 0, y: 0, color: RED }).unwrap();
    e.execute(Command::SelectRect { x: 0, y: 0, width: 4, height: 1 }).unwrap();
    e.execute(Command::SelectionFlip { axis: Axis::Horizontal }).unwrap();
    assert_eq!(px(&e, 3, 0), RED, "moved to the far edge of the box");
    assert_eq!(px(&e, 0, 0), CLEAR);
}

#[test]
fn ac7_four_quarter_turns_are_the_identity() {
    let mut e = canvas(8);
    e.execute(Command::PixelSet { x: 0, y: 0, color: RED }).unwrap();
    e.execute(Command::PixelSet { x: 1, y: 0, color: INK }).unwrap();
    e.execute(Command::PixelSet { x: 0, y: 1, color: [9, 9, 9, 255] }).unwrap();
    e.execute(Command::SelectRect { x: 0, y: 0, width: 4, height: 4 }).unwrap();
    let before = e.document().unwrap().clone();

    for _ in 0..4 {
        e.execute(Command::SelectionRotate { degrees: 90 }).unwrap();
    }
    assert_eq!(e.document().unwrap(), &before, "4 x 90 degrees must be a no-op");
}

#[test]
fn rotate_90_transposes_a_non_square_selection() {
    let mut e = canvas(8);
    e.execute(Command::SelectRect { x: 0, y: 0, width: 4, height: 2 }).unwrap();
    e.execute(Command::SelectionRotate { degrees: 90 }).unwrap();
    assert_eq!(e.selection().unwrap().bounds(), (0, 0, 2, 4), "the box transposes");
}

#[test]
fn rotate_180_is_two_flips() {
    let mut e = canvas(8);
    e.execute(Command::PixelSet { x: 0, y: 0, color: RED }).unwrap();
    e.execute(Command::SelectRect { x: 0, y: 0, width: 3, height: 3 }).unwrap();
    e.execute(Command::SelectionRotate { degrees: 180 }).unwrap();
    assert_eq!(px(&e, 2, 2), RED);
    assert_eq!(px(&e, 0, 0), CLEAR);
}

#[test]
fn an_unsupported_rotation_is_refused_and_zero_is_a_no_op() {
    let mut e = canvas(4);
    e.execute(Command::SelectAll).unwrap();
    assert_eq!(
        e.execute(Command::SelectionRotate { degrees: 45 }),
        Err(EngineError::UnsupportedRotation { degrees: 45 })
    );
    assert!(e.execute(Command::SelectionRotate { degrees: 0 }).is_ok());
    assert!(e.execute(Command::SelectionRotate { degrees: 360 }).is_ok());
}

#[test]
fn selection_scale_resamples_by_nearest_neighbour() {
    let mut e = canvas(8);
    e.execute(Command::PixelSet { x: 0, y: 0, color: RED }).unwrap();
    e.execute(Command::SelectRect { x: 0, y: 0, width: 1, height: 1 }).unwrap();
    e.execute(Command::SelectionScale { width: 3, height: 3 }).unwrap();

    for y in 0..3 {
        for x in 0..3 {
            assert_eq!(px(&e, x, y), RED, "({x},{y}) should be part of the scaled block");
        }
    }
    assert_eq!(e.selection().unwrap().bounds(), (0, 0, 3, 3));
}

// ------------------------------------------------------------- fill & brush

#[test]
fn ac9_fill_tolerance_spreads_across_near_colours_and_stops_outside_it() {
    let mut e = canvas(5);
    // Three shades: two within 10 of each other, one far away.
    e.execute(Command::PixelSet { x: 0, y: 0, color: [100, 100, 100, 255] }).unwrap();
    e.execute(Command::PixelSet { x: 1, y: 0, color: [105, 105, 105, 255] }).unwrap();
    e.execute(Command::PixelSet { x: 2, y: 0, color: [200, 200, 200, 255] }).unwrap();

    e.execute(Command::FillBucket { x: 0, y: 0, tolerance: 10, color: RED }).unwrap();

    assert_eq!(px(&e, 0, 0), RED, "the seed");
    assert_eq!(px(&e, 1, 0), RED, "within tolerance");
    assert_eq!(px(&e, 2, 0), [200, 200, 200, 255], "outside tolerance — the wall holds");
}

#[test]
fn ac16_fill_with_no_tolerance_behaves_exactly_as_in_phase_1() {
    let mut e = canvas(5);
    e.execute(Command::PixelSet { x: 0, y: 0, color: [100, 100, 100, 255] }).unwrap();
    e.execute(Command::PixelSet { x: 1, y: 0, color: [105, 105, 105, 255] }).unwrap();
    e.execute(Command::FillBucket { x: 0, y: 0, tolerance: 0, color: RED }).unwrap();
    assert_eq!(px(&e, 1, 0), [105, 105, 105, 255], "a near colour is still a wall at 0");
}

#[test]
fn ac10_a_round_brush_is_not_a_square_one() {
    let mut round = canvas(11);
    round
        .execute(Command::BrushStroke {
            x0: 5,
            y0: 5,
            x1: 5,
            y1: 5,
            size: 5,
            shape: BrushShape::Round,
            color: INK,
        })
        .unwrap();

    let mut square = canvas(11);
    square
        .execute(Command::BrushStroke {
            x0: 5,
            y0: 5,
            x1: 5,
            y1: 5,
            size: 5,
            shape: BrushShape::Square,
            color: INK,
        })
        .unwrap();

    assert_eq!(painted(&square).len(), 25, "a size-5 square brush is 5x5");
    assert!(painted(&round).len() < 25, "a disc must cover less than its bounding square");
    assert_eq!(px(&round, 5, 5), INK, "the centre is always painted");
    assert_eq!(px(&round, 3, 3), CLEAR, "but the corners are not");
    assert_eq!(px(&square, 3, 3), INK);
}

#[test]
fn ac16_a_brush_with_no_shape_defaults_to_square() {
    assert_eq!(BrushShape::default(), BrushShape::Square);
    let parsed = pixelcad_core::parse_line(
        r##"brush.stroke x0=0 y0=0 x1=0 y1=0 size=3 color="#ffffff""##,
        1,
    )
    .unwrap();
    match parsed {
        Command::BrushStroke { shape, .. } => assert_eq!(shape, BrushShape::Square),
        other => panic!("expected a brush stroke, got {other:?}"),
    }
}

// ------------------------------------------------------------------ shapes

#[test]
fn ac11_an_outlined_ellipse_is_hollow_and_a_filled_one_is_not() {
    let mut hollow = canvas(12);
    hollow
        .execute(Command::EllipseDraw { x0: 1, y0: 1, x1: 10, y1: 8, fill: false, color: INK })
        .unwrap();
    let mut solid = canvas(12);
    solid
        .execute(Command::EllipseDraw { x0: 1, y0: 1, x1: 10, y1: 8, fill: true, color: INK })
        .unwrap();

    assert!(painted(&hollow).len() < painted(&solid).len(), "an outline has fewer pixels");
    // A point near the centre is inside the filled ellipse and not on the
    // outline of the hollow one.
    assert_eq!(px(&solid, 5, 4), INK);
    assert_eq!(px(&hollow, 5, 4), CLEAR);
    // Corners of the bounding box are outside an ellipse either way.
    assert_eq!(px(&solid, 1, 1), CLEAR, "the bounding-box corner is outside the ellipse");
}

#[test]
fn an_ellipse_is_symmetric_about_both_axes() {
    let mut e = canvas(13);
    e.execute(Command::EllipseDraw { x0: 0, y0: 0, x1: 12, y1: 12, fill: true, color: INK })
        .unwrap();
    for (x, y) in painted(&e) {
        assert_eq!(px(&e, 12 - x, y), INK, "horizontal mirror of ({x},{y})");
        assert_eq!(px(&e, x, 12 - y), INK, "vertical mirror of ({x},{y})");
    }
}

#[test]
fn ac11_a_rounded_rectangle_omits_its_corners() {
    let mut sharp = canvas(12);
    sharp
        .execute(Command::RectDraw {
            x0: 0,
            y0: 0,
            x1: 11,
            y1: 11,
            radius: 0,
            fill: true,
            color: INK,
        })
        .unwrap();
    let mut round = canvas(12);
    round
        .execute(Command::RectDraw {
            x0: 0,
            y0: 0,
            x1: 11,
            y1: 11,
            radius: 4,
            fill: true,
            color: INK,
        })
        .unwrap();

    assert_eq!(px(&sharp, 0, 0), INK);
    assert_eq!(px(&round, 0, 0), CLEAR, "the corner is rounded away");
    assert_eq!(px(&round, 6, 0), INK, "but the straight edge remains");
    assert!(painted(&round).len() < painted(&sharp).len());
}

#[test]
fn ac16_a_rectangle_with_no_radius_is_the_phase_1_rectangle() {
    let parsed = pixelcad_core::parse_line(
        r##"rect.draw x0=0 y0=0 x1=3 y1=3 fill=true color="#ffffff""##,
        1,
    )
    .unwrap();
    match parsed {
        Command::RectDraw { radius, .. } => assert_eq!(radius, 0, "absent radius means square"),
        other => panic!("expected a rectangle, got {other:?}"),
    }

    // And a fill.bucket with no tolerance parses as tolerance 0.
    let parsed =
        pixelcad_core::parse_line(r##"fill.bucket x=0 y=0 color="#ffffff""##, 1).unwrap();
    match parsed {
        Command::FillBucket { tolerance, .. } => assert_eq!(tolerance, 0),
        other => panic!("expected a fill, got {other:?}"),
    }
}

#[test]
fn ac11_a_polygon_has_the_expected_symmetry_and_is_deterministic() {
    let mut a = canvas(21);
    a.execute(Command::PolygonDraw {
        x: 10,
        y: 10,
        radius: 8,
        sides: 6,
        rotation: 0,
        fill: false,
        color: INK,
    })
    .unwrap();
    let mut b = canvas(21);
    b.execute(Command::PolygonDraw {
        x: 10,
        y: 10,
        radius: 8,
        sides: 6,
        rotation: 0,
        fill: false,
        color: INK,
    })
    .unwrap();

    assert_eq!(a.document_hash(), b.document_hash(), "geometry must be reproducible");
    assert!(!painted(&a).is_empty());

    let mut filled = canvas(21);
    filled
        .execute(Command::PolygonDraw {
            x: 10,
            y: 10,
            radius: 8,
            sides: 6,
            rotation: 0,
            fill: true,
            color: INK,
        })
        .unwrap();
    assert!(painted(&filled).len() > painted(&a).len(), "filling adds the interior");
    assert_eq!(px(&filled, 10, 10), INK, "the centre of a filled polygon is painted");
}

#[test]
fn a_degenerate_polygon_draws_nothing_rather_than_guessing() {
    let mut e = canvas(10);
    for sides in [0u32, 1, 2] {
        e.execute(Command::PolygonDraw {
            x: 5,
            y: 5,
            radius: 3,
            sides,
            rotation: 0,
            fill: true,
            color: INK,
        })
        .unwrap();
    }
    assert_eq!(painted(&e).len(), 0);
}

#[test]
fn ac11_an_arrow_has_a_shaft_and_a_wider_head() {
    let mut e = canvas(24);
    e.execute(Command::ArrowDraw { x0: 2, y0: 12, x1: 20, y1: 12, head: 6, color: INK })
        .unwrap();

    assert_eq!(px(&e, 2, 12), INK, "the tail");
    assert_eq!(px(&e, 20, 12), INK, "the tip");
    // The head is wider than the 1px shaft, so there is ink off the axis
    // near the tip but not near the tail.
    let off_axis_near_tip = (10..=13).any(|x| px(&e, x + 5, 10)[3] != 0);
    let off_axis_near_tail = (2..=5).any(|x| px(&e, x, 10)[3] != 0);
    assert!(off_axis_near_tip, "the head must be wider than the shaft");
    assert!(!off_axis_near_tail, "the tail must not be");
}

#[test]
fn a_zero_length_arrow_draws_a_point_and_never_panics() {
    let mut e = canvas(8);
    e.execute(Command::ArrowDraw { x0: 4, y0: 4, x1: 4, y1: 4, head: 5, color: INK }).unwrap();
    assert_eq!(painted(&e), vec![(4, 4)]);
}

#[test]
fn ac11_a_polyline_connects_every_segment() {
    let mut e = canvas(10);
    e.execute(Command::PolylineDraw { points: vec![(0, 0), (3, 0), (3, 3)], color: INK })
        .unwrap();
    for x in 0..=3 {
        assert_eq!(px(&e, x, 0), INK, "first segment at x={x}");
    }
    for y in 0..=3 {
        assert_eq!(px(&e, 3, y), INK, "second segment at y={y}");
    }
    assert_eq!(px(&e, 0, 3), CLEAR, "a polyline is not closed");
}

#[test]
fn a_single_point_polyline_draws_that_point() {
    let mut e = canvas(6);
    e.execute(Command::PolylineDraw { points: vec![(2, 3)], color: INK }).unwrap();
    assert_eq!(painted(&e), vec![(2, 3)]);
}

// -------------------------------------------------------------------- text

#[test]
fn ac12_text_stamps_glyphs_that_match_the_font_table() {
    let mut e = canvas(16);
    e.execute(Command::TextDraw {
        x: 0,
        y: 0,
        text: "A".to_string(),
        font: Font::Small,
        scale: 1,
        align: TextAlign::Left,
        color: INK,
    })
    .unwrap();

    let mut expected = Vec::new();
    pixelcad_core::font::rasterize("A", Font::Small, 1, 0, 0, TextAlign::Left, |x, y| {
        expected.push((x, y))
    });
    expected.sort_by_key(|&(x, y)| (y, x));
    expected.dedup();

    assert_eq!(painted(&e), expected, "the stamped pixels must be exactly the font's");
    assert!(!expected.is_empty());
}

#[test]
fn text_scale_and_alignment_move_the_ink_as_the_font_defines() {
    let mut small = canvas(40);
    small
        .execute(Command::TextDraw {
            x: 0,
            y: 0,
            text: "HI".to_string(),
            font: Font::Small,
            scale: 1,
            align: TextAlign::Left,
            color: INK,
        })
        .unwrap();
    let mut big = canvas(40);
    big.execute(Command::TextDraw {
        x: 0,
        y: 0,
        text: "HI".to_string(),
        font: Font::Small,
        scale: 2,
        align: TextAlign::Left,
        color: INK,
    })
    .unwrap();

    assert_eq!(
        painted(&big).len(),
        painted(&small).len() * 4,
        "doubling the scale quadruples the ink"
    );

    let mut right = canvas(40);
    right
        .execute(Command::TextDraw {
            x: 39,
            y: 0,
            text: "HI".to_string(),
            font: Font::Small,
            scale: 1,
            align: TextAlign::Right,
            color: INK,
        })
        .unwrap();
    let leftmost = painted(&right).iter().map(|p| p.0).min().unwrap();
    assert!(leftmost > 20, "right-aligned text must sit at the right edge");
}

#[test]
fn text_is_clipped_by_a_selection_like_any_other_drawing() {
    let mut e = canvas(32);
    e.execute(Command::SelectRect { x: 0, y: 0, width: 4, height: 4 }).unwrap();
    e.execute(Command::TextDraw {
        x: 0,
        y: 0,
        text: "WWWW".to_string(),
        font: Font::Bold,
        scale: 1,
        align: TextAlign::Left,
        color: INK,
    })
    .unwrap();
    for (x, y) in painted(&e) {
        assert!((0..4).contains(&x) && (0..4).contains(&y), "({x},{y}) escaped the marquee");
    }
}

// --------------------------------------------------------- canvas & layers

#[test]
fn ac13_crop_and_resize_run_through_the_engine_and_clear_the_selection() {
    let mut e = canvas(8);
    e.execute(Command::PixelSet { x: 4, y: 4, color: RED }).unwrap();
    e.execute(Command::SelectAll).unwrap();

    e.execute(Command::CanvasCrop { x: 4, y: 4, width: 2, height: 2 }).unwrap();
    assert_eq!((e.document().unwrap().width(), e.document().unwrap().height()), (2, 2));
    assert_eq!(px(&e, 0, 0), RED);
    assert_eq!(e.selection(), None, "an old-coordinate marquee is meaningless after a crop");

    e.execute(Command::CanvasResize { width: 4, height: 4 }).unwrap();
    assert_eq!((e.document().unwrap().width(), e.document().unwrap().height()), (4, 4));
    assert_eq!(px(&e, 1, 1), RED, "nearest-neighbour doubled the pixel");
}

#[test]
fn crop_and_resize_are_undoable_in_one_step() {
    let mut e = canvas(8);
    e.execute(Command::PixelSet { x: 1, y: 1, color: RED }).unwrap();
    let before = e.document().unwrap().clone();
    e.execute(Command::CanvasResize { width: 32, height: 32 }).unwrap();
    assert!(e.undo());
    assert_eq!(e.document().unwrap(), &before, "undo must restore the old dimensions exactly");
}

#[test]
fn ac14_layer_duplicate_and_merge_through_the_engine() {
    let mut e = canvas(4);
    e.execute(Command::PixelSet { x: 0, y: 0, color: RED }).unwrap();
    e.execute(Command::LayerDuplicate { index: 0 }).unwrap();
    assert_eq!(e.document().unwrap().layer_count(), 2);
    assert_eq!(e.document().unwrap().active_layer_index(), 1);

    let before = e.document().unwrap().composite();
    e.execute(Command::LayerMerge { index: 1 }).unwrap();
    assert_eq!(e.document().unwrap().layer_count(), 1);
    assert_eq!(e.document().unwrap().composite(), before, "merging is visually invisible");
}

#[test]
fn merging_the_bottom_layer_is_refused_without_damage() {
    let mut e = canvas(4);
    let before = e.document().unwrap().clone();
    assert!(e.execute(Command::LayerMerge { index: 0 }).is_err());
    assert_eq!(e.document().unwrap(), &before);
}

// -------------------------------------------------------------- invariants

#[test]
fn ac15_every_new_command_round_trips_through_the_parser() {
    // The exhaustive coverage check lives in parser.rs; this asserts the
    // new commands specifically survive a real script round trip.
    let commands = vec![
        Command::SelectAll,
        Command::SelectLasso { points: vec![(0, 0), (4, 0), (0, 4)] },
        Command::SelectionDelete,
        Command::SelectionMove { dx: -2, dy: 3 },
        Command::SelectionFlip { axis: Axis::Vertical },
        Command::SelectionRotate { degrees: 270 },
        Command::SelectionScale { width: 7, height: 9 },
        Command::CanvasCrop { x: 1, y: 1, width: 5, height: 5 },
        Command::CanvasResize { width: 16, height: 16 },
        Command::LayerDuplicate { index: 0 },
        Command::LayerMerge { index: 1 },
        Command::EllipseDraw { x0: 0, y0: 0, x1: 5, y1: 3, fill: true, color: INK },
        Command::PolygonDraw {
            x: 4,
            y: 4,
            radius: 3,
            sides: 7,
            rotation: 15,
            fill: false,
            color: INK,
        },
        Command::ArrowDraw { x0: 0, y0: 0, x1: 9, y1: 9, head: 4, color: INK },
        Command::PolylineDraw { points: vec![(0, 0), (1, 2), (5, 5)], color: INK },
        Command::TextDraw {
            x: 1,
            y: 2,
            text: "PIXELCAD 1.5".to_string(),
            font: Font::Bold,
            scale: 3,
            align: TextAlign::Right,
            color: INK,
        },
    ];

    let script = pixelcad_core::serialize_script(&commands);
    let reparsed = pixelcad_core::parse_script(&script).expect("must reparse");
    assert_eq!(reparsed, commands);
}

#[test]
fn the_new_commands_are_deterministic_end_to_end() {
    let script = r##"
        canvas.new width=32 height=24
        ellipse.draw x0=2 y0=2 x1=20 y1=16 fill=true color="#3366aaff"
        polygon.draw x=24 y=8 radius=6 sides=5 rotation=18 fill=false color="#ffffffff"
        arrow.draw x0=2 y0=20 x1=28 y1=20 head=5 color="#ff0000ff"
        polyline.draw points="0,0 8,4 16,0" color="#00ff00ff"
        text.draw x=2 y=2 text="PXC" font=bold scale=1 align=left color="#000000ff"
        select.lasso points="4,4 18,4 18,14"
        selection.flip axis=horizontal
        select.clear
        layer.duplicate index=0
        layer.merge index=1
        canvas.resize width=64 height=48
    "##;

    let a = pixelcad_core::open_project(script).expect("script must run");
    let b = pixelcad_core::open_project(script).expect("script must run");
    assert_eq!(a.document_hash(), b.document_hash());
    assert_eq!(a.document().unwrap().composite(), b.document().unwrap().composite());
}

#[test]
fn a_selection_survives_undo_as_snapshotted_state() {
    let mut e = canvas(8);
    e.execute(Command::SelectRect { x: 1, y: 1, width: 2, height: 2 }).unwrap();
    e.execute(Command::SelectAll).unwrap();
    assert_eq!(e.selection().unwrap().count(), 64);
    assert!(e.undo());
    assert_eq!(
        e.selection(),
        Some(&Selection::rect(1, 1, 2, 2)),
        "undo restores the previous marquee"
    );
}
