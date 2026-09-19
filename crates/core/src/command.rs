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
    fn format_round_trips_through_parse() {
        let c = [0x11, 0x22, 0x33, 0x44];
        let s = format_hex_color(c);
        assert_eq!(parse_hex_color(&s).unwrap(), c);
    }
}
