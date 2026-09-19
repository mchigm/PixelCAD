//! `pixelcad-core` — headless document model and typed command engine.
//!
//! This crate must never depend on any UI, GPU, or platform-specific crate.
//! It is the single source of truth for document state and command
//! execution, shared verbatim by the GUI shell (`pixelcad-app`) and the
//! headless CLI (`pixelcad-cli`).

pub mod document;

pub use document::{Color, Document, DocumentError, TRANSPARENT};
