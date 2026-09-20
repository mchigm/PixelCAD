//! The command executor: applies [`Command`]s to a [`Document`], keeps a
//! fixed-size palette, and implements undo/redo as full-state snapshots.
//!
//! Snapshot-based undo/redo is deliberately simple: correctness now,
//! optimize later (see PLAN.md). Every operation here is deterministic —
//! given the same starting state and the same command sequence, the
//! resulting document is byte-identical, which is what makes CLI replay and
//! GUI-recorded scripts interchangeable.

use crate::command::{sanitize_name, Axis, BrushShape, Command};
use crate::selection::Selection;
use crate::document::{Color, Document, DocumentError, TRANSPARENT};
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
    #[error(transparent)]
    Base64(#[from] crate::base64::Base64Error),
    #[error("image.import declares {expected} bytes of pixel data but carries {found}")]
    ImageDataLength { expected: usize, found: usize },
    #[error("this command needs an active selection; run select.rect, select.lasso or select.all first")]
    NoSelection,
    #[error("rotation must be 90, 180 or 270 degrees (got {degrees})")]
    UnsupportedRotation { degrees: u32 },
}

/// The full engine state at one point in history: the document (if a canvas
/// has been created), the palette, the active drawing colour, and the
/// current selection. All four are snapshotted together, so undo restores
/// tool state as faithfully as it restores pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EngineState {
    document: Option<Document>,
    palette: [Color; PALETTE_SIZE],
    current_color: Color,
    selection: Option<Selection>,
}

impl Default for EngineState {
    fn default() -> Self {
        Self {
            document: None,
            palette: DEFAULT_PALETTE,
            current_color: DEFAULT_PALETTE[0],
            selection: None,
        }
    }
}

/// Executes [`Command`]s against a document, with snapshot-based undo/redo
/// and a replayable command history.
#[derive(Debug, Clone)]
pub struct Engine {
    /// `states[i]` is the state that exists after executing `steps[0..i]`.
    /// `states[0]` is always the initial (empty) state.
    states: Vec<EngineState>,
    /// One *undo step* per entry, each holding one or more commands that
    /// were applied together. `steps[i]` produced `states[i + 1]`.
    ///
    /// Phase 0 had one command per undo step. Phase 1 groups them so that a
    /// single pointer drag — which emits dozens of `brush.stroke` commands —
    /// is undone by a single Undo, while the serialized script still
    /// contains every individual command.
    steps: Vec<Vec<Command>>,
    /// Index into `states` for the currently active state. Everything in
    /// `steps[cursor..]` has been undone and is kept only so `redo` can
    /// restore it; a fresh `execute` call discards it.
    cursor: usize,
    /// True between [`Engine::begin_group`] and [`Engine::end_group`]: new
    /// commands extend `steps[cursor - 1]` instead of starting a new step.
    group_open: bool,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    /// Creates a fresh engine: no document, default palette, empty history.
    pub fn new() -> Self {
        Self {
            states: vec![EngineState::default()],
            steps: Vec::new(),
            cursor: 0,
            group_open: false,
        }
    }

    /// The current document, if `canvas.new` has been executed.
    pub fn document(&self) -> Option<&Document> {
        self.states[self.cursor].document.as_ref()
    }

    /// The current palette.
    pub fn palette(&self) -> &[Color; PALETTE_SIZE] {
        &self.states[self.cursor].palette
    }

    /// The active drawing colour (set by `color.set` / `color.pick`).
    pub fn current_color(&self) -> Color {
        self.states[self.cursor].current_color
    }

    /// The active rectangular selection, if any.
    pub fn selection(&self) -> Option<&Selection> {
        self.states[self.cursor].selection.as_ref()
    }

    /// The commands currently in effect, flattened across undo steps, in
    /// execution order (excludes anything undone via [`Engine::undo`] that
    /// has not been redone). This is exactly what `.pxc` "Save script"
    /// serializes — grouping is an undo concern, never a file-format one.
    pub fn history(&self) -> Vec<Command> {
        self.steps[..self.cursor].iter().flatten().cloned().collect()
    }

    /// How many undo steps are currently in effect. Differs from
    /// `history().len()` whenever any step holds more than one command.
    pub fn step_count(&self) -> usize {
        self.cursor
    }

    /// Serializes [`Engine::history`] to `.pxc` text.
    pub fn save_script(&self) -> String {
        serialize_script(&self.history())
    }

    /// Starts a grouped undo step. Every command executed until
    /// [`Engine::end_group`] becomes part of one step and is undone as one.
    /// Nesting is not supported: an already-open group is closed first.
    pub fn begin_group(&mut self) {
        if self.group_open {
            self.end_group();
        }
        self.states.truncate(self.cursor + 1);
        self.steps.truncate(self.cursor);

        let carried = self.states[self.cursor].clone();
        self.states.push(carried);
        self.steps.push(Vec::new());
        self.cursor += 1;
        self.group_open = true;
    }

    /// Closes the current group. A group in which nothing succeeded leaves
    /// no undo step behind — clicking the canvas and drawing nothing must
    /// not create an Undo that appears to do nothing.
    pub fn end_group(&mut self) {
        if !self.group_open {
            return;
        }
        self.group_open = false;
        if self.steps[self.cursor - 1].is_empty() {
            self.states.pop();
            self.steps.pop();
            self.cursor -= 1;
        }
    }

    /// Whether a group is currently open.
    pub fn group_open(&self) -> bool {
        self.group_open
    }

    /// Executes a single command. Outside a group this creates its own undo
    /// step; inside one it extends the open step. Either way, any previously
    /// undone work beyond the cursor is discarded (standard undo/redo
    /// semantics: a new action clears the redo stack).
    ///
    /// On error nothing is mutated: the command is applied to a clone that
    /// is only committed on success.
    pub fn execute(&mut self, command: Command) -> Result<(), EngineError> {
        let mut next_state = self.states[self.cursor].clone();
        apply(&mut next_state, &command)?;

        if self.group_open {
            self.states[self.cursor] = next_state;
            self.steps[self.cursor - 1].push(command);
            return Ok(());
        }

        self.states.truncate(self.cursor + 1);
        self.steps.truncate(self.cursor);

        self.states.push(next_state);
        self.steps.push(vec![command]);
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
        !self.group_open && self.cursor < self.steps.len()
    }

    /// Moves one *step* back in history, if possible. Returns whether it
    /// moved. An open group is closed first, so undoing mid-drag is safe.
    pub fn undo(&mut self) -> bool {
        self.end_group();
        if self.cursor == 0 {
            return false;
        }
        self.cursor -= 1;
        true
    }

    /// Moves one *step* forward in history (restoring a previously undone
    /// step), if possible. Returns whether it moved.
    pub fn redo(&mut self) -> bool {
        self.end_group();
        if self.cursor >= self.steps.len() {
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
            let selection = state.selection.clone();
            let doc = state.document.as_mut().ok_or(EngineError::NoCanvas)?;
            write_strict(doc, selection.as_ref(), x, y, color)
        }
        Command::LineDraw { x0, y0, x1, y1, color } => {
            let selection = state.selection.clone();
            let doc = state.document.as_mut().ok_or(EngineError::NoCanvas)?;
            for (x, y) in bresenham_points(x0, y0, x1, y1) {
                write_strict(doc, selection.as_ref(), x, y, color)?;
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
        Command::ColorSet { color } => {
            state.current_color = color;
            Ok(())
        }
        Command::ColorPick { x, y } => {
            let doc = state.document.as_ref().ok_or(EngineError::NoCanvas)?;
            state.current_color = doc.composited_pixel(x, y)?;
            Ok(())
        }
        Command::SelectRect { x, y, width, height } => {
            state.selection = Some(Selection::rect(x, y, width, height));
            Ok(())
        }
        Command::SelectClear => {
            state.selection = None;
            Ok(())
        }
        Command::BrushStroke { x0, y0, x1, y1, size, shape, color } => {
            let selection = state.selection.clone();
            let doc = state.document.as_mut().ok_or(EngineError::NoCanvas)?;
            let size = size.max(1) as i64;
            // Centre the square on the path point. For even sizes the extra
            // pixel goes right/down; fixing the rule here (rather than
            // rounding) keeps the result identical on every platform.
            let lo = -((size - 1) / 2);
            let hi = size / 2;
            // Radius test in doubled coordinates so an even diameter has
            // no centre pixel to bias, and so the whole test is integer.
            let diameter = size;
            let limit = (diameter * diameter) as i64;
            for (px, py) in bresenham_points(x0, y0, x1, y1) {
                for dy in lo..=hi {
                    for dx in lo..=hi {
                        if shape == BrushShape::Round {
                            let ox = 2 * dx + if diameter % 2 == 0 { 1 } else { 0 };
                            let oy = 2 * dy + if diameter % 2 == 0 { 1 } else { 0 };
                            if ox * ox + oy * oy > limit {
                                continue;
                            }
                        }
                        write_lenient(doc, selection.as_ref(), px + dx, py + dy, color);
                    }
                }
            }
            Ok(())
        }
        Command::RectDraw { x0, y0, x1, y1, radius, fill, color } => {
            let selection = state.selection.clone();
            let doc = state.document.as_mut().ok_or(EngineError::NoCanvas)?;
            let (left, right) = (x0.min(x1), x0.max(x1));
            let (top, bottom) = (y0.min(y1), y0.max(y1));
            // Corner radius is clamped to half the shorter side: a larger
            // value would make opposite corners overlap and produce a shape
            // nobody asked for. Clamping degrades gracefully into a stadium
            // and then a circle.
            let max_radius = (((right - left).min(bottom - top)) / 2).max(0) as u32;
            let radius = radius.min(max_radius) as i64;

            for y in top..=bottom {
                for x in left..=right {
                    // Distance into the nearest corner box, if any.
                    let cx = if x < left + radius {
                        Some(left + radius)
                    } else if x > right - radius {
                        Some(right - radius)
                    } else {
                        None
                    };
                    let cy = if y < top + radius {
                        Some(top + radius)
                    } else if y > bottom - radius {
                        Some(bottom - radius)
                    } else {
                        None
                    };

                    let (inside, on_edge) = match (cx, cy) {
                        (Some(cx), Some(cy)) => {
                            let (dx, dy) = (x - cx, y - cy);
                            let d2 = dx * dx + dy * dy;
                            (d2 <= radius * radius, {
                                let inner = (radius - 1).max(0);
                                d2 <= radius * radius && d2 > inner * inner
                            })
                        }
                        _ => (true, x == left || x == right || y == top || y == bottom),
                    };

                    if !inside {
                        continue;
                    }
                    if fill || on_edge {
                        write_lenient(doc, selection.as_ref(), x, y, color);
                    }
                }
            }
            Ok(())
        }
        Command::FillBucket { x, y, tolerance, color } => {
            let selection = state.selection.clone();
            let doc = state.document.as_mut().ok_or(EngineError::NoCanvas)?;
            flood_fill(doc, selection.as_ref(), x, y, tolerance, color);
            Ok(())
        }
        Command::EllipseDraw { x0, y0, x1, y1, fill, color } => {
            let selection = state.selection.clone();
            let doc = state.document.as_mut().ok_or(EngineError::NoCanvas)?;
            for (x, y) in ellipse_points(x0, y0, x1, y1, fill) {
                write_lenient(doc, selection.as_ref(), x, y, color);
            }
            Ok(())
        }
        Command::PolygonDraw { x, y, radius, sides, rotation, fill, color } => {
            let selection = state.selection.clone();
            let doc = state.document.as_mut().ok_or(EngineError::NoCanvas)?;
            let vertices = regular_polygon(x, y, radius, sides, rotation);
            if vertices.len() < 3 {
                // Fewer than three sides has no shape to draw. Silently
                // drawing a line instead would be a worse surprise than
                // drawing nothing.
                return Ok(());
            }
            for (px, py) in polygon_points(&vertices, fill) {
                write_lenient(doc, selection.as_ref(), px, py, color);
            }
            Ok(())
        }
        Command::ArrowDraw { x0, y0, x1, y1, head, color } => {
            let selection = state.selection.clone();
            let doc = state.document.as_mut().ok_or(EngineError::NoCanvas)?;
            for (x, y) in bresenham_points(x0, y0, x1, y1) {
                write_lenient(doc, selection.as_ref(), x, y, color);
            }
            for (x, y) in arrow_head_points(x0, y0, x1, y1, head) {
                write_lenient(doc, selection.as_ref(), x, y, color);
            }
            Ok(())
        }
        Command::PolylineDraw { ref points, color } => {
            let selection = state.selection.clone();
            let doc = state.document.as_mut().ok_or(EngineError::NoCanvas)?;
            for pair in points.windows(2) {
                for (x, y) in bresenham_points(pair[0].0, pair[0].1, pair[1].0, pair[1].1) {
                    write_lenient(doc, selection.as_ref(), x, y, color);
                }
            }
            if points.len() == 1 {
                write_lenient(doc, selection.as_ref(), points[0].0, points[0].1, color);
            }
            Ok(())
        }
        Command::TextDraw { x, y, ref text, font, scale, align, color } => {
            let selection = state.selection.clone();
            let doc = state.document.as_mut().ok_or(EngineError::NoCanvas)?;
            crate::font::rasterize(text, font, scale, x, y, align, |px, py| {
                write_lenient(doc, selection.as_ref(), px, py, color);
            });
            Ok(())
        }

        Command::SelectAll => {
            let doc = state.document.as_ref().ok_or(EngineError::NoCanvas)?;
            state.selection = Some(Selection::all(doc.width(), doc.height()));
            Ok(())
        }
        Command::SelectLasso { ref points } => {
            state.selection = Some(Selection::from_polygon(points));
            Ok(())
        }
        Command::SelectionDelete => {
            let Some(selection) = state.selection.clone() else {
                return Err(EngineError::NoSelection);
            };
            let doc = state.document.as_mut().ok_or(EngineError::NoCanvas)?;
            for (x, y) in selection.cells() {
                if doc.in_bounds(x, y) {
                    let _ = doc.set_pixel(x, y, TRANSPARENT);
                }
            }
            Ok(())
        }
        Command::SelectionMove { dx, dy } => {
            let Some(selection) = state.selection.clone() else {
                return Err(EngineError::NoSelection);
            };
            let doc = state.document.as_mut().ok_or(EngineError::NoCanvas)?;
            // Lift, clear, then drop. Lifting everything *before* clearing
            // is what makes an overlapping move (the common case: nudging
            // by one pixel) correct rather than smearing the source across
            // its own destination.
            let lifted: Vec<(i64, i64, Color)> = selection
                .cells()
                .filter(|&(x, y)| doc.in_bounds(x, y))
                .map(|(x, y)| (x, y, doc.get_pixel(x, y).unwrap()))
                .collect();
            for &(x, y, _) in &lifted {
                let _ = doc.set_pixel(x, y, TRANSPARENT);
            }
            for (x, y, color) in lifted {
                write_lenient_unmasked(doc, x + dx, y + dy, color);
            }
            state.selection = Some(selection.translated(dx, dy));
            Ok(())
        }
        Command::SelectionFlip { axis } => {
            let Some(selection) = state.selection.clone() else {
                return Err(EngineError::NoSelection);
            };
            let doc = state.document.as_mut().ok_or(EngineError::NoCanvas)?;
            let (ox, oy, w, h) = selection.bounds();
            let block = lift_block(doc, &selection);
            clear_selection_pixels(doc, &selection);
            for row in 0..h as i64 {
                for col in 0..w as i64 {
                    let (sc, sr) = match axis {
                        Axis::Horizontal => (w as i64 - 1 - col, row),
                        Axis::Vertical => (col, h as i64 - 1 - row),
                    };
                    if let Some(color) = block[(sr * w as i64 + sc) as usize] {
                        write_lenient_unmasked(doc, ox + col, oy + row, color);
                    }
                }
            }
            state.selection = Some(match axis {
                Axis::Horizontal => selection.flipped_horizontally(),
                Axis::Vertical => selection.flipped_vertically(),
            });
            Ok(())
        }
        Command::SelectionRotate { degrees } => {
            let degrees = degrees % 360;
            if !matches!(degrees, 90 | 180 | 270) {
                if degrees == 0 {
                    return Ok(());
                }
                return Err(EngineError::UnsupportedRotation { degrees });
            }
            let Some(selection) = state.selection.clone() else {
                return Err(EngineError::NoSelection);
            };
            let doc = state.document.as_mut().ok_or(EngineError::NoCanvas)?;
            let (ox, oy, w, h) = selection.bounds();
            let block = lift_block(doc, &selection);
            clear_selection_pixels(doc, &selection);

            // 90 and 270 transpose the box. The result is anchored at the
            // same top-left corner rather than about the centre, so a
            // rotation never drifts off-canvas in a way the user did not
            // ask for.
            let (nw, nh) = if degrees == 180 { (w, h) } else { (h, w) };
            for row in 0..nh as i64 {
                for col in 0..nw as i64 {
                    let (sc, sr) = match degrees {
                        90 => (row, h as i64 - 1 - col),
                        180 => (w as i64 - 1 - col, h as i64 - 1 - row),
                        _ => (w as i64 - 1 - row, col),
                    };
                    if sc < 0 || sr < 0 || sc >= w as i64 || sr >= h as i64 {
                        continue;
                    }
                    if let Some(color) = block[(sr * w as i64 + sc) as usize] {
                        write_lenient_unmasked(doc, ox + col, oy + row, color);
                    }
                }
            }
            // The rotated shape is a new shape; the marquee becomes the
            // rotated bounding box rather than a rotated mask.
            state.selection = Some(Selection::rect(ox, oy, nw, nh));
            Ok(())
        }
        Command::SelectionScale { width, height } => {
            if width == 0 || height == 0 {
                return Err(EngineError::Document(DocumentError::ZeroSize { width, height }));
            }
            let Some(selection) = state.selection.clone() else {
                return Err(EngineError::NoSelection);
            };
            let doc = state.document.as_mut().ok_or(EngineError::NoCanvas)?;
            let (ox, oy, w, h) = selection.bounds();
            if w == 0 || h == 0 {
                return Ok(());
            }
            let block = lift_block(doc, &selection);
            clear_selection_pixels(doc, &selection);
            for row in 0..height as usize {
                let sr = row * h as usize / height as usize;
                for col in 0..width as usize {
                    let sc = col * w as usize / width as usize;
                    if let Some(color) = block[sr * w as usize + sc] {
                        write_lenient_unmasked(doc, ox + col as i64, oy + row as i64, color);
                    }
                }
            }
            state.selection = Some(Selection::rect(ox, oy, width, height));
            Ok(())
        }

        Command::CanvasCrop { x, y, width, height } => {
            let doc = state.document.as_mut().ok_or(EngineError::NoCanvas)?;
            doc.crop(x, y, width, height)?;
            // A selection in old coordinates is meaningless after a crop.
            state.selection = None;
            Ok(())
        }
        Command::CanvasResize { width, height } => {
            let doc = state.document.as_mut().ok_or(EngineError::NoCanvas)?;
            doc.resize(width, height)?;
            state.selection = None;
            Ok(())
        }
        Command::LayerDuplicate { index } => {
            let doc = state.document.as_mut().ok_or(EngineError::NoCanvas)?;
            doc.duplicate_layer(index)?;
            Ok(())
        }
        Command::LayerMerge { index } => {
            let doc = state.document.as_mut().ok_or(EngineError::NoCanvas)?;
            doc.merge_layer_down(index)?;
            Ok(())
        }
        Command::ImageImport { x, y, width, height, ref data } => {
            let selection = state.selection.clone();
            let doc = state.document.as_mut().ok_or(EngineError::NoCanvas)?;
            let pixels = crate::base64::decode(data)?;
            let expected = width as usize * height as usize * 4;
            if pixels.len() != expected {
                return Err(EngineError::ImageDataLength { expected, found: pixels.len() });
            }
            for row in 0..height as i64 {
                for col in 0..width as i64 {
                    let i = ((row * width as i64 + col) * 4) as usize;
                    let color = [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]];
                    write_lenient(doc, selection.as_ref(), x + col, y + row, color);
                }
            }
            Ok(())
        }
    }
}

/// 4-connected flood fill on the active layer.
///
/// Deterministic by construction: an explicit LIFO stack (no recursion, so
/// no stack overflow on a large canvas), a fixed neighbour order, and a
/// `visited` bitmap keyed by linear pixel index. The target colour is the
/// active layer's *own* pixel at the seed, not the composite — filling acts
/// on the layer you are drawing on, which is what every pixel editor does.
///
/// Terminates in every case, including the degenerate one where the fill
/// colour already equals the target colour (the `visited` set, not a colour
/// comparison, is what bounds the walk).
fn flood_fill(
    doc: &mut Document,
    selection: Option<&Selection>,
    seed_x: i64,
    seed_y: i64,
    tolerance: u8,
    color: Color,
) {
    if !doc.in_bounds(seed_x, seed_y) {
        return;
    }
    let target = match doc.get_pixel(seed_x, seed_y) {
        Ok(c) => c,
        Err(_) => return,
    };

    let width = doc.width() as i64;
    let height = doc.height() as i64;
    let mut visited = vec![false; (width * height) as usize];
    let mut stack = vec![(seed_x, seed_y)];

    while let Some((x, y)) = stack.pop() {
        if x < 0 || y < 0 || x >= width || y >= height {
            continue;
        }
        let idx = (y * width + x) as usize;
        if visited[idx] {
            continue;
        }
        visited[idx] = true;

        // A selection is a hard wall, not just a write mask: the fill region
        // is bounded by the marquee, so it cannot leak around it and
        // reappear elsewhere inside the selection.
        if let Some(rect) = selection {
            if !rect.contains(x, y) {
                continue;
            }
        }
        match doc.get_pixel(x, y) {
            Ok(here) if within_tolerance(here, target, tolerance) => {}
            _ => continue,
        }
        write_lenient(doc, selection, x, y, color);

        // Fixed neighbour order: right, left, down, up.
        stack.push((x + 1, y));
        stack.push((x - 1, y));
        stack.push((x, y + 1));
        stack.push((x, y - 1));
    }
}

/// Writes one pixel, honouring the selection, and **erroring** if the
/// coordinate is outside the canvas.
///
/// Used by the precise commands (`pixel.set`, `line.draw`) where an
/// out-of-canvas coordinate is a genuine authoring mistake worth reporting,
/// and where Phase 0 already behaved this way.
fn write_strict(
    doc: &mut Document,
    selection: Option<&Selection>,
    x: i64,
    y: i64,
    color: Color,
) -> Result<(), EngineError> {
    if let Some(rect) = selection {
        if !rect.contains(x, y) {
            // Masked out by the selection. Not an error: this is exactly
            // what a marquee is for.
            return Ok(());
        }
    }
    doc.set_pixel(x, y, color)?;
    Ok(())
}

/// Writes one pixel, honouring the selection, and **silently skipping**
/// coordinates outside the canvas.
///
/// Used by the area commands (`brush.stroke`, `rect.draw`, `fill.bucket`,
/// `image.import`) whose footprint legitimately spills past the edge — a
/// size-5 brush dragged along the border must not abort the whole stroke.
fn write_lenient(
    doc: &mut Document,
    selection: Option<&Selection>,
    x: i64,
    y: i64,
    color: Color,
) {
    if !doc.in_bounds(x, y) {
        return;
    }
    if let Some(rect) = selection {
        if !rect.contains(x, y) {
            return;
        }
    }
    let _ = doc.set_pixel(x, y, color);
}

/// Computes every integer point on the line from `(x0, y0)` to `(x1, y1)`
/// inclusive, using Bresenham's algorithm. Deterministic: always produces
/// the same point sequence for the same inputs, on every platform.
pub(crate) fn bresenham_points(x0: i64, y0: i64, x1: i64, y1: i64) -> Vec<(i64, i64)> {
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


/// Writes a pixel honouring only the canvas bounds, ignoring the selection.
///
/// Selection-aware writes are the norm, but the selection *transform*
/// commands are the exception: they move the selected pixels, so their
/// destination is by definition outside the old marquee. Clipping them to it
/// would delete exactly the pixels the user asked to move.
fn write_lenient_unmasked(doc: &mut Document, x: i64, y: i64, color: Color) {
    if doc.in_bounds(x, y) {
        let _ = doc.set_pixel(x, y, color);
    }
}

/// Copies the selection's bounding box off the active layer.
///
/// Cells outside the mask come back as `None` so a later write can skip them
/// and leave whatever is underneath intact — a rotated lasso selection must
/// not stamp a rectangle of transparency over its neighbours.
fn lift_block(doc: &Document, selection: &Selection) -> Vec<Option<Color>> {
    let (ox, oy, w, h) = selection.bounds();
    let mut block = vec![None; w as usize * h as usize];
    for row in 0..h as i64 {
        for col in 0..w as i64 {
            let (x, y) = (ox + col, oy + row);
            if selection.contains(x, y) && doc.in_bounds(x, y) {
                block[(row * w as i64 + col) as usize] = doc.get_pixel(x, y).ok();
            }
        }
    }
    block
}

/// Clears every selected pixel on the active layer to transparency.
fn clear_selection_pixels(doc: &mut Document, selection: &Selection) {
    for (x, y) in selection.cells() {
        if doc.in_bounds(x, y) {
            let _ = doc.set_pixel(x, y, TRANSPARENT);
        }
    }
}

/// Whether two colours are within `tolerance` on every channel.
///
/// Per-channel maximum difference, not Euclidean distance: it is the metric
/// a user can predict from a hex value, it needs no square root, and it is
/// exact in integers.
fn within_tolerance(a: Color, b: Color, tolerance: u8) -> bool {
    a.iter().zip(b.iter()).all(|(x, y)| x.abs_diff(*y) <= tolerance)
}

/// The points of an axis-aligned ellipse inscribed in a bounding box.
///
/// Uses the midpoint (Bresenham) ellipse algorithm in integer arithmetic, so
/// it is exact and identical on every platform. Even-sized boxes have no
/// single centre pixel, which the doubled-coordinate form handles naturally.
fn ellipse_points(x0: i64, y0: i64, x1: i64, y1: i64, fill: bool) -> Vec<(i64, i64)> {
    let (left, right) = (x0.min(x1), x0.max(x1));
    let (top, bottom) = (y0.min(y1), y0.max(y1));
    let (w, h) = (right - left, bottom - top);
    let mut out = Vec::new();

    // Degenerate boxes are a line, which is what a user dragging a
    // zero-height ellipse expects to see.
    if w == 0 || h == 0 {
        for y in top..=bottom {
            for x in left..=right {
                out.push((x, y));
            }
        }
        return out;
    }

    let a = w as f64 / 2.0;
    let b = h as f64 / 2.0;
    let _ = (a, b); // documented below: no float is used in the loop

    // Integer form: for each row, solve the ellipse equation for the half
    // width using only integer multiplication, then emit the span. Scanning
    // by row makes the filled and outlined cases share one traversal and
    // guarantees a closed outline with no gaps, which the classic
    // eight-way midpoint plot does not for very flat ellipses.
    let (cx2, cy2) = (left + right, top + bottom); // doubled centre
    let (rx, ry) = (w, h); // doubled radii
    for y in top..=bottom {
        let dy = 2 * y - cy2;
        // (dx/rx)^2 + (dy/ry)^2 <= 1  ->  dx^2 * ry^2 <= (ry^2 - dy^2) * rx^2
        let rem = (ry * ry) - (dy * dy);
        if rem < 0 {
            continue;
        }
        let max_dx2 = rem * rx * rx / (ry * ry);
        let mut half = 0i64;
        while (2 * half + if rx % 2 == 0 { 0 } else { 1 }).pow(2) <= max_dx2 {
            half += 1;
        }
        let span = half.max(1) - 1;
        let start = (cx2 / 2) - span;
        let end = (cx2 - cx2 / 2) + span;
        let (start, end) = (start.max(left), end.min(right));
        if fill {
            for x in start..=end {
                out.push((x, y));
            }
        } else {
            // Outline: the first and last row are solid caps, otherwise
            // only the two edge pixels.
            let is_cap = y == top || y == bottom;
            if is_cap {
                for x in start..=end {
                    out.push((x, y));
                }
            } else {
                out.push((start, y));
                if end != start {
                    out.push((end, y));
                }
            }
        }
    }
    out
}

/// The vertices of a regular polygon, computed with an integer-friendly
/// fixed-point cosine table so the result is bit-identical everywhere.
fn regular_polygon(cx: i64, cy: i64, radius: u32, sides: u32, rotation: i64) -> Vec<(i64, i64)> {
    if sides < 3 || radius == 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(sides as usize);
    for i in 0..sides {
        // Angle in whole degrees keeps the input exact and the lookup
        // deterministic; the table itself is fixed point.
        let degrees = rotation + (360 * i as i64) / sides as i64;
        let (sin, cos) = sin_cos_fixed(degrees);
        let r = radius as i64;
        out.push((cx + (r * cos) / FIXED_ONE, cy + (r * sin) / FIXED_ONE));
    }
    out
}

/// Scale of the fixed-point trigonometry used by [`regular_polygon`].
const FIXED_ONE: i64 = 10_000;

/// Sine and cosine of a whole number of degrees, in `FIXED_ONE` units.
///
/// A 91-entry quarter table plus quadrant reflection: no floating point at
/// run time, so a polygon is bit-identical on every platform and every run,
/// which `f64::sin` cannot promise across targets.
fn sin_cos_fixed(degrees: i64) -> (i64, i64) {
    /// sin(0..=90 degrees) * 10000.
    static SIN_TABLE: [i64; 91] = [
        0, 175, 349, 523, 698, 872, 1045, 1219, 1392, 1564, 1736, 1908, 2079, 2250, 2419, 2588,
        2756, 2924, 3090, 3256, 3420, 3584, 3746, 3907, 4067, 4226, 4384, 4540, 4695, 4848, 5000,
        5150, 5299, 5446, 5592, 5736, 5878, 6018, 6157, 6293, 6428, 6561, 6691, 6820, 6947, 7071,
        7193, 7314, 7431, 7547, 7660, 7771, 7880, 7986, 8090, 8192, 8290, 8387, 8480, 8572, 8660,
        8746, 8829, 8910, 8988, 9063, 9135, 9205, 9272, 9336, 9397, 9455, 9511, 9563, 9613, 9659,
        9703, 9744, 9781, 9816, 9848, 9877, 9903, 9925, 9945, 9962, 9976, 9986, 9994, 9998, 10000,
    ];
    fn sin_of(d: i64) -> i64 {
        let d = d.rem_euclid(360);
        match d {
            0..=90 => SIN_TABLE[d as usize],
            91..=180 => SIN_TABLE[(180 - d) as usize],
            181..=270 => -SIN_TABLE[(d - 180) as usize],
            _ => -SIN_TABLE[(360 - d) as usize],
        }
    }
    (sin_of(degrees), sin_of(degrees + 90))
}

/// Scan-converts a closed polygon, outlined or filled. Shares its rule with
/// [`Selection::from_polygon`] so a lasso and a drawn polygon of the same
/// points cover the same pixels.
fn polygon_points(vertices: &[(i64, i64)], fill: bool) -> Vec<(i64, i64)> {
    let mut out = Vec::new();
    if fill {
        let selection = Selection::from_polygon(vertices);
        out.extend(selection.cells());
    } else {
        for i in 0..vertices.len() {
            let (x0, y0) = vertices[i];
            let (x1, y1) = vertices[(i + 1) % vertices.len()];
            out.extend(bresenham_points(x0, y0, x1, y1));
        }
    }
    out
}

/// The filled triangular head of an arrow pointing at `(x1, y1)`.
fn arrow_head_points(x0: i64, y0: i64, x1: i64, y1: i64, head: u32) -> Vec<(i64, i64)> {
    if head == 0 {
        return Vec::new();
    }
    let (dx, dy) = (x1 - x0, y1 - y0);
    if dx == 0 && dy == 0 {
        return Vec::new();
    }
    // Integer approximation of the unit vector, scaled by FIXED_ONE. An
    // integer square root keeps this deterministic.
    let len = isqrt((dx * dx + dy * dy) as u64) as i64;
    let len = len.max(1);
    let (ux, uy) = (dx * FIXED_ONE / len, dy * FIXED_ONE / len);
    let h = head as i64;
    // Base of the head, h back along the shaft.
    let bx = x1 - ux * h / FIXED_ONE;
    let by = y1 - uy * h / FIXED_ONE;
    // Perpendicular, half as wide as the head is long.
    let (px, py) = (-uy, ux);
    let half = h / 2;
    let left = (bx + px * half / FIXED_ONE, by + py * half / FIXED_ONE);
    let right = (bx - px * half / FIXED_ONE, by - py * half / FIXED_ONE);
    polygon_points(&[(x1, y1), left, right], true)
}

/// Integer square root (Newton), so no float enters the geometry path.
fn isqrt(n: u64) -> u64 {
    if n < 2 {
        return n;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
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
    fn color_set_and_pick_drive_the_active_colour() {
        let mut engine = Engine::new();
        assert_eq!(engine.current_color(), DEFAULT_PALETTE[0]);

        engine.execute(Command::CanvasNew { width: 4, height: 4 }).unwrap();
        engine.execute(Command::ColorSet { color: [10, 20, 30, 255] }).unwrap();
        assert_eq!(engine.current_color(), [10, 20, 30, 255]);

        engine.execute(Command::PixelSet { x: 2, y: 2, color: [99, 88, 77, 255] }).unwrap();
        engine.execute(Command::ColorPick { x: 2, y: 2 }).unwrap();
        assert_eq!(engine.current_color(), [99, 88, 77, 255], "eyedropper reads the canvas");

        engine.execute(Command::ColorPick { x: 0, y: 0 }).unwrap();
        assert_eq!(engine.current_color(), [0, 0, 0, 0], "picking empty canvas yields transparent");
    }

    #[test]
    fn color_pick_reads_the_composite_not_the_active_layer() {
        let mut engine = Engine::new();
        engine.execute(Command::CanvasNew { width: 1, height: 1 }).unwrap();
        engine.execute(Command::PixelSet { x: 0, y: 0, color: [7, 8, 9, 255] }).unwrap();
        engine.execute(Command::LayerAdd { name: "top".to_string() }).unwrap();
        // The active layer is empty here; the eyedropper must still see what
        // the user sees.
        engine.execute(Command::ColorPick { x: 0, y: 0 }).unwrap();
        assert_eq!(engine.current_color(), [7, 8, 9, 255]);
    }

    #[test]
    fn color_pick_out_of_bounds_errors() {
        let mut engine = Engine::new();
        engine.execute(Command::CanvasNew { width: 2, height: 2 }).unwrap();
        assert!(matches!(
            engine.execute(Command::ColorPick { x: 5, y: 0 }),
            Err(EngineError::Document(DocumentError::OutOfBounds { .. }))
        ));
    }

    #[test]
    fn selection_clips_pixel_and_line_writes() {
        let mut engine = Engine::new();
        engine.execute(Command::CanvasNew { width: 8, height: 8 }).unwrap();
        engine.execute(Command::SelectRect { x: 2, y: 2, width: 3, height: 3 }).unwrap();

        let ink = [255, 0, 0, 255];
        // A line crossing the whole canvas through the selection band.
        engine.execute(Command::LineDraw { x0: 0, y0: 3, x1: 7, y1: 3, color: ink }).unwrap();

        let doc = engine.document().unwrap();
        for x in 0..8i64 {
            let expected = if (2..5).contains(&x) { ink } else { [0, 0, 0, 0] };
            assert_eq!(doc.get_pixel(x, 3).unwrap(), expected, "at x={x}");
        }

        // A single pixel.set outside the marquee is masked, not an error.
        engine.execute(Command::PixelSet { x: 7, y: 7, color: ink }).unwrap();
        assert_eq!(engine.document().unwrap().get_pixel(7, 7).unwrap(), [0, 0, 0, 0]);

        // Clearing the selection restores unrestricted drawing.
        engine.execute(Command::SelectClear).unwrap();
        assert_eq!(engine.selection(), None);
        engine.execute(Command::PixelSet { x: 7, y: 7, color: ink }).unwrap();
        assert_eq!(engine.document().unwrap().get_pixel(7, 7).unwrap(), ink);
    }

    #[test]
    fn out_of_canvas_writes_still_error_for_precise_commands() {
        let mut engine = Engine::new();
        engine.execute(Command::CanvasNew { width: 4, height: 4 }).unwrap();
        assert!(matches!(
            engine.execute(Command::PixelSet { x: 9, y: 0, color: [1, 1, 1, 1] }),
            Err(EngineError::Document(DocumentError::OutOfBounds { .. }))
        ));
    }

    #[test]
    fn undo_restores_the_previous_selection_and_colour() {
        let mut engine = Engine::new();
        engine.execute(Command::CanvasNew { width: 4, height: 4 }).unwrap();
        engine.execute(Command::ColorSet { color: [1, 2, 3, 4] }).unwrap();
        engine.execute(Command::SelectRect { x: 0, y: 0, width: 2, height: 2 }).unwrap();
        assert!(engine.selection().is_some());

        assert!(engine.undo());
        assert_eq!(engine.selection(), None, "selection is snapshotted state");
        assert_eq!(engine.current_color(), [1, 2, 3, 4]);

        assert!(engine.undo());
        assert_eq!(engine.current_color(), DEFAULT_PALETTE[0]);
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

    #[test]
    fn image_import_stamps_a_block_at_an_offset() {
        let mut e = canvas(6);
        // A 2x2 block: red, green / blue, transparent.
        let data = crate::base64::encode(&[
            255, 0, 0, 255, //
            0, 255, 0, 255, //
            0, 0, 255, 255, //
            0, 0, 0, 0,
        ]);
        e.execute(Command::ImageImport { x: 2, y: 3, width: 2, height: 2, data }).unwrap();

        let doc = e.document().unwrap();
        assert_eq!(doc.get_pixel(2, 3).unwrap(), [255, 0, 0, 255]);
        assert_eq!(doc.get_pixel(3, 3).unwrap(), [0, 255, 0, 255]);
        assert_eq!(doc.get_pixel(2, 4).unwrap(), [0, 0, 255, 255]);
        assert_eq!(doc.get_pixel(3, 4).unwrap(), [0, 0, 0, 0]);
        assert_eq!(doc.get_pixel(0, 0).unwrap(), [0, 0, 0, 0], "nothing else touched");
    }

    #[test]
    fn image_import_clips_at_the_canvas_edge() {
        let mut e = canvas(2);
        let data = crate::base64::encode(&[9u8; 4 * 4]); // 2x2 block
        e.execute(Command::ImageImport { x: 1, y: 1, width: 2, height: 2, data }).unwrap();
        assert_eq!(painted(&e), vec![(1, 1)], "only the on-canvas corner lands");
    }

    #[test]
    fn image_import_rejects_a_payload_of_the_wrong_length() {
        let mut e = canvas(4);
        let data = crate::base64::encode(&[0u8; 8]); // claims 2x2 = 16 bytes
        let err = e
            .execute(Command::ImageImport { x: 0, y: 0, width: 2, height: 2, data })
            .unwrap_err();
        assert_eq!(err, EngineError::ImageDataLength { expected: 16, found: 8 });
        assert_eq!(painted(&e).len(), 0, "a rejected import writes nothing");
    }

    #[test]
    fn image_import_rejects_corrupt_base64() {
        let mut e = canvas(4);
        let err = e
            .execute(Command::ImageImport {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
                data: "not*valid".to_string(),
            })
            .unwrap_err();
        assert!(matches!(err, EngineError::Base64(_)), "got: {err:?}");
    }

    #[test]
    fn a_group_is_one_undo_step_but_many_commands_in_the_script() {
        let mut e = canvas(8);
        assert_eq!(e.step_count(), 1, "canvas.new is its own step");

        e.begin_group();
        for x in 0..5 {
            e.execute(Command::BrushStroke { x0: x, y0: 0, x1: x, y1: 0, size: 1, shape: BrushShape::Square, color: INK })
                .unwrap();
        }
        e.end_group();

        assert_eq!(e.step_count(), 2, "the whole drag is one undo step");
        assert_eq!(e.history().len(), 6, "but all six commands are in the script");
        assert_eq!(painted(&e).len(), 5);

        assert!(e.undo());
        assert_eq!(painted(&e).len(), 0, "one undo removes the entire stroke");
        assert_eq!(e.history().len(), 1);

        assert!(e.redo());
        assert_eq!(painted(&e).len(), 5, "one redo restores the entire stroke");
        assert_eq!(e.history().len(), 6);
    }

    #[test]
    fn an_empty_group_leaves_no_undo_step() {
        let mut e = canvas(4);
        let before = e.step_count();
        e.begin_group();
        e.end_group();
        assert_eq!(e.step_count(), before, "a group that did nothing is not undoable");
        assert!(!e.can_redo());
    }

    #[test]
    fn a_group_whose_commands_all_failed_leaves_no_undo_step() {
        let mut e = canvas(4);
        let before = e.step_count();
        e.begin_group();
        assert!(e.execute(Command::PixelSet { x: 99, y: 99, color: INK }).is_err());
        e.end_group();
        assert_eq!(e.step_count(), before);
    }

    #[test]
    fn end_group_is_idempotent_and_begin_group_closes_a_dangling_one() {
        let mut e = canvas(4);
        e.begin_group();
        e.execute(Command::PixelSet { x: 0, y: 0, color: INK }).unwrap();
        // No end_group: starting another group must close this one cleanly.
        e.begin_group();
        e.execute(Command::PixelSet { x: 1, y: 0, color: INK }).unwrap();
        e.end_group();
        e.end_group(); // idempotent

        assert_eq!(e.step_count(), 3, "canvas.new + two groups");
        assert!(e.undo());
        assert_eq!(painted(&e), vec![(0, 0)]);
    }

    #[test]
    fn undo_during_an_open_group_closes_it_first() {
        let mut e = canvas(4);
        e.begin_group();
        e.execute(Command::PixelSet { x: 0, y: 0, color: INK }).unwrap();
        assert!(e.group_open());

        assert!(e.undo());
        assert!(!e.group_open());
        assert_eq!(painted(&e).len(), 0, "the in-progress stroke was undone as one step");
    }

    #[test]
    fn a_new_group_after_undo_clears_the_redo_stack() {
        let mut e = canvas(4);
        e.begin_group();
        e.execute(Command::PixelSet { x: 0, y: 0, color: INK }).unwrap();
        e.end_group();
        e.undo();
        assert!(e.can_redo());

        e.begin_group();
        e.execute(Command::PixelSet { x: 1, y: 1, color: INK }).unwrap();
        e.end_group();
        assert!(!e.can_redo(), "a new action discards the redo branch");
        assert_eq!(painted(&e), vec![(1, 1)]);
    }

    #[test]
    fn grouped_history_still_round_trips_through_the_parser() {
        let mut e = canvas(8);
        e.begin_group();
        e.execute(Command::BrushStroke { x0: 0, y0: 0, x1: 4, y1: 4, size: 2, shape: BrushShape::Square, color: INK })
            .unwrap();
        e.execute(Command::FillBucket { x: 7, y: 7, tolerance: 0, color: INK }).unwrap();
        e.end_group();

        let reparsed = parse_script(&e.save_script()).unwrap();
        assert_eq!(reparsed, e.history(), "grouping must not leak into the file format");
    }

    /// Convenience: a fresh engine with an `n x n` canvas.
    fn canvas(n: u32) -> Engine {
        let mut e = Engine::new();
        e.execute(Command::CanvasNew { width: n, height: n }).unwrap();
        e
    }

    /// Every non-transparent pixel of the active layer, sorted — a compact,
    /// order-independent way to assert an exact drawing result.
    fn painted(engine: &Engine) -> Vec<(i64, i64)> {
        let doc = engine.document().unwrap();
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

    const INK: Color = [0x11, 0x22, 0x33, 0xff];

    #[test]
    fn brush_size_one_behaves_like_the_pencil() {
        let mut e = canvas(8);
        e.execute(Command::BrushStroke { x0: 1, y0: 1, x1: 3, y1: 1, size: 1, shape: BrushShape::Square, color: INK })
            .unwrap();
        assert_eq!(painted(&e), vec![(1, 1), (2, 1), (3, 1)]);
    }

    #[test]
    fn brush_size_three_paints_a_three_by_three_block_per_point() {
        let mut e = canvas(8);
        e.execute(Command::BrushStroke { x0: 4, y0: 4, x1: 4, y1: 4, size: 3, shape: BrushShape::Square, color: INK })
            .unwrap();
        let mut expected = Vec::new();
        for y in 3..=5 {
            for x in 3..=5 {
                expected.push((x, y));
            }
        }
        assert_eq!(painted(&e), expected);
    }

    #[test]
    fn a_wide_brush_at_the_canvas_edge_clips_instead_of_failing() {
        let mut e = canvas(4);
        // Centred on (0,0) with size 5, most of the footprint is off-canvas.
        e.execute(Command::BrushStroke { x0: 0, y0: 0, x1: 0, y1: 0, size: 5, shape: BrushShape::Square, color: INK })
            .unwrap();
        assert_eq!(painted(&e), vec![(0, 0), (1, 0), (2, 0), (0, 1), (1, 1), (2, 1), (0, 2), (1, 2), (2, 2)]);
    }

    #[test]
    fn the_eraser_is_a_transparent_brush_stroke() {
        let mut e = canvas(4);
        e.execute(Command::RectDraw { x0: 0, y0: 0, x1: 3, y1: 3, radius: 0, fill: true, color: INK })
            .unwrap();
        assert_eq!(painted(&e).len(), 16);

        e.execute(Command::BrushStroke {
            x0: 1,
            y0: 1,
            x1: 2,
            y1: 1,
            size: 1, shape: BrushShape::Square,
            color: crate::document::TRANSPARENT,
        })
        .unwrap();
        assert_eq!(e.document().unwrap().get_pixel(1, 1).unwrap(), [0, 0, 0, 0]);
        assert_eq!(e.document().unwrap().get_pixel(2, 1).unwrap(), [0, 0, 0, 0]);
        assert_eq!(painted(&e).len(), 14, "exactly two pixels erased");
    }

    #[test]
    fn outlined_rectangle_leaves_its_interior_empty() {
        let mut e = canvas(6);
        e.execute(Command::RectDraw { x0: 1, y0: 1, x1: 4, y1: 4, radius: 0, fill: false, color: INK })
            .unwrap();
        let doc = e.document().unwrap();
        assert_eq!(doc.get_pixel(1, 1).unwrap(), INK, "corner");
        assert_eq!(doc.get_pixel(4, 4).unwrap(), INK, "opposite corner");
        assert_eq!(doc.get_pixel(2, 1).unwrap(), INK, "top edge");
        assert_eq!(doc.get_pixel(2, 2).unwrap(), [0, 0, 0, 0], "interior stays empty");
        assert_eq!(painted(&e).len(), 12, "4x4 outline = 12 pixels");
    }

    #[test]
    fn filled_rectangle_differs_from_the_outline_by_exactly_its_interior() {
        let mut outline = canvas(6);
        outline
            .execute(Command::RectDraw { x0: 1, y0: 1, x1: 4, y1: 4, radius: 0, fill: false, color: INK })
            .unwrap();
        let mut filled = canvas(6);
        filled
            .execute(Command::RectDraw { x0: 1, y0: 1, x1: 4, y1: 4, radius: 0, fill: true, color: INK })
            .unwrap();

        let extra: Vec<_> = painted(&filled)
            .into_iter()
            .filter(|p| !painted(&outline).contains(p))
            .collect();
        assert_eq!(extra, vec![(2, 2), (3, 2), (2, 3), (3, 3)], "only the 2x2 interior");
    }

    #[test]
    fn rectangle_corners_may_be_given_in_any_order() {
        let mut a = canvas(6);
        a.execute(Command::RectDraw { x0: 1, y0: 1, x1: 4, y1: 4, radius: 0, fill: true, color: INK })
            .unwrap();
        let mut b = canvas(6);
        b.execute(Command::RectDraw { x0: 4, y0: 4, x1: 1, y1: 1, radius: 0, fill: true, color: INK })
            .unwrap();
        assert_eq!(a.document_hash(), b.document_hash());
    }

    #[test]
    fn flood_fill_stops_at_a_drawn_border() {
        let mut e = canvas(7);
        // A closed 5x5 box outline at (1,1)-(5,5); fill its interior.
        e.execute(Command::RectDraw { x0: 1, y0: 1, x1: 5, y1: 5, radius: 0, fill: false, color: INK })
            .unwrap();
        let paint = [0xff, 0x00, 0x00, 0xff];
        e.execute(Command::FillBucket { x: 3, y: 3, tolerance: 0, color: paint }).unwrap();

        let doc = e.document().unwrap();
        for y in 2..=4 {
            for x in 2..=4 {
                assert_eq!(doc.get_pixel(x, y).unwrap(), paint, "interior ({x},{y})");
            }
        }
        assert_eq!(doc.get_pixel(0, 0).unwrap(), [0, 0, 0, 0], "outside the box is untouched");
        assert_eq!(doc.get_pixel(6, 6).unwrap(), [0, 0, 0, 0]);
        assert_eq!(doc.get_pixel(1, 1).unwrap(), INK, "the border itself survives");
    }

    #[test]
    fn flood_fill_with_the_colour_already_present_terminates() {
        // The degenerate case: if termination depended on "did the pixel
        // change", this would loop forever.
        let mut e = canvas(16);
        e.execute(Command::FillBucket { x: 0, y: 0, tolerance: 0, color: [0, 0, 0, 0] }).unwrap();
        assert_eq!(painted(&e).len(), 0);
    }

    #[test]
    fn flood_fill_on_an_empty_canvas_fills_everything() {
        let mut e = canvas(4);
        e.execute(Command::FillBucket { x: 2, y: 2, tolerance: 0, color: INK }).unwrap();
        assert_eq!(painted(&e).len(), 16);
    }

    #[test]
    fn flood_fill_acts_on_the_active_layer_only() {
        let mut e = canvas(4);
        e.execute(Command::RectDraw { x0: 0, y0: 0, x1: 3, y1: 3, radius: 0, fill: true, color: INK })
            .unwrap();
        e.execute(Command::LayerAdd { name: "top".to_string() }).unwrap();
        // The top layer is empty, so the fill covers all of it even though
        // the composite underneath is solid.
        e.execute(Command::FillBucket { x: 0, y: 0, tolerance: 0, color: [1, 2, 3, 255] }).unwrap();
        let doc = e.document().unwrap();
        assert_eq!(doc.get_pixel_on(0, 0, 0).unwrap(), INK, "bottom layer untouched");
        assert_eq!(doc.get_pixel_on(1, 0, 0).unwrap(), [1, 2, 3, 255]);
    }

    #[test]
    fn selection_clips_a_brush_stroke_and_a_flood_fill() {
        // AC8: nothing may be written outside the marquee.
        let mut e = canvas(8);
        e.execute(Command::SelectRect { x: 2, y: 2, width: 3, height: 3 }).unwrap();

        e.execute(Command::BrushStroke { x0: 0, y0: 3, x1: 7, y1: 3, size: 3, shape: BrushShape::Square, color: INK })
            .unwrap();
        e.execute(Command::FillBucket { x: 3, y: 3, tolerance: 0, color: [9, 9, 9, 255] }).unwrap();

        for (x, y) in painted(&e) {
            assert!(
                (2..5).contains(&x) && (2..5).contains(&y),
                "pixel ({x},{y}) escaped the 3x3 selection"
            );
        }
        assert!(!painted(&e).is_empty(), "the selection should still have been painted");
    }

    #[test]
    fn a_fill_cannot_leak_around_the_selection_boundary() {
        let mut e = canvas(8);
        e.execute(Command::SelectRect { x: 0, y: 0, width: 2, height: 8 }).unwrap();
        e.execute(Command::FillBucket { x: 0, y: 0, tolerance: 0, color: INK }).unwrap();
        assert_eq!(painted(&e).len(), 16, "exactly the 2x8 selection");
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
