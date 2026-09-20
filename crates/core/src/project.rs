//! The versioned PixelCAD project file (`.pxcproj`).
//!
//! # Why this is not a zip archive
//!
//! `ROADMAP.md` calls this a "`.pxc` archive". It is implemented as a
//! **self-contained versioned text container** rather than a zip, for four
//! concrete reasons:
//!
//! 1. `pixelcad-core` must stay dependency-free apart from `thiserror`; a
//!    zip writer is a dependency with its own compression-level and
//!    timestamp behaviour.
//! 2. Byte-level determinism is a project invariant. A text container has no
//!    timestamps, no entry ordering, and no compressor version to vary.
//! 3. "Every user action becomes a serializable, replayable command" — so
//!    the natural project file *is* the command log. Opening a project is
//!    literally replaying it, which means there is exactly one code path
//!    that can produce a document, and it is the one the CLI already tests.
//! 4. It stays diffable and mergeable in git, which matters for a tool whose
//!    whole premise is scriptable, reproducible authoring.
//!
//! # Format
//!
//! ```text
//! # optional comments
//! pixelcad.project version=1
//! canvas.new width=64 height=64
//! layer.add name="Rigging"
//! ...
//! ```
//!
//! The header must be the **first non-blank, non-comment line**. A file with
//! no header is accepted as legacy version `0` — that is exactly a Phase 0
//! `.pxc` script, and those must keep working forever.
//!
//! A file declaring a version **newer** than [`PROJECT_FORMAT_VERSION`] is
//! rejected outright with [`ProjectError::UnsupportedVersion`]. Because
//! parsing completes before any command is executed, a rejected file cannot
//! partially mutate anything.

use crate::command::Command;
use crate::engine::Engine;
use crate::parser::{parse_line, serialize_script, ParseError};

/// The project format version this build writes and is the newest it can
/// read.
pub const PROJECT_FORMAT_VERSION: u32 = 1;

/// The version reported for a bare Phase 0 `.pxc` script with no header.
pub const LEGACY_SCRIPT_VERSION: u32 = 0;

/// The header keyword that opens a project file.
const HEADER_KEYWORD: &str = "pixelcad.project";

/// Errors produced while reading a project file.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProjectError {
    #[error(
        "project file declares format version {found}, but this build of PixelCAD \
         supports at most version {supported} — upgrade PixelCAD to open it"
    )]
    UnsupportedVersion { found: u32, supported: u32 },
    #[error("line {line_number}: malformed project header: {message}")]
    MalformedHeader { line_number: usize, message: String },
    #[error("{0}")]
    Parse(#[from] ParseError),
}

/// A project file, parsed but not yet executed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    /// The declared format version ([`LEGACY_SCRIPT_VERSION`] if the file
    /// had no header).
    pub version: u32,
    /// The command log, in execution order.
    pub commands: Vec<Command>,
}

/// Serializes an engine's current history as a version-1 project file.
///
/// Grouping is deliberately not represented: undo steps are session state,
/// not document state. Reopening a project gives one undo step per command,
/// which is correct — you did not perform those drags in this session.
pub fn serialize_project(engine: &Engine) -> String {
    let mut out = String::new();
    out.push_str("# PixelCAD project file. Replay with:\n");
    out.push_str("#   pixelcad-cli run <this file> --out out.png\n");
    out.push_str(&format!("{HEADER_KEYWORD} version={PROJECT_FORMAT_VERSION}\n"));
    out.push_str(&serialize_script(&engine.history()));
    out
}

/// Parses project text into a [`Project`].
///
/// Accepts both a headed `.pxcproj` and a bare Phase 0 `.pxc` script.
/// Returns an error **before executing anything**, so a bad file can never
/// half-apply.
pub fn parse_project(text: &str) -> Result<Project, ProjectError> {
    let mut version: Option<u32> = None;
    let mut commands = Vec::new();

    for (i, raw_line) in text.lines().enumerate() {
        let line_number = i + 1;
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix(HEADER_KEYWORD) {
            if version.is_some() {
                return Err(ProjectError::MalformedHeader {
                    line_number,
                    message: "duplicate header".to_string(),
                });
            }
            if !commands.is_empty() {
                return Err(ProjectError::MalformedHeader {
                    line_number,
                    message: "header must precede all commands".to_string(),
                });
            }
            version = Some(parse_header_version(rest, line_number)?);
            continue;
        }

        // Not the header: an ordinary command. A file that starts with
        // commands is a legacy script.
        if version.is_none() {
            version = Some(LEGACY_SCRIPT_VERSION);
        }
        commands.push(parse_line(trimmed, line_number)?);
    }

    let version = version.unwrap_or(LEGACY_SCRIPT_VERSION);
    if version > PROJECT_FORMAT_VERSION {
        return Err(ProjectError::UnsupportedVersion {
            found: version,
            supported: PROJECT_FORMAT_VERSION,
        });
    }
    Ok(Project { version, commands })
}

fn parse_header_version(rest: &str, line_number: usize) -> Result<u32, ProjectError> {
    let rest = rest.trim();
    let value = rest.strip_prefix("version=").ok_or_else(|| ProjectError::MalformedHeader {
        line_number,
        message: format!("expected `{HEADER_KEYWORD} version=<n>`, got {rest:?}"),
    })?;
    value.trim_matches('"').parse::<u32>().map_err(|e| ProjectError::MalformedHeader {
        line_number,
        message: format!("version {value:?} is not a number: {e}"),
    })
}

/// Parses project text and replays it into a fresh [`Engine`].
///
/// Building a *new* engine rather than mutating a caller-supplied one is
/// what guarantees the "never partially corrupt an open document" invariant
/// at the API level: the caller only receives an engine if the whole file
/// parsed and replayed successfully.
pub fn open_project(text: &str) -> Result<Engine, OpenError> {
    let project = parse_project(text)?;
    let mut engine = Engine::new();
    engine.execute_all(project.commands)?;
    Ok(engine)
}

/// Failure modes of [`open_project`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OpenError {
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error("while replaying the project: {0}")]
    Execute(#[from] crate::engine::EngineError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Command;

    fn sample_engine() -> Engine {
        let mut e = Engine::new();
        e.execute(Command::CanvasNew { width: 8, height: 8 }).unwrap();
        e.execute(Command::LayerAdd { name: "Rigging".to_string() }).unwrap();
        e.execute(Command::RectDraw {
            x0: 1,
            y0: 1,
            x1: 6,
            y1: 6,
            radius: 0,
            fill: false,
            color: [0x1d, 0x1d, 0x1f, 0xff],
        })
        .unwrap();
        e.execute(Command::LayerOpacity { index: 1, value: 200 }).unwrap();
        e
    }

    #[test]
    fn serialized_project_declares_the_current_version_first() {
        let text = serialize_project(&sample_engine());
        let first_real_line = text
            .lines()
            .find(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
            .unwrap();
        assert_eq!(first_real_line, "pixelcad.project version=1");
    }

    #[test]
    fn save_and_reload_preserves_the_document_and_the_history() {
        let original = sample_engine();
        let text = serialize_project(&original);
        let reloaded = open_project(&text).unwrap();

        assert_eq!(
            original.document_hash(),
            reloaded.document_hash(),
            "a round trip must reproduce the document exactly"
        );
        assert_eq!(original.history(), reloaded.history());
    }

    #[test]
    fn a_round_trip_is_a_fixed_point() {
        let text = serialize_project(&sample_engine());
        let again = serialize_project(&open_project(&text).unwrap());
        assert_eq!(text, again, "save -> load -> save must be byte-identical");
    }

    #[test]
    fn a_bare_phase_0_script_still_loads_as_legacy_version_zero() {
        let script = "canvas.new width=4 height=4\npixel.set x=1 y=1 color=\"#ff0000\"\n";
        let project = parse_project(script).unwrap();
        assert_eq!(project.version, LEGACY_SCRIPT_VERSION);
        assert_eq!(project.commands.len(), 2);

        let engine = open_project(script).unwrap();
        assert_eq!(engine.document().unwrap().get_pixel(1, 1).unwrap(), [255, 0, 0, 255]);
    }

    #[test]
    fn a_future_version_is_rejected_and_names_both_versions() {
        let text = "pixelcad.project version=99\ncanvas.new width=4 height=4\n";
        let err = parse_project(text).unwrap_err();
        assert_eq!(
            err,
            ProjectError::UnsupportedVersion { found: 99, supported: PROJECT_FORMAT_VERSION }
        );
        let message = err.to_string();
        assert!(message.contains("99") && message.contains('1'), "got: {message}");
    }

    #[test]
    fn a_future_version_leaves_no_partial_document() {
        // The whole point of parsing before executing: the canvas.new on
        // line 2 must never run.
        let text = "pixelcad.project version=99\ncanvas.new width=4 height=4\n";
        assert!(matches!(
            open_project(text),
            Err(OpenError::Project(ProjectError::UnsupportedVersion { .. }))
        ));
    }

    #[test]
    fn a_parse_error_anywhere_aborts_the_whole_load() {
        let text = "pixelcad.project version=1\ncanvas.new width=4 height=4\nbogus.command x=1\n";
        assert!(matches!(open_project(text), Err(OpenError::Project(ProjectError::Parse(_)))));
    }

    #[test]
    fn an_execution_error_is_reported_distinctly_from_a_parse_error() {
        // Parses fine, but pixel.set before canvas.new cannot execute.
        let text = "pixelcad.project version=1\npixel.set x=0 y=0 color=\"#ffffff\"\n";
        assert!(matches!(open_project(text), Err(OpenError::Execute(_))));
    }

    #[test]
    fn a_malformed_header_is_reported_with_its_line_number() {
        let text = "\n# comment\npixelcad.project v=1\n";
        let err = parse_project(text).unwrap_err();
        assert!(
            matches!(err, ProjectError::MalformedHeader { line_number: 3, .. }),
            "got: {err:?}"
        );
    }

    #[test]
    fn a_header_after_commands_is_rejected() {
        let text = "canvas.new width=4 height=4\npixelcad.project version=1\n";
        assert!(matches!(
            parse_project(text),
            Err(ProjectError::MalformedHeader { line_number: 2, .. })
        ));
    }

    #[test]
    fn a_duplicate_header_is_rejected() {
        let text = "pixelcad.project version=1\npixelcad.project version=1\n";
        assert!(matches!(parse_project(text), Err(ProjectError::MalformedHeader { .. })));
    }

    #[test]
    fn comments_and_blank_lines_before_the_header_are_allowed() {
        let text = "# a note\n\n  \npixelcad.project version=1\ncanvas.new width=2 height=2\n";
        let project = parse_project(text).unwrap();
        assert_eq!(project.version, 1);
        assert_eq!(project.commands.len(), 1);
    }

    #[test]
    fn an_empty_file_is_an_empty_legacy_project() {
        let project = parse_project("").unwrap();
        assert_eq!(project.version, LEGACY_SCRIPT_VERSION);
        assert!(project.commands.is_empty());
    }
}
