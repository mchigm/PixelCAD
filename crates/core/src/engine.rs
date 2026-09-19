//! The command executor: applies [`Command`]s to a [`Document`], keeps a
//! fixed-size palette, and implements undo/redo as full-state snapshots.
//!
//! Snapshot-based undo/redo is deliberately simple: correctness now,
//! optimize later (see PLAN.md). Every operation here is deterministic —
//! given the same starting state and the same command sequence, the
//! resulting document is byte-identical, which is what makes CLI replay and
//! GUI-recorded scripts interchangeable.

use crate::command::{sanitize_name, Command};
use crate::document::{Color, Document, DocumentError};
use crate::parser::serialize_script;

/// Number of fixed palette swatches (see PLAN.md Task 5).
pub const PALETTE_SIZE: usize = 16;

/// A reasonable default 16-swatch palette. Purely a starting point for the
/// GUI; `palette.set` commands can overwrite any slot.
pub const DEFAULT_PALETTE: [Color; PALETTE_SIZE] = [
    [0x00, 0x00, 0x00, 0xff], // black
    [0xff, 0xff, 0xff, 0xff], // white
    [0x88, 0x88, 0x88, 0xff], // gray
    [0xc0, 0xc0, 0xc0, 0xff], // light gray
    [0xff, 0x00, 0x00, 0xff], // red
    [0x00, 0xff, 0x00, 0xff], // green
    [0x00, 0x00, 0xff, 0xff], // blue
    [0xff, 0xff, 0x00, 0xff], // yellow
    [0x00, 0xff, 0xff, 0xff], // cyan
    [0xff, 0x00, 0xff, 0xff], // magenta
    [0xff, 0xa5, 0x00, 0xff], // orange
    [0x80, 0x00, 0x80, 0xff], // purple
    [0x8b, 0x45, 0x13, 0xff], // brown
    [0xff, 0xc0, 0xcb, 0xff], // pink
    [0x00, 0x64, 0x00, 0xff], // dark green
    [0x00, 0x00, 0x80, 0xff], // navy
];

/// Errors produced while executing a [`Command`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EngineError {
    #[error("no canvas exists yet; run canvas.new first")]
    NoCanvas,
    #[error("palette index {index} is out of range (0..{PALETTE_SIZE})")]
    PaletteIndexOutOfRange { index: u8 },
    #[error(transparent)]
    Document(#[from] DocumentError),
}

/// The full engine state at one point in history: the document (if a canvas
/// has been created) and the palette.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EngineState {
    document: Option<Document>,
    palette: [Color; PALETTE_SIZE],
}

impl Default for EngineState {
    fn default() -> Self {
        Self { document: None, palette: DEFAULT_PALETTE }
    }
}

/// Executes [`Command`]s against a document, with snapshot-based undo/redo
/// and a replayable command history.
#[derive(Debug, Clone)]
pub struct Engine {
    /// `states[i]` is the state that exists after executing `commands[0..i]`.
    /// `states[0]` is always the initial (empty) state.
    states: Vec<EngineState>,
    /// `commands[i]` is the command that produced `states[i + 1]`.
    commands: Vec<Command>,
    /// Index into `states` for the currently active state. Everything in
    /// `commands[cursor..]` has been undone and is kept only so `redo` can
    /// restore it; a fresh `execute` call discards it.
    cursor: usize,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    /// Creates a fresh engine: no document, default palette, empty history.
    pub fn new() -> Self {
        Self { states: vec![EngineState::default()], commands: Vec::new(), cursor: 0 }
    }

    /// The current document, if `canvas.new` has been executed.
    pub fn document(&self) -> Option<&Document> {
        self.states[self.cursor].document.as_ref()
    }

    /// The current palette.
    pub fn palette(&self) -> &[Color; PALETTE_SIZE] {
        &self.states[self.cursor].palette
    }

    /// The commands currently in effect, in execution order (excludes any
    /// commands undone via [`Engine::undo`] that have not been redone).
    /// This is exactly what `.pxc` "Save script" should serialize.
    pub fn history(&self) -> &[Command] {
        &self.commands[..self.cursor]
    }

    /// Serializes [`Engine::history`] to `.pxc` text.
    pub fn save_script(&self) -> String {
        serialize_script(self.history())
    }

    /// Executes a single command, appending it to history. Any previously
    /// undone commands beyond the current cursor are discarded (standard
    /// undo/redo semantics: a new action clears the redo stack).
    pub fn execute(&mut self, command: Command) -> Result<(), EngineError> {
        let mut next_state = self.states[self.cursor].clone();
        apply(&mut next_state, &command)?;

        self.states.truncate(self.cursor + 1);
        self.commands.truncate(self.cursor);

        self.states.push(next_state);
        self.commands.push(command);
        self.cursor += 1;
        Ok(())
    }

    /// Executes a sequence of commands in order, stopping at the first
    /// error. On error, all commands before the failing one remain applied.
    pub fn execute_all<I: IntoIterator<Item = Command>>(
        &mut self,
        commands: I,
    ) -> Result<(), EngineError> {
        for command in commands {
            self.execute(command)?;
        }
        Ok(())
    }

    /// Whether [`Engine::undo`] would move (i.e. there is history before the
    /// current position).
    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }

    /// Whether [`Engine::redo`] would move (i.e. there is undone history
    /// ahead of the current position).
    pub fn can_redo(&self) -> bool {
        self.cursor < self.commands.len()
    }

    /// Moves one step back in history, if possible. Returns whether it
    /// moved.
    pub fn undo(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        self.cursor -= 1;
        true
    }

    /// Moves one step forward in history (re-applying a previously undone
    /// command), if possible. Returns whether it moved.
    pub fn redo(&mut self) -> bool {
        if self.cursor >= self.commands.len() {
            return false;
        }
        self.cursor += 1;
        true
    }

    /// A deterministic hash of the current document, or `None` if no canvas
    /// exists yet. Used by determinism tests to compare independently
    /// produced engines without comparing raw buffers.
    pub fn document_hash(&self) -> Option<u64> {
        self.document().map(Document::content_hash)
    }
}

/// Applies `command` to `state` in place.
fn apply(state: &mut EngineState, command: &Command) -> Result<(), EngineError> {
    match *command {
        Command::CanvasNew { width, height } => {
            state.document = Some(Document::new(width, height)?);
            Ok(())
        }
        Command::PixelSet { x, y, color } => {
            let doc = state.document.as_mut().ok_or(EngineError::NoCanvas)?;
            doc.set_pixel(x, y, color)?;
            Ok(())
        }
        Command::LineDraw { x0, y0, x1, y1, color } => {
            let doc = state.document.as_mut().ok_or(EngineError::NoCanvas)?;
            for (x, y) in bresenham_points(x0, y0, x1, y1) {
                doc.set_pixel(x, y, color)?;
            }
            Ok(())
        }
        Command::PaletteSet { index, color } => {
            let i = index as usize;
            if i >= PALETTE_SIZE {
                return Err(EngineError::PaletteIndexOutOfRange { index });
            }
            state.palette[i] = color;
            Ok(())
        }
        Command::LayerAdd { ref name } => {
            let doc = state.document.as_mut().ok_or(EngineError::NoCanvas)?;
            doc.add_layer(sanitize_name(name));
            Ok(())
        }
        Command::LayerSelect { index } => {
            let doc = state.document.as_mut().ok_or(EngineError::NoCanvas)?;
            doc.select_layer(index)?;
            Ok(())
        }
        Command::LayerRemove { index } => {
            let doc = state.document.as_mut().ok_or(EngineError::NoCanvas)?;
            doc.remove_layer(index)?;
            Ok(())
        }
        Command::LayerRename { index, ref name } => {
            let doc = state.document.as_mut().ok_or(EngineError::NoCanvas)?;
            doc.rename_layer(index, sanitize_name(name))?;
            Ok(())
        }
        Command::LayerMove { from, to } => {
            let doc = state.document.as_mut().ok_or(EngineError::NoCanvas)?;
            doc.move_layer(from, to)?;
            Ok(())
        }
        Command::LayerVisible { index, value } => {
            let doc = state.document.as_mut().ok_or(EngineError::NoCanvas)?;
            doc.set_layer_visible(index, value)?;
            Ok(())
        }
        Command::LayerOpacity { index, value } => {
            let doc = state.document.as_mut().ok_or(EngineError::NoCanvas)?;
            doc.set_layer_opacity(index, value)?;
            Ok(())
        }
    }
}

/// Computes every integer point on the line from `(x0, y0)` to `(x1, y1)`
/// inclusive, using Bresenham's algorithm. Deterministic: always produces
/// the same point sequence for the same inputs, on every platform.
fn bresenham_points(x0: i64, y0: i64, x1: i64, y1: i64) -> Vec<(i64, i64)> {
    let mut points = Vec::new();
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx: i64 = if x0 < x1 { 1 } else { -1 };
    let sy: i64 = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    let (mut x, mut y) = (x0, y0);
    loop {
        points.push((x, y));
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
    points
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_script;

    #[test]
    fn canvas_new_creates_document() {
        let mut engine = Engine::new();
        assert!(engine.document().is_none());
        engine.execute(Command::CanvasNew { width: 4, height: 4 }).unwrap();
        let doc = engine.document().unwrap();
        assert_eq!((doc.width(), doc.height()), (4, 4));
    }

    #[test]
    fn pixel_set_requires_canvas() {
        let mut engine = Engine::new();
        let err = engine
            .execute(Command::PixelSet { x: 0, y: 0, color: [0, 0, 0, 0xff] })
            .unwrap_err();
        assert_eq!(err, EngineError::NoCanvas);
    }

    #[test]
    fn undo_redo_round_trip() {
        let mut engine = Engine::new();
        engine.execute(Command::CanvasNew { width: 2, height: 2 }).unwrap();
        engine.execute(Command::PixelSet { x: 0, y: 0, color: [1, 1, 1, 1] }).unwrap();
        let after_set = engine.document().unwrap().clone();

        assert!(engine.undo());
        assert_eq!(engine.document().unwrap().get_pixel(0, 0).unwrap(), [0, 0, 0, 0]);

        assert!(engine.redo());
        assert_eq!(engine.document().unwrap(), &after_set);

        // redo() at the end of history is a no-op.
        assert!(!engine.redo());
    }

    #[test]
    fn new_action_after_undo_clears_redo_history() {
        let mut engine = Engine::new();
        engine.execute(Command::CanvasNew { width: 2, height: 2 }).unwrap();
        engine.execute(Command::PixelSet { x: 0, y: 0, color: [1, 1, 1, 1] }).unwrap();
        engine.undo();
        engine.execute(Command::PixelSet { x: 1, y: 1, color: [2, 2, 2, 2] }).unwrap();
        // The old "set (0,0)" branch is gone; redo has nothing to do.
        assert!(!engine.redo());
        assert_eq!(engine.history().len(), 2);
    }

    #[test]
    fn history_reflects_only_active_commands() {
        let mut engine = Engine::new();
        engine.execute(Command::CanvasNew { width: 2, height: 2 }).unwrap();
        engine.execute(Command::PixelSet { x: 0, y: 0, color: [1, 1, 1, 1] }).unwrap();
        engine.undo();
        assert_eq!(engine.history(), &[Command::CanvasNew { width: 2, height: 2 }]);
    }

    #[test]
    fn palette_set_updates_slot() {
        let mut engine = Engine::new();
        engine.execute(Command::PaletteSet { index: 3, color: [9, 9, 9, 9] }).unwrap();
        assert_eq!(engine.palette()[3], [9, 9, 9, 9]);
    }

    #[test]
    fn palette_set_out_of_range_errors() {
        let mut engine = Engine::new();
        let err = engine
            .execute(Command::PaletteSet { index: 200, color: [0, 0, 0, 0] })
            .unwrap_err();
        assert_eq!(err, EngineError::PaletteIndexOutOfRange { index: 200 });
    }

    #[test]
    fn layer_commands_build_and_reorder_a_stack() {
        let mut engine = Engine::new();
        engine.execute(Command::CanvasNew { width: 2, height: 1 }).unwrap();
        engine.execute(Command::LayerAdd { name: "rigging".to_string() }).unwrap();
        assert_eq!(engine.document().unwrap().layer_count(), 2);
        assert_eq!(engine.document().unwrap().active_layer_index(), 1);

        engine.execute(Command::LayerRename { index: 0, name: "hull".to_string() }).unwrap();
        assert_eq!(engine.document().unwrap().layers()[0].name(), "hull");

        engine.execute(Command::LayerMove { from: 1, to: 0 }).unwrap();
        assert_eq!(engine.document().unwrap().layers()[0].name(), "rigging");

        engine.execute(Command::LayerVisible { index: 0, value: false }).unwrap();
        assert!(!engine.document().unwrap().layers()[0].visible());

        engine.execute(Command::LayerOpacity { index: 0, value: 64 }).unwrap();
        assert_eq!(engine.document().unwrap().layers()[0].opacity(), 64);

        engine.execute(Command::LayerSelect { index: 1 }).unwrap();
        engine.execute(Command::LayerRemove { index: 0 }).unwrap();
        assert_eq!(engine.document().unwrap().layer_count(), 1);
    }

    #[test]
    fn layer_commands_require_a_canvas() {
        let mut engine = Engine::new();
        let err = engine.execute(Command::LayerAdd { name: "x".to_string() }).unwrap_err();
        assert_eq!(err, EngineError::NoCanvas);
    }

    #[test]
    fn removing_the_last_layer_is_refused_and_leaves_the_document_intact() {
        let mut engine = Engine::new();
        engine.execute(Command::CanvasNew { width: 2, height: 2 }).unwrap();
        let before = engine.document().unwrap().clone();
        let err = engine.execute(Command::LayerRemove { index: 0 }).unwrap_err();
        assert_eq!(err, EngineError::Document(DocumentError::LastLayer));
        assert_eq!(
            engine.document().unwrap(),
            &before,
            "a rejected command must not mutate the live document"
        );
        assert_eq!(engine.history().len(), 1, "a failed command must not enter history");
    }

    #[test]
    fn undo_restores_a_removed_layer_with_its_pixels() {
        let mut engine = Engine::new();
        engine.execute(Command::CanvasNew { width: 2, height: 1 }).unwrap();
        engine.execute(Command::LayerAdd { name: "top".to_string() }).unwrap();
        engine.execute(Command::PixelSet { x: 0, y: 0, color: [1, 2, 3, 4] }).unwrap();
        engine.execute(Command::LayerRemove { index: 1 }).unwrap();
        assert_eq!(engine.document().unwrap().layer_count(), 1);

        assert!(engine.undo());
        let doc = engine.document().unwrap();
        assert_eq!(doc.layer_count(), 2);
        assert_eq!(doc.get_pixel_on(1, 0, 0).unwrap(), [1, 2, 3, 4]);
    }

    #[test]
    fn line_draw_sets_every_bresenham_point() {
        let mut engine = Engine::new();
        engine.execute(Command::CanvasNew { width: 10, height: 10 }).unwrap();
        engine
            .execute(Command::LineDraw {
                x0: 0,
                y0: 0,
                x1: 4,
                y1: 4,
                color: [255, 255, 255, 255],
            })
            .unwrap();
        let doc = engine.document().unwrap();
        for i in 0..=4 {
            assert_eq!(doc.get_pixel(i, i).unwrap(), [255, 255, 255, 255]);
        }
    }

    const SAMPLE_SCRIPT: &str = r##"
        canvas.new width=16 height=16
        palette.set index=0 color="#1d1d1f"
        line.draw x0=0 y0=0 x1=15 y1=0 color="#1d1d1f"
        line.draw x0=0 y0=0 x1=0 y1=15 color="#1d1d1f"
        pixel.set x=8 y=8 color="#ff0000"
    "##;

    #[test]
    fn determinism_replaying_script_twice_yields_identical_hash() {
        let commands = parse_script(SAMPLE_SCRIPT).unwrap();

        let mut engine_a = Engine::new();
        engine_a.execute_all(commands.clone()).unwrap();

        let mut engine_b = Engine::new();
        engine_b.execute_all(commands).unwrap();

        assert_eq!(engine_a.document_hash(), engine_b.document_hash());
        assert!(engine_a.document_hash().is_some());
    }

    #[test]
    fn save_script_round_trips_through_parser() {
        let commands = parse_script(SAMPLE_SCRIPT).unwrap();
        let mut engine = Engine::new();
        engine.execute_all(commands.clone()).unwrap();

        let saved = engine.save_script();
        let reparsed = parse_script(&saved).unwrap();
        assert_eq!(reparsed, commands);
    }
}
