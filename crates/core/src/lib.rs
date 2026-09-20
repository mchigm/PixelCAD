//! `pixelcad-core` — headless document model and typed command engine.
//!
//! This crate must never depend on any UI, GPU, or platform-specific crate.
//! It is the single source of truth for document state and command
//! execution, shared verbatim by the GUI shell (`pixelcad-app`) and the
//! headless CLI (`pixelcad-cli`).

pub mod base64;
pub mod command;
pub mod document;
pub mod engine;
pub mod font;
pub mod parser;
pub mod project;
pub mod selection;

pub use command::{Axis, BrushShape, Command};
pub use document::{Color, Document, DocumentError, Layer, OPAQUE, TRANSPARENT};
pub use engine::{Engine, EngineError, DEFAULT_PALETTE, PALETTE_SIZE};
pub use selection::Selection;
pub use parser::{parse_line, parse_script, serialize_command, serialize_script, ParseError};
pub use project::{
    open_project, parse_project, serialize_project, OpenError, Project, ProjectError,
    LEGACY_SCRIPT_VERSION, PROJECT_FORMAT_VERSION,
};
