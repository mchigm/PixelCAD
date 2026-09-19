//! The typed command vocabulary.
//!
//! Every user-visible action — in the GUI or the CLI — is represented as a
//! `Command` value. Commands are the only way to mutate a [`crate::Document`]
//! through the [`crate::engine::Engine`], and they are the unit of
//! serialization for `.pxc` scripts. There is no stringly-typed execution
//! path: parsing happens once, at the text/struct boundary, in
//! [`crate::parser`].

use crate::document::Color;

/// A single typed, replayable action against a [`crate::Document`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// `canvas.new width=<u32> height=<u32>` — (re)create the document with
    /// the given dimensions, discarding any existing content.
    CanvasNew { width: u32, height: u32 },
    /// `pixel.set x=<i64> y=<i64> color=<hex>` — set a single pixel.
    PixelSet { x: i64, y: i64, color: Color },
    /// `line.draw x0=<i64> y0=<i64> x1=<i64> y1=<i64> color=<hex>` — draw a
    /// straight line with Bresenham's algorithm.
    LineDraw { x0: i64, y0: i64, x1: i64, y1: i64, color: Color },
    /// `palette.set index=<u8> color=<hex>` — assign a color to a palette
    /// slot. Purely metadata: it does not touch pixel content, but it is
    /// still a document-level fact worth recording and replaying (e.g. so a
    /// `.pxc` script fully reproduces the GUI's palette state).
    PaletteSet { index: u8, color: Color },

    // ---------------------------------------------------------- layers (P1)
    /// `layer.add name="<text>"` — insert a new transparent layer above the
    /// active one and make it active.
    LayerAdd { name: String },
    /// `layer.select index=<usize>` — change the active layer.
    LayerSelect { index: usize },
    /// `layer.remove index=<usize>` — delete a layer (never the last one).
    LayerRemove { index: usize },
    /// `layer.rename index=<usize> name="<text>"`.
    LayerRename { index: usize, name: String },
    /// `layer.move from=<usize> to=<usize>` — reorder the stack.
    LayerMove { from: usize, to: usize },
    /// `layer.visible index=<usize> value=<bool>`.
    LayerVisible { index: usize, value: bool },
    /// `layer.opacity index=<usize> value=<u8>`.
    LayerOpacity { index: usize, value: u8 },

    // ------------------------------------------- colour & selection (P1)
    /// `color.set color=<hex>` — set the engine's active drawing colour.
    /// Recorded so a GUI session replays with the same colour state; the
    /// drawing commands themselves still carry an explicit colour, so a
    /// script remains readable without tracking this.
    ColorSet { color: Color },
    /// `color.pick x=<i64> y=<i64>` — the eyedropper: read the *composited*
    /// pixel at `(x, y)` into the active colour.
    ColorPick { x: i64, y: i64 },
    /// `select.rect x=<i64> y=<i64> width=<u32> height=<u32>` — constrain
    /// every subsequent pixel write to this rectangle.
    SelectRect { x: i64, y: i64, width: u32, height: u32 },
    /// `select.clear` — drop the selection; writes are bounded only by the
    /// canvas again.
    SelectClear,

    // ----------------------------------------------------- drawing (P1)
    /// `brush.stroke x0=<i64> y0=<i64> x1=<i64> y1=<i64> size=<u32>
    /// color=<hex>` — sweep a square brush of side `size` along the line
    /// from `(x0, y0)` to `(x1, y1)`.
    ///
    /// **The eraser is this command with `color="#00000000"`.** Pixel writes
    /// replace rather than blend, so painting fully transparent *is*
    /// erasing; a separate `eraser.*` command would be the same code with a
    /// different name.
    BrushStroke { x0: i64, y0: i64, x1: i64, y1: i64, size: u32, color: Color },
    /// `rect.draw x0=<i64> y0=<i64> x1=<i64> y1=<i64> fill=<bool>
    /// color=<hex>` — an axis-aligned rectangle, outlined or filled.
    /// Corners may be given in any order.
    RectDraw { x0: i64, y0: i64, x1: i64, y1: i64, fill: bool, color: Color },
    /// `fill.bucket x=<i64> y=<i64> color=<hex>` — 4-connected flood fill on
    /// the active layer, replacing the contiguous region that exactly
    /// matches the starting pixel's RGBA.
    FillBucket { x: i64, y: i64, color: Color },
}

impl Command {
    /// The namespaced command name, as used in `.pxc` text (e.g.
    /// `"pixel.set"`).
    pub fn name(&self) -> &'static str {
        match self {
            Command::CanvasNew { .. } => "canvas.new",
            Command::PixelSet { .. } => "pixel.set",
            Command::LineDraw { .. } => "line.draw",
            Command::PaletteSet { .. } => "palette.set",
            Command::LayerAdd { .. } => "layer.add",
            Command::LayerSelect { .. } => "layer.select",
            Command::LayerRemove { .. } => "layer.remove",
            Command::LayerRename { .. } => "layer.rename",
            Command::LayerMove { .. } => "layer.move",
            Command::LayerVisible { .. } => "layer.visible",
            Command::LayerOpacity { .. } => "layer.opacity",
            Command::ColorSet { .. } => "color.set",
            Command::ColorPick { .. } => "color.pick",
            Command::SelectRect { .. } => "select.rect",
            Command::SelectClear => "select.clear",
            Command::BrushStroke { .. } => "brush.stroke",
            Command::RectDraw { .. } => "rect.draw",
            Command::FillBucket { .. } => "fill.bucket",
        }
    }
}

/// Parses a `#rrggbb` or `#rrggbbaa` hex color string into an RGBA8 [`Color`].
/// Missing alpha defaults to `0xff` (fully opaque).
pub fn parse_hex_color(s: &str) -> Result<Color, String> {
    let s = s.strip_prefix('#').unwrap_or(s);
    let bytes = match s.len() {
        6 => {
            let r = u8::from_str_radix(&s[0..2], 16).map_err(|e| e.to_string())?;
            let g = u8::from_str_radix(&s[2..4], 16).map_err(|e| e.to_string())?;
            let b = u8::from_str_radix(&s[4..6], 16).map_err(|e| e.to_string())?;
            [r, g, b, 0xff]
        }
        8 => {
            let r = u8::from_str_radix(&s[0..2], 16).map_err(|e| e.to_string())?;
            let g = u8::from_str_radix(&s[2..4], 16).map_err(|e| e.to_string())?;
            let b = u8::from_str_radix(&s[4..6], 16).map_err(|e| e.to_string())?;
            let a = u8::from_str_radix(&s[6..8], 16).map_err(|e| e.to_string())?;
            [r, g, b, a]
        }
        _ => return Err(format!("expected #rrggbb or #rrggbbaa, got {:?}", s)),
    };
    Ok(bytes)
}

/// Formats a [`Color`] as `#rrggbbaa` hex text, matching what
/// [`parse_hex_color`] accepts.
pub fn format_hex_color(c: Color) -> String {
    format!("#{:02x}{:02x}{:02x}{:02x}", c[0], c[1], c[2], c[3])
}

/// Makes an arbitrary string safe to write as a quoted `.pxc` value.
///
/// The `.pxc` grammar has no escape sequences by design (it is meant to stay
/// trivially hand-editable), so a layer name containing a double quote or a
/// newline could not survive a round trip. Rather than inventing escaping,
/// such characters are replaced with a space at the point a name enters the
/// engine *and* at the point it is serialized — so both paths agree and the
/// document never holds a name it could not write back out.
pub fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| if c == '"' || c.is_control() { ' ' } else { c })
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rgb_hex_with_default_alpha() {
        assert_eq!(parse_hex_color("#1d1d1f").unwrap(), [0x1d, 0x1d, 0x1f, 0xff]);
    }

    #[test]
    fn parses_rgba_hex() {
        assert_eq!(parse_hex_color("#1d1d1f80").unwrap(), [0x1d, 0x1d, 0x1f, 0x80]);
    }

    #[test]
    fn rejects_bad_length() {
        assert!(parse_hex_color("#abc").is_err());
    }

    #[test]
    fn sanitize_name_strips_unrepresentable_characters() {
        assert_eq!(sanitize_name("hull \"outer\""), "hull  outer");
        assert_eq!(sanitize_name("a\nb"), "a b");
        assert_eq!(sanitize_name("  padded  "), "padded");
        assert_eq!(sanitize_name("Rigging 2"), "Rigging 2", "ordinary names are untouched");
    }

    #[test]
    fn format_round_trips_through_parse() {
        let c = [0x11, 0x22, 0x33, 0x44];
        let s = format_hex_color(c);
        assert_eq!(parse_hex_color(&s).unwrap(), c);
    }
}
