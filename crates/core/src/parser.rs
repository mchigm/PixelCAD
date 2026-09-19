//! Text <-> [`Command`] conversion: the `.pxc` script format.
//!
//! Grammar, one command per line:
//!
//! ```text
//! <namespace>.<verb> key=value key=value ...
//! ```
//!
//! - Blank lines and lines whose first non-whitespace character is `#` are
//!   comments and are skipped.
//! - Values containing whitespace must be double-quoted (colors are always
//!   written quoted by convention, e.g. `color="#1d1d1f"`, though the quotes
//!   are optional for values with no whitespace).
//! - Numeric fields are parsed with Rust's standard integer parsing; no
//!   locale-dependent or platform-dependent behaviour is involved anywhere
//!   in this module, which keeps parsing deterministic.
//!
//! This is the *only* place `.pxc` text is interpreted. Once a line becomes
//! a [`Command`], nothing downstream ever re-inspects strings to decide
//! behaviour.

use std::collections::HashMap;
use std::fmt;

use crate::command::{format_hex_color, parse_hex_color, sanitize_name, Command};

/// An error produced while parsing a `.pxc` script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub line_number: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line_number, self.message)
    }
}

impl std::error::Error for ParseError {}

/// Splits a line into whitespace-separated tokens, treating
/// double-quoted spans as a single token (quotes are stripped from the
/// returned text but preserved verbatim inside).
fn tokenize(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut has_content = false;

    for ch in line.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                has_content = true;
            }
            c if c.is_whitespace() && !in_quotes => {
                if has_content {
                    tokens.push(std::mem::take(&mut current));
                    has_content = false;
                }
            }
            c => {
                current.push(c);
                has_content = true;
            }
        }
    }
    if has_content {
        tokens.push(current);
    }
    tokens
}

/// Splits a `key=value` token into its parts.
fn split_kv<'a>(token: &'a str, line_number: usize) -> Result<(&'a str, &'a str), ParseError> {
    token.split_once('=').ok_or_else(|| ParseError {
        line_number,
        message: format!("expected key=value, got {:?}", token),
    })
}

fn require<'a>(
    fields: &'a HashMap<&str, &str>,
    key: &str,
    line_number: usize,
) -> Result<&'a str, ParseError> {
    fields.get(key).copied().ok_or_else(|| ParseError {
        line_number,
        message: format!("missing required field {:?}", key),
    })
}

fn parse_u32(value: &str, key: &str, line_number: usize) -> Result<u32, ParseError> {
    value.parse::<u32>().map_err(|e| ParseError {
        line_number,
        message: format!("field {:?} = {:?} is not a valid u32: {}", key, value, e),
    })
}

fn parse_u8(value: &str, key: &str, line_number: usize) -> Result<u8, ParseError> {
    value.parse::<u8>().map_err(|e| ParseError {
        line_number,
        message: format!("field {:?} = {:?} is not a valid u8: {}", key, value, e),
    })
}

fn parse_i64(value: &str, key: &str, line_number: usize) -> Result<i64, ParseError> {
    value.parse::<i64>().map_err(|e| ParseError {
        line_number,
        message: format!("field {:?} = {:?} is not a valid i64: {}", key, value, e),
    })
}

fn parse_usize(value: &str, key: &str, line_number: usize) -> Result<usize, ParseError> {
    value.parse::<usize>().map_err(|e| ParseError {
        line_number,
        message: format!("field {:?} = {:?} is not a valid index: {}", key, value, e),
    })
}

fn parse_bool(value: &str, key: &str, line_number: usize) -> Result<bool, ParseError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(ParseError {
            line_number,
            message: format!("field {:?} = {:?} must be true or false", key, other),
        }),
    }
}

fn parse_color(value: &str, key: &str, line_number: usize) -> Result<[u8; 4], ParseError> {
    parse_hex_color(value).map_err(|e| ParseError {
        line_number,
        message: format!("field {:?} = {:?} is not a valid color: {}", key, value, e),
    })
}

/// Parses a single non-empty, non-comment line into a [`Command`].
pub fn parse_line(line: &str, line_number: usize) -> Result<Command, ParseError> {
    let tokens = tokenize(line);
    let (name, rest) = tokens.split_first().ok_or_else(|| ParseError {
        line_number,
        message: "empty line".to_string(),
    })?;

    let mut fields: HashMap<&str, &str> = HashMap::new();
    for token in rest {
        let (k, v) = split_kv(token, line_number)?;
        fields.insert(k, v);
    }

    match name.as_str() {
        "canvas.new" => Ok(Command::CanvasNew {
            width: parse_u32(require(&fields, "width", line_number)?, "width", line_number)?,
            height: parse_u32(require(&fields, "height", line_number)?, "height", line_number)?,
        }),
        "pixel.set" => Ok(Command::PixelSet {
            x: parse_i64(require(&fields, "x", line_number)?, "x", line_number)?,
            y: parse_i64(require(&fields, "y", line_number)?, "y", line_number)?,
            color: parse_color(require(&fields, "color", line_number)?, "color", line_number)?,
        }),
        "line.draw" => Ok(Command::LineDraw {
            x0: parse_i64(require(&fields, "x0", line_number)?, "x0", line_number)?,
            y0: parse_i64(require(&fields, "y0", line_number)?, "y0", line_number)?,
            x1: parse_i64(require(&fields, "x1", line_number)?, "x1", line_number)?,
            y1: parse_i64(require(&fields, "y1", line_number)?, "y1", line_number)?,
            color: parse_color(require(&fields, "color", line_number)?, "color", line_number)?,
        }),
        "palette.set" => Ok(Command::PaletteSet {
            index: parse_u8(require(&fields, "index", line_number)?, "index", line_number)?,
            color: parse_color(require(&fields, "color", line_number)?, "color", line_number)?,
        }),
        "layer.add" => Ok(Command::LayerAdd {
            name: sanitize_name(require(&fields, "name", line_number)?),
        }),
        "layer.select" => Ok(Command::LayerSelect {
            index: parse_usize(require(&fields, "index", line_number)?, "index", line_number)?,
        }),
        "layer.remove" => Ok(Command::LayerRemove {
            index: parse_usize(require(&fields, "index", line_number)?, "index", line_number)?,
        }),
        "layer.rename" => Ok(Command::LayerRename {
            index: parse_usize(require(&fields, "index", line_number)?, "index", line_number)?,
            name: sanitize_name(require(&fields, "name", line_number)?),
        }),
        "layer.move" => Ok(Command::LayerMove {
            from: parse_usize(require(&fields, "from", line_number)?, "from", line_number)?,
            to: parse_usize(require(&fields, "to", line_number)?, "to", line_number)?,
        }),
        "layer.visible" => Ok(Command::LayerVisible {
            index: parse_usize(require(&fields, "index", line_number)?, "index", line_number)?,
            value: parse_bool(require(&fields, "value", line_number)?, "value", line_number)?,
        }),
        "layer.opacity" => Ok(Command::LayerOpacity {
            index: parse_usize(require(&fields, "index", line_number)?, "index", line_number)?,
            value: parse_u8(require(&fields, "value", line_number)?, "value", line_number)?,
        }),
        other => Err(ParseError {
            line_number,
            message: format!("unknown command {:?}", other),
        }),
    }
}

/// Parses a full `.pxc` script into an ordered list of commands, skipping
/// blank lines and full-line comments (lines whose first non-whitespace
/// character is `#`).
pub fn parse_script(text: &str) -> Result<Vec<Command>, ParseError> {
    let mut commands = Vec::new();
    for (i, raw_line) in text.lines().enumerate() {
        let line_number = i + 1;
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        commands.push(parse_line(trimmed, line_number)?);
    }
    Ok(commands)
}

/// Serializes a single [`Command`] back to `.pxc` text (no trailing
/// newline).
pub fn serialize_command(command: &Command) -> String {
    match command {
        Command::CanvasNew { width, height } => {
            format!("canvas.new width={} height={}", width, height)
        }
        Command::PixelSet { x, y, color } => {
            format!("pixel.set x={} y={} color=\"{}\"", x, y, format_hex_color(*color))
        }
        Command::LineDraw { x0, y0, x1, y1, color } => format!(
            "line.draw x0={} y0={} x1={} y1={} color=\"{}\"",
            x0,
            y0,
            x1,
            y1,
            format_hex_color(*color)
        ),
        Command::PaletteSet { index, color } => {
            format!("palette.set index={} color=\"{}\"", index, format_hex_color(*color))
        }
        Command::LayerAdd { name } => format!("layer.add name=\"{}\"", sanitize_name(name)),
        Command::LayerSelect { index } => format!("layer.select index={}", index),
        Command::LayerRemove { index } => format!("layer.remove index={}", index),
        Command::LayerRename { index, name } => {
            format!("layer.rename index={} name=\"{}\"", index, sanitize_name(name))
        }
        Command::LayerMove { from, to } => format!("layer.move from={} to={}", from, to),
        Command::LayerVisible { index, value } => {
            format!("layer.visible index={} value={}", index, value)
        }
        Command::LayerOpacity { index, value } => {
            format!("layer.opacity index={} value={}", index, value)
        }
    }
}

/// Serializes a full command list to `.pxc` text, one command per line,
/// terminated with a trailing newline.
pub fn serialize_script(commands: &[Command]) -> String {
    let mut out = String::new();
    for command in commands {
        out.push_str(&serialize_command(command));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_commands() -> Vec<Command> {
        vec![
            Command::CanvasNew { width: 64, height: 32 },
            Command::PixelSet { x: 3, y: 4, color: [0x1d, 0x1d, 0x1f, 0xff] },
            Command::LineDraw {
                x0: 0,
                y0: 0,
                x1: 10,
                y1: 5,
                color: [0xff, 0xff, 0xff, 0x80],
            },
            Command::PaletteSet { index: 2, color: [0x00, 0x00, 0x00, 0xff] },
            Command::LayerAdd { name: "Rigging".to_string() },
            Command::LayerSelect { index: 1 },
            Command::LayerRemove { index: 2 },
            Command::LayerRename { index: 0, name: "Hull outline".to_string() },
            Command::LayerMove { from: 2, to: 0 },
            Command::LayerVisible { index: 1, value: false },
            Command::LayerOpacity { index: 1, value: 128 },
        ]
    }

    #[test]
    fn round_trips_all_command_kinds() {
        for command in all_commands() {
            let text = serialize_command(&command);
            let parsed = parse_line(&text, 1).unwrap();
            assert_eq!(parsed, command, "round trip failed for {:?}", text);
        }
    }

    #[test]
    fn round_trips_full_script() {
        let commands = all_commands();
        let script = serialize_script(&commands);
        let parsed = parse_script(&script).unwrap();
        assert_eq!(parsed, commands);
    }

    #[test]
    fn skips_blank_lines_and_comments() {
        let script = "\n# a comment\ncanvas.new width=1 height=1\n   \n# trailing\n";
        let parsed = parse_script(script).unwrap();
        assert_eq!(parsed, vec![Command::CanvasNew { width: 1, height: 1 }]);
    }

    #[test]
    fn accepts_unquoted_color_value() {
        let parsed = parse_line("pixel.set x=1 y=2 color=#ff0000", 1).unwrap();
        assert_eq!(parsed, Command::PixelSet { x: 1, y: 2, color: [0xff, 0, 0, 0xff] });
    }

    #[test]
    fn reports_line_number_on_error() {
        let err = parse_script("canvas.new width=1 height=1\npixel.set x=1\n").unwrap_err();
        assert_eq!(err.line_number, 2);
    }

    #[test]
    fn layer_name_with_spaces_survives_a_round_trip() {
        let command = Command::LayerAdd { name: "Deck plan A".to_string() };
        let text = serialize_command(&command);
        assert_eq!(text, r#"layer.add name="Deck plan A""#);
        assert_eq!(parse_line(&text, 1).unwrap(), command);
    }

    #[test]
    fn layer_name_is_sanitized_identically_by_both_directions() {
        // A name the grammar cannot represent is normalised the same way
        // whether it arrives from text or from a constructed Command, so a
        // save/load cycle is still a fixed point.
        let dirty = Command::LayerAdd { name: "a\"b".to_string() };
        let text = serialize_command(&dirty);
        let parsed = parse_line(&text, 1).unwrap();
        assert_eq!(parsed, Command::LayerAdd { name: "a b".to_string() });
        assert_eq!(serialize_command(&parsed), text, "sanitisation is idempotent");
    }

    #[test]
    fn rejects_bad_boolean() {
        let err = parse_line("layer.visible index=0 value=yes", 3).unwrap_err();
        assert_eq!(err.line_number, 3);
        assert!(err.message.contains("must be true or false"), "got: {}", err.message);
    }

    #[test]
    fn rejects_unknown_command() {
        let err = parse_line("foo.bar x=1", 5).unwrap_err();
        assert_eq!(err.line_number, 5);
        assert!(err.message.contains("unknown command"));
    }
}
