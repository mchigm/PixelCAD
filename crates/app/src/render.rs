//! Nearest-neighbour upscaling of a [`Document`] into a display buffer, with
//! an optional pixel-grid overlay.
//!
//! Deliberately kept independent of Slint: it is a pure function from
//! `(Document, zoom, draw_grid)` to `(width_px, height_px, rgba_bytes)`, so
//! it can be unit-tested without a window or event loop. `main.rs` wraps the
//! result in a `slint::Image` for display.

use pixelcad_core::Document;

/// Zoom level at or above which the pixel grid overlay is drawn.
pub const GRID_ZOOM_THRESHOLD: u32 = 8;

/// Grid line color: a semi-transparent dark gray, alpha-composited over the
/// scaled document pixel underneath it.
const GRID_LINE_COLOR: [u8; 4] = [0, 0, 0, 96];

/// Renders `document` at integer `zoom` (each document pixel becomes a
/// `zoom x zoom` block), overlaying a 1px grid at cell boundaries when
/// `zoom >= GRID_ZOOM_THRESHOLD`. Returns `(display_width, display_height,
/// rgba8_bytes)`.
///
/// `zoom` is clamped to a minimum of 1 (a zoom of 0 would produce an empty
/// buffer).
pub fn render_display_buffer(document: &Document, zoom: u32, draw_grid: bool) -> (u32, u32, Vec<u8>) {
    let zoom = zoom.max(1);
    let src_w = document.width();
    let src_h = document.height();
    let dst_w = src_w * zoom;
    let dst_h = src_h * zoom;
    // The viewport shows the flattened stack, exactly what the CLI exports.
    let src = document.composite();

    let mut out = vec![0u8; dst_w as usize * dst_h as usize * 4];
    let grid_active = draw_grid && zoom >= GRID_ZOOM_THRESHOLD;

    for dy in 0..dst_h {
        let sy = dy / zoom;
        for dx in 0..dst_w {
            let sx = dx / zoom;
            let src_i = (sy as usize * src_w as usize + sx as usize) * 4;
            let mut pixel = [src[src_i], src[src_i + 1], src[src_i + 2], src[src_i + 3]];

            if grid_active && (dx % zoom == 0 || dy % zoom == 0) {
                pixel = alpha_over(GRID_LINE_COLOR, pixel);
            }

            let dst_i = (dy as usize * dst_w as usize + dx as usize) * 4;
            out[dst_i..dst_i + 4].copy_from_slice(&pixel);
        }
    }

    (dst_w, dst_h, out)
}

/// Standard "over" alpha compositing of `fg` onto `bg`, both straight
/// (non-premultiplied) RGBA8. Order of composition is fixed and pixels are
/// computed independently, so this introduces no iteration-order-dependent
/// behaviour.
fn alpha_over(fg: [u8; 4], bg: [u8; 4]) -> [u8; 4] {
    let fa = fg[3] as f32 / 255.0;
    let ba = bg[3] as f32 / 255.0;
    let out_a = fa + ba * (1.0 - fa);
    if out_a <= 0.0 {
        return [0, 0, 0, 0];
    }
    let blend = |f: u8, b: u8| -> u8 {
        let f = f as f32 / 255.0;
        let b = b as f32 / 255.0;
        let v = (f * fa + b * ba * (1.0 - fa)) / out_a;
        (v * 255.0).round().clamp(0.0, 255.0) as u8
    };
    [blend(fg[0], bg[0]), blend(fg[1], bg[1]), blend(fg[2], bg[2]), (out_a * 255.0).round() as u8]
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixelcad_core::{Document, Engine};

    fn small_doc() -> Document {
        let mut engine = Engine::new();
        engine
            .execute(pixelcad_core::Command::CanvasNew { width: 2, height: 2 })
            .unwrap();
        engine
            .execute(pixelcad_core::Command::PixelSet { x: 0, y: 0, color: [255, 0, 0, 255] })
            .unwrap();
        engine.document().unwrap().clone()
    }

    #[test]
    fn zoom_one_is_identity_and_never_draws_grid() {
        let doc = small_doc();
        let (w, h, buf) = render_display_buffer(&doc, 1, true);
        assert_eq!((w, h), (2, 2));
        assert_eq!(buf, doc.composite());
    }

    #[test]
    fn renderer_shows_the_composited_stack_not_the_active_layer() {
        let mut engine = Engine::new();
        engine.execute(pixelcad_core::Command::CanvasNew { width: 1, height: 1 }).unwrap();
        engine
            .execute(pixelcad_core::Command::PixelSet { x: 0, y: 0, color: [255, 0, 0, 255] })
            .unwrap();
        // Add an empty layer on top and make it active: the renderer must
        // still show the red pixel underneath.
        engine.execute(pixelcad_core::Command::LayerAdd { name: "top".into() }).unwrap();
        let doc = engine.document().unwrap();
        let (_, _, buf) = render_display_buffer(doc, 1, false);
        assert_eq!(&buf[0..4], &[255, 0, 0, 255]);
    }

    #[test]
    fn zoom_scales_each_pixel_into_a_block() {
        let doc = small_doc();
        let (w, h, buf) = render_display_buffer(&doc, 4, false);
        assert_eq!((w, h), (8, 8));
        // Every pixel in the top-left 4x4 block must be the red source pixel.
        for y in 0..4 {
            for x in 0..4 {
                let i = (y * w as usize + x) * 4;
                assert_eq!(&buf[i..i + 4], &[255, 0, 0, 255]);
            }
        }
    }

    #[test]
    fn grid_only_drawn_at_or_above_threshold() {
        let doc = small_doc();
        let (_, _, below) = render_display_buffer(&doc, GRID_ZOOM_THRESHOLD - 1, true);
        let (_, _, below_no_grid) = render_display_buffer(&doc, GRID_ZOOM_THRESHOLD - 1, false);
        assert_eq!(below, below_no_grid, "grid must not render below the threshold");

        let (w, _, at_threshold) = render_display_buffer(&doc, GRID_ZOOM_THRESHOLD, true);
        // The top-left pixel (0,0) is always a grid line intersection.
        let top_left = &at_threshold[0..4];
        assert_ne!(top_left, &[255, 0, 0, 255], "grid line must alter the boundary pixel");
        // A pixel strictly inside the first cell (not on any grid line) keeps
        // the source color exactly.
        let inside_i = (1 * w as usize + 1) * 4;
        assert_eq!(&at_threshold[inside_i..inside_i + 4], &[255, 0, 0, 255]);
    }

    #[test]
    fn zoom_zero_is_clamped_to_one() {
        let doc = small_doc();
        let (w, h, _) = render_display_buffer(&doc, 0, true);
        assert_eq!((w, h), (2, 2));
    }
}
