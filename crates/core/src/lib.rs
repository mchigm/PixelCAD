//! `pixelcad-core` — headless document model and typed command engine.
//!
//! This crate must never depend on any UI, GPU, or platform-specific crate.
//! It is the single source of truth for document state and command
//! execution, shared verbatim by the GUI shell (`pixelcad-app`) and the
//! headless CLI (`pixelcad-cli`).

pub mod command;
pub mod document;
pub mod engine;
pub mod parser;

pub use command::Command;
pub use document::{Color, Document, DocumentError, Layer, OPAQUE, TRANSPARENT};
pub use engine::{Engine, EngineError, SelectionRect, DEFAULT_PALETTE, PALETTE_SIZE};
pub use parser::{parse_line, parse_script, serialize_command, serialize_script, ParseError};
