//! Pure application logic for `pixelcad-app`, kept free of any Slint type so
//! it can be unit-tested without a window, event loop, or platform backend.
//!
//! `main.rs` is the only place that touches Slint: it owns a `Controller`,
//! forwards UI callbacks into it, and re-renders after every mutation.
//!
//! The division of responsibility is deliberate and load-bearing:
//! - **Document state** (pixels, layers, palette, active colour, selection)
//!   lives in `pixelcad-core`'s `Engine` and is reached *only* by executing
//!   commands, so every user action is replayable.
//! - **View state** (pan, zoom, which swatch is highlighted, the active
//!   tool, brush size) lives here and is deliberately *not* recorded — it
//!   changes nothing about the document, and putting it in the command log
//!   would make scripts noisy and diffs meaningless.

use std::io;
use std::path::Path;

use pixelcad_core::{
    base64, open_project, serialize_project, Axis, BrushShape, Color, Command, Document, Engine,
    EngineError, Selection, PALETTE_SIZE,
};
use pixelcad_core::font::{Font, TextAlign};

/// Minimum and maximum zoom levels (document pixels per screen pixel block).
pub const MIN_ZOOM: u32 = 1;
pub const MAX_ZOOM: u32 = 32;

/// Brush size bounds, in document pixels per side.
pub const MIN_BRUSH: u32 = 1;
pub const MAX_BRUSH: u32 = 32;

/// The drawing tools exposed by the GUI.
///
/// Every tool maps onto commands the engine already has; none of them has a
/// private drawing path. `Eraser` in particular is `Brush` with a fully
/// transparent colour — see `Command::BrushStroke`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Brush,
    Eraser,
    Fill,
    Eyedropper,
    Line,
    Rectangle,
    Ellipse,
    Polyline,
    Arrow,
    Text,
    Select,
    Lasso,
}

impl Tool {
    /// The single-key shortcut for this tool.
    pub fn shortcut(self) -> char {
        match self {
            Tool::Brush => 'b',
            Tool::Eraser => 'e',
            Tool::Fill => 'g', // "g" for the bucket, as in every other editor
            Tool::Eyedropper => 'i',
            Tool::Line => 'l',
            Tool::Rectangle => 'r',
            Tool::Ellipse => 'o', // "o" for oval; "e" is taken by the eraser
            Tool::Polyline => 'p',
            Tool::Arrow => 'a',
            Tool::Text => 't',
            Tool::Select => 'm', // "m" for marquee
            Tool::Lasso => 'f',  // "f" for free-form
        }
    }

    /// Resolves a keyboard character to a tool, if any.
    pub fn from_shortcut(key: char) -> Option<Tool> {
        Tool::ALL.into_iter().find(|t| t.shortcut() == key.to_ascii_lowercase())
    }

    pub const ALL: [Tool; 12] = [
        Tool::Brush,
        Tool::Eraser,
        Tool::Fill,
        Tool::Eyedropper,
        Tool::Line,
        Tool::Rectangle,
        Tool::Ellipse,
        Tool::Polyline,
        Tool::Arrow,
        Tool::Text,
        Tool::Select,
        Tool::Lasso,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Tool::Brush => "Brush",
            Tool::Eraser => "Eraser",
            Tool::Fill => "Fill",
            Tool::Eyedropper => "Pick",
            Tool::Line => "Line",
            Tool::Rectangle => "Rect",
            Tool::Ellipse => "Ellipse",
            Tool::Polyline => "Polyline",
            Tool::Arrow => "Arrow",
            Tool::Text => "Text",
            Tool::Select => "Select",
            Tool::Lasso => "Lasso",
        }
    }

    /// Whether this tool commits on pointer release from an anchor point.
    fn is_anchored(self) -> bool {
        matches!(self, Tool::Line | Tool::Rectangle | Tool::Ellipse | Tool::Arrow | Tool::Select)
    }

    /// Whether this tool accumulates a free-form path while dragging.
    fn is_path(self) -> bool {
        matches!(self, Tool::Polyline | Tool::Lasso)
    }
}

/// What an in-progress pointer drag on the canvas is doing.
#[derive(Debug, Clone, Copy, PartialEq)]
enum DragMode {
    Idle,
    /// Free-hand painting (brush or eraser); carries the last document pixel
    /// touched so each new point is connected to it with a single
    /// `brush.stroke` (no gaps on a fast mouse move).
    Painting { last: (i64, i64) },
    /// A drag whose result depends only on its start and end points (line,
    /// rectangle, selection). Nothing is committed until the pointer is
    /// released, so the user can still adjust the end point.
    Anchored { origin: (i64, i64), current: (i64, i64) },
    /// Panning the view; carries the pointer position and pan offset at the
    /// moment the drag started.
    Panning { anchor_pointer: (f32, f32), anchor_pan: (f32, f32) },
}

/// What was last copied. Session state, deliberately **not** part of the
/// document or the command log: copying changes nothing, so recording it
/// would put noise in every script. Pasting *is* recorded, as an
/// `image.import`, so a session still replays exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clipboard {
    pub width: u32,
    pub height: u32,
    /// Packed RGBA8, row-major.
    pub pixels: Vec<u8>,
}

/// Owns the [`Engine`] plus all view/tool state for the GUI.
pub struct Controller {
    engine: Engine,
    pan: (f32, f32),
    zoom: u32,
    selected_palette_index: usize,
    tool: Tool,
    brush_size: u32,
    brush_shape: BrushShape,
    tolerance: u8,
    opacity: u8,
    text_buffer: String,
    /// Points accumulated by a free-form (polyline / lasso) drag.
    path: Vec<(i64, i64)>,
    clipboard: Option<Clipboard>,
    recent_colors: Vec<Color>,
    drag: DragMode,
}

impl Controller {
    /// Creates a controller with a fresh `width x height` canvas (recorded
    /// as the first `canvas.new` command in history).
    pub fn new(width: u32, height: u32) -> Self {
        let mut engine = Engine::new();
        engine
            .execute(Command::CanvasNew { width, height })
            .expect("initial canvas.new with caller-supplied dimensions must succeed");
        Self {
            engine,
            pan: (0.0, 0.0),
            zoom: 8,
            selected_palette_index: 0,
            tool: Tool::Brush,
            brush_size: 1,
            brush_shape: BrushShape::Square,
            tolerance: 0,
            opacity: 255,
            text_buffer: String::new(),
            path: Vec::new(),
            clipboard: None,
            recent_colors: Vec::new(),
            drag: DragMode::Idle,
        }
    }

    pub fn document(&self) -> &Document {
        self.engine.document().expect("Controller always keeps a canvas alive")
    }

    pub fn palette(&self) -> &[Color; PALETTE_SIZE] {
        self.engine.palette()
    }

    pub fn selected_palette_index(&self) -> usize {
        self.selected_palette_index
    }

    /// The colour the next stroke will use. This is the *engine's* colour,
    /// not the highlighted swatch, so an eyedropper pick takes effect
    /// immediately without disturbing the palette.
    pub fn current_color(&self) -> Color {
        self.engine.current_color()
    }

    pub fn tool(&self) -> Tool {
        self.tool
    }

    pub fn set_tool(&mut self, tool: Tool) {
        // Abandon any half-finished gesture so a tool switch mid-drag cannot
        // commit a stroke with the wrong tool's semantics.
        self.drag = DragMode::Idle;
        self.path.clear();
        self.tool = tool;
    }

    pub fn brush_size(&self) -> u32 {
        self.brush_size
    }

    pub fn set_brush_size(&mut self, size: u32) {
        self.brush_size = size.clamp(MIN_BRUSH, MAX_BRUSH);
    }

    pub fn increase_brush(&mut self) {
        self.set_brush_size(self.brush_size + 1);
    }

    pub fn decrease_brush(&mut self) {
        self.set_brush_size(self.brush_size.saturating_sub(1));
    }

    pub fn selection(&self) -> Option<&Selection> {
        self.engine.selection()
    }

    pub fn zoom(&self) -> u32 {
        self.zoom
    }

    pub fn pan(&self) -> (f32, f32) {
        self.pan
    }

    pub fn should_draw_grid(&self) -> bool {
        self.zoom >= crate::render::GRID_ZOOM_THRESHOLD
    }

    /// `canvas.new` is never undoable from the GUI: undo can only unwind
    /// work done on top of it.
    pub fn can_undo(&self) -> bool {
        self.engine.step_count() > 1
    }

    pub fn can_redo(&self) -> bool {
        self.engine.can_redo()
    }

    // ------------------------------------------------------------- palette

    pub fn select_palette(&mut self, index: usize) {
        if index < PALETTE_SIZE {
            self.selected_palette_index = index;
            let color = self.palette()[index];
            // Selecting a swatch is a document-level fact (it changes the
            // active colour), so it is recorded.
            let _ = self.engine.execute(Command::ColorSet { color });
            self.remember_color(color);
        }
    }

    /// Replaces the highlighted swatch's colour and makes it current.
    /// Returns false if `hex` is not a valid `#rrggbb`/`#rrggbbaa` string.
    pub fn set_swatch_color(&mut self, hex: &str) -> bool {
        let Ok(color) = pixelcad_core::command::parse_hex_color(hex.trim()) else {
            return false;
        };
        let index = self.selected_palette_index as u8;
        self.engine.begin_group();
        let ok = self.engine.execute(Command::PaletteSet { index, color }).is_ok()
            && self.engine.execute(Command::ColorSet { color }).is_ok();
        self.engine.end_group();
        ok
    }

    // -------------------------------------------------------------- layers

    pub fn layer_names(&self) -> Vec<String> {
        self.document().layers().iter().map(|l| l.name().to_string()).collect()
    }

    pub fn active_layer_index(&self) -> usize {
        self.document().active_layer_index()
    }

    pub fn add_layer(&mut self) -> Result<(), EngineError> {
        let name = format!("Layer {}", self.document().layer_count() + 1);
        self.engine.execute(Command::LayerAdd { name })
    }

    pub fn select_layer(&mut self, index: usize) -> Result<(), EngineError> {
        self.engine.execute(Command::LayerSelect { index })
    }

    pub fn remove_active_layer(&mut self) -> Result<(), EngineError> {
        let index = self.active_layer_index();
        self.engine.execute(Command::LayerRemove { index })
    }

    pub fn toggle_layer_visible(&mut self, index: usize) -> Result<(), EngineError> {
        let current = self
            .document()
            .layers()
            .get(index)
            .map(|l| l.visible())
            .unwrap_or(true);
        self.engine.execute(Command::LayerVisible { index, value: !current })
    }

    pub fn layer_visible(&self, index: usize) -> bool {
        self.document().layers().get(index).map(|l| l.visible()).unwrap_or(false)
    }


    // ------------------------------------------------- brush / tool state

    pub fn brush_shape(&self) -> BrushShape {
        self.brush_shape
    }

    pub fn set_brush_shape(&mut self, shape: BrushShape) {
        self.brush_shape = shape;
    }

    pub fn tolerance(&self) -> u8 {
        self.tolerance
    }

    pub fn set_tolerance(&mut self, tolerance: u8) {
        self.tolerance = tolerance;
    }

    pub fn opacity(&self) -> u8 {
        self.opacity
    }

    /// Sets the stroke opacity.
    ///
    /// Because pixel writes *replace* rather than blend (which is what makes
    /// the eraser work), opacity is delivered as the alpha actually written,
    /// not as accumulating paint. Painting at 50 % twice therefore gives 50 %,
    /// not 75 %. True flow needs a blend mode in the write path and is in
    /// `BACKLOG.md`.
    pub fn set_opacity(&mut self, opacity: u8) {
        self.opacity = opacity;
    }

    pub fn text_buffer(&self) -> &str {
        &self.text_buffer
    }

    pub fn set_text_buffer(&mut self, text: impl Into<String>) {
        self.text_buffer = text.into();
    }

    /// Most recently used colours, newest first, capped at 8.
    pub fn recent_colors(&self) -> &[Color] {
        &self.recent_colors
    }

    fn remember_color(&mut self, color: Color) {
        self.recent_colors.retain(|c| *c != color);
        self.recent_colors.insert(0, color);
        self.recent_colors.truncate(8);
    }

    /// The active colour with the current opacity applied to its alpha.
    fn inked(&self) -> Color {
        let c = self.current_color();
        [c[0], c[1], c[2], ((c[3] as u32 * self.opacity as u32) / 255) as u8]
    }

    // ------------------------------------------------------- selection ops

    pub fn select_all(&mut self) -> Result<(), EngineError> {
        self.engine.execute(Command::SelectAll)
    }

    pub fn delete_selection(&mut self) -> Result<(), EngineError> {
        self.engine.execute(Command::SelectionDelete)
    }

    /// Moves the selected pixels by `(dx, dy)` — the arrow-key nudge.
    pub fn nudge(&mut self, dx: i64, dy: i64) -> Result<(), EngineError> {
        self.engine.execute(Command::SelectionMove { dx, dy })
    }

    pub fn flip_selection(&mut self, axis: Axis) -> Result<(), EngineError> {
        self.engine.execute(Command::SelectionFlip { axis })
    }

    pub fn rotate_selection(&mut self, degrees: u32) -> Result<(), EngineError> {
        self.engine.execute(Command::SelectionRotate { degrees })
    }

    pub fn scale_selection(&mut self, width: u32, height: u32) -> Result<(), EngineError> {
        self.engine.execute(Command::SelectionScale { width, height })
    }

    // ---------------------------------------------------------- clipboard

    pub fn clipboard(&self) -> Option<&Clipboard> {
        self.clipboard.as_ref()
    }

    /// Copies the selection from the active layer. With no selection, copies
    /// the whole layer — matching what every editor does with Ctrl+A implied.
    pub fn copy(&mut self) -> bool {
        self.capture(false)
    }

    /// Copies what is actually *visible* in the selected region, flattening
    /// every layer. This is Paint's `Ctrl+Shift+C`, and it is a genuinely
    /// different operation from `copy`, not a convenience alias.
    pub fn copy_composite(&mut self) -> bool {
        self.capture(true)
    }

    fn capture(&mut self, composite: bool) -> bool {
        let doc = self.document();
        let (ox, oy, w, h) = match self.engine.selection() {
            Some(s) if !s.is_empty() => s.bounds(),
            _ => (0, 0, doc.width(), doc.height()),
        };
        if w == 0 || h == 0 {
            return false;
        }
        let selection = self.engine.selection().cloned();
        let flat = composite.then(|| doc.composite());

        let mut pixels = Vec::with_capacity(w as usize * h as usize * 4);
        for row in 0..h as i64 {
            for col in 0..w as i64 {
                let (x, y) = (ox + col, oy + row);
                // Outside the mask reads as transparent, so a lasso copy
                // carries its shape rather than a rectangle of neighbours.
                let inside = selection.as_ref().is_none_or(|s| s.contains(x, y));
                let color = if !inside || !doc.in_bounds(x, y) {
                    [0, 0, 0, 0]
                } else if let Some(buf) = &flat {
                    let i = (y as usize * doc.width() as usize + x as usize) * 4;
                    [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
                } else {
                    doc.get_pixel(x, y).unwrap_or([0, 0, 0, 0])
                };
                pixels.extend_from_slice(&color);
            }
        }
        self.clipboard = Some(Clipboard { width: w, height: h, pixels });
        true
    }

    /// Copy, then clear the copied pixels.
    pub fn cut(&mut self) -> Result<(), EngineError> {
        if !self.copy() {
            return Ok(());
        }
        if self.engine.selection().is_some() {
            self.delete_selection()
        } else {
            Ok(())
        }
    }

    /// Pastes the clipboard at `(x, y)` and selects what was pasted.
    ///
    /// Emitted as `image.import` + `select.rect` in one undo group, so the
    /// paste is fully replayable from the saved script even though the
    /// clipboard itself never enters the command log.
    pub fn paste_at(&mut self, x: i64, y: i64) -> Result<(), EngineError> {
        let Some(clip) = self.clipboard.clone() else {
            return Ok(());
        };
        self.engine.begin_group();
        let result = self
            .engine
            .execute(Command::ImageImport {
                x,
                y,
                width: clip.width,
                height: clip.height,
                data: base64::encode(&clip.pixels),
            })
            .and_then(|()| {
                self.engine.execute(Command::SelectRect {
                    x,
                    y,
                    width: clip.width,
                    height: clip.height,
                })
            });
        self.engine.end_group();
        result
    }

    /// Pastes at the current selection's corner, or at the origin.
    pub fn paste(&mut self) -> Result<(), EngineError> {
        let (x, y) = match self.engine.selection() {
            Some(s) => (s.x(), s.y()),
            None => (0, 0),
        };
        self.paste_at(x, y)
    }

    /// Duplicates the selection in place, offset slightly so it is visible.
    pub fn duplicate_selection(&mut self) -> Result<(), EngineError> {
        if !self.copy() {
            return Ok(());
        }
        let (x, y) = match self.engine.selection() {
            Some(s) => (s.x(), s.y()),
            None => (0, 0),
        };
        self.paste_at(x + 1, y + 1)
    }

    // ------------------------------------------------- canvas / layer ops

    pub fn crop_to_selection(&mut self) -> Result<(), EngineError> {
        let Some(s) = self.engine.selection() else {
            return Err(EngineError::NoSelection);
        };
        let (x, y, width, height) = s.bounds();
        self.engine.execute(Command::CanvasCrop { x, y, width, height })
    }

    pub fn resize_canvas(&mut self, width: u32, height: u32) -> Result<(), EngineError> {
        self.engine.execute(Command::CanvasResize { width, height })
    }

    pub fn duplicate_active_layer(&mut self) -> Result<(), EngineError> {
        let index = self.active_layer_index();
        self.engine.execute(Command::LayerDuplicate { index })
    }

    pub fn merge_active_layer_down(&mut self) -> Result<(), EngineError> {
        let index = self.active_layer_index();
        self.engine.execute(Command::LayerMerge { index })
    }

    pub fn move_active_layer(&mut self, to: usize) -> Result<(), EngineError> {
        let from = self.active_layer_index();
        self.engine.execute(Command::LayerMove { from, to })
    }

    /// Stamps the current text buffer at `(x, y)`.
    pub fn stamp_text(&mut self, x: i64, y: i64) -> Result<(), EngineError> {
        if self.text_buffer.is_empty() {
            return Ok(());
        }
        let color = self.inked();
        let (text, scale) = (self.text_buffer.clone(), self.brush_size.max(1));
        self.engine.execute(Command::TextDraw {
            x,
            y,
            text,
            font: Font::Small,
            scale,
            align: TextAlign::Left,
            color,
        })
    }

    // ---------------------------------------------------------- view state

    pub fn zoom_in(&mut self) {
        self.zoom = (self.zoom + 1).min(MAX_ZOOM);
    }

    pub fn zoom_out(&mut self) {
        self.zoom = self.zoom.saturating_sub(1).max(MIN_ZOOM);
    }

    /// Interprets a raw scroll delta: positive `delta_y` zooms out, negative
    /// zooms in (matches the convention this crate's callers dispatch in
    /// tests; real trackpad/wheel sign varies by platform, but the mapping
    /// only needs to be internally consistent).
    pub fn handle_scroll(&mut self, delta_y: f32) {
        if delta_y < 0.0 {
            self.zoom_in();
        } else if delta_y > 0.0 {
            self.zoom_out();
        }
    }

    fn screen_to_doc_clamped(&self, screen_x: f32, screen_y: f32) -> (i64, i64) {
        let zoom = self.zoom as f32;
        let doc_x = ((screen_x - self.pan.0) / zoom).floor() as i64;
        let doc_y = ((screen_y - self.pan.1) / zoom).floor() as i64;
        let max_x = self.document().width() as i64 - 1;
        let max_y = self.document().height() as i64 - 1;
        (doc_x.clamp(0, max_x), doc_y.clamp(0, max_y))
    }

    // ------------------------------------------------------------ gestures

    /// Begins a pan gesture anchored at `(screen_x, screen_y)`.
    pub fn begin_pan(&mut self, screen_x: f32, screen_y: f32) {
        self.drag = DragMode::Panning { anchor_pointer: (screen_x, screen_y), anchor_pan: self.pan };
    }

    /// Begins whatever gesture the active tool implies at this point.
    ///
    /// Painting and one-shot tools open an undo group here and close it in
    /// [`Controller::end_drag`], so the whole gesture is a single Undo.
    pub fn begin_stroke(&mut self, screen_x: f32, screen_y: f32) -> Result<(), EngineError> {
        let (x, y) = self.screen_to_doc_clamped(screen_x, screen_y);
        match self.tool {
            Tool::Brush | Tool::Eraser => {
                let color = self.stroke_color();
                let (size, shape) = (self.brush_size, self.brush_shape);
                self.engine.begin_group();
                self.engine.execute(Command::BrushStroke {
                    x0: x,
                    y0: y,
                    x1: x,
                    y1: y,
                    size,
                    shape,
                    color,
                })?;
                self.drag = DragMode::Painting { last: (x, y) };
                Ok(())
            }
            Tool::Fill => {
                let (color, tolerance) = (self.inked(), self.tolerance);
                self.drag = DragMode::Idle;
                self.engine.execute(Command::FillBucket { x, y, tolerance, color })
            }
            Tool::Eyedropper => {
                self.drag = DragMode::Idle;
                self.engine.execute(Command::ColorPick { x, y })?;
                let picked = self.current_color();
                self.remember_color(picked);
                Ok(())
            }
            Tool::Text => {
                self.drag = DragMode::Idle;
                self.stamp_text(x, y)
            }
            tool if tool.is_path() => {
                // Free-form: accumulate points now, commit on release.
                self.path = vec![(x, y)];
                self.drag = DragMode::Anchored { origin: (x, y), current: (x, y) };
                Ok(())
            }
            _ => {
                // Deferred until release: nothing is committed while the
                // user is still choosing the end point.
                self.drag = DragMode::Anchored { origin: (x, y), current: (x, y) };
                Ok(())
            }
        }
    }

    /// The colour a stroke paints with: the active colour, except that the
    /// eraser always paints full transparency.
    fn stroke_color(&self) -> Color {
        match self.tool {
            Tool::Eraser => pixelcad_core::TRANSPARENT,
            _ => self.inked(),
        }
    }

    /// Continues whichever gesture is active. No-op if nothing is active.
    pub fn continue_drag(&mut self, screen_x: f32, screen_y: f32) -> Result<(), EngineError> {
        match self.drag {
            DragMode::Idle => Ok(()),
            DragMode::Painting { last } => {
                let (x, y) = self.screen_to_doc_clamped(screen_x, screen_y);
                if (x, y) != last {
                    let color = self.stroke_color();
                    let (size, shape) = (self.brush_size, self.brush_shape);
                    self.engine.execute(Command::BrushStroke {
                        x0: last.0,
                        y0: last.1,
                        x1: x,
                        y1: y,
                        size,
                        shape,
                        color,
                    })?;
                    self.drag = DragMode::Painting { last: (x, y) };
                }
                Ok(())
            }
            DragMode::Anchored { origin, .. } => {
                let current = self.screen_to_doc_clamped(screen_x, screen_y);
                if self.tool.is_path() && self.path.last() != Some(&current) {
                    self.path.push(current);
                }
                self.drag = DragMode::Anchored { origin, current };
                Ok(())
            }
            DragMode::Panning { anchor_pointer, anchor_pan } => {
                self.pan = (
                    anchor_pan.0 + (screen_x - anchor_pointer.0),
                    anchor_pan.1 + (screen_y - anchor_pointer.1),
                );
                Ok(())
            }
        }
    }

    /// Ends the active gesture, committing anything that was deferred.
    pub fn end_drag(&mut self) -> Result<(), EngineError> {
        let drag = std::mem::replace(&mut self.drag, DragMode::Idle);
        match drag {
            DragMode::Painting { .. } => {
                self.engine.end_group();
                Ok(())
            }
            DragMode::Anchored { origin, current } => {
                let color = self.inked();
                let path = std::mem::take(&mut self.path);
                let command = match self.tool {
                    Tool::Line => Command::LineDraw {
                        x0: origin.0,
                        y0: origin.1,
                        x1: current.0,
                        y1: current.1,
                        color,
                    },
                    Tool::Rectangle => Command::RectDraw {
                        x0: origin.0,
                        y0: origin.1,
                        x1: current.0,
                        y1: current.1,
                        radius: 0,
                        fill: false,
                        color,
                    },
                    Tool::Ellipse => Command::EllipseDraw {
                        x0: origin.0,
                        y0: origin.1,
                        x1: current.0,
                        y1: current.1,
                        fill: false,
                        color,
                    },
                    Tool::Arrow => Command::ArrowDraw {
                        x0: origin.0,
                        y0: origin.1,
                        x1: current.0,
                        y1: current.1,
                        head: (self.brush_size * 4).max(4),
                        color,
                    },
                    Tool::Polyline => {
                        if path.len() < 2 {
                            return Ok(());
                        }
                        Command::PolylineDraw { points: path, color }
                    }
                    Tool::Lasso => {
                        if path.len() < 3 {
                            // Too few points to enclose anything; treat it
                            // as "the user cancelled" rather than drawing a
                            // degenerate selection they cannot see.
                            return Ok(());
                        }
                        Command::SelectLasso { points: path }
                    }
                    Tool::Select => Command::SelectRect {
                        x: origin.0.min(current.0),
                        y: origin.1.min(current.1),
                        width: (origin.0 - current.0).unsigned_abs() as u32 + 1,
                        height: (origin.1 - current.1).unsigned_abs() as u32 + 1,
                    },
                    // Unreachable: no other tool uses Anchored.
                    _ => return Ok(()),
                };
                self.engine.execute(command)
            }
            _ => Ok(()),
        }
    }

    pub fn clear_selection(&mut self) -> Result<(), EngineError> {
        self.engine.execute(Command::SelectClear)
    }

    // ------------------------------------------------------------ history

    pub fn undo(&mut self) -> bool {
        if !self.can_undo() {
            return false;
        }
        self.engine.undo()
    }

    pub fn redo(&mut self) -> bool {
        self.engine.redo()
    }

    // ---------------------------------------------------------------- I/O

    /// Writes the currently active command history to `path` as `.pxc` text.
    pub fn save_script(&self, path: &Path) -> io::Result<()> {
        std::fs::write(path, self.engine.save_script())
    }

    /// Writes a versioned `.pxcproj` project file.
    pub fn save_project(&self, path: &Path) -> io::Result<()> {
        std::fs::write(path, serialize_project(&self.engine))
    }

    /// Replaces the document with the contents of a `.pxcproj` (or a legacy
    /// `.pxc` script).
    ///
    /// The existing engine is only swapped in **after** the new one has been
    /// fully built, so a corrupt or future-version file leaves the user's
    /// open work exactly as it was — the "never corrupt the document"
    /// invariant, enforced here rather than trusted.
    pub fn open_project_file(&mut self, path: &Path) -> Result<(), String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let engine = open_project(&text).map_err(|e| e.to_string())?;
        if engine.document().is_none() {
            return Err("project contains no canvas.new; nothing to open".to_string());
        }
        self.engine = engine;
        self.drag = DragMode::Idle;
        self.selected_palette_index = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every non-transparent pixel of the composite, in scan order.
    fn painted(c: &Controller) -> Vec<(i64, i64)> {
        let doc = c.document();
        let buf = doc.composite();
        let mut out = Vec::new();
        for y in 0..doc.height() as i64 {
            for x in 0..doc.width() as i64 {
                let i = (y as usize * doc.width() as usize + x as usize) * 4;
                if buf[i + 3] != 0 {
                    out.push((x, y));
                }
            }
        }
        out
    }

    #[test]
    fn new_controller_has_a_canvas_and_default_color() {
        let c = Controller::new(16, 16);
        assert_eq!((c.document().width(), c.document().height()), (16, 16));
        assert_eq!(c.current_color(), c.palette()[0]);
        assert_eq!(c.tool(), Tool::Brush);
        assert_eq!(c.brush_size(), 1);
        assert!(!c.can_undo(), "canvas.new itself must not be undoable");
        assert!(!c.can_redo());
    }

    #[test]
    fn select_palette_changes_current_color() {
        let mut c = Controller::new(8, 8);
        c.select_palette(3);
        assert_eq!(c.selected_palette_index(), 3);
        assert_eq!(c.current_color(), c.palette()[3]);
    }

    #[test]
    fn select_palette_out_of_range_is_ignored() {
        let mut c = Controller::new(8, 8);
        c.select_palette(3);
        c.select_palette(999);
        assert_eq!(c.selected_palette_index(), 3, "out-of-range selection must be a no-op");
    }

    #[test]
    fn editing_a_swatch_records_a_palette_set_command() {
        let mut c = Controller::new(8, 8);
        c.select_palette(5);
        assert!(c.set_swatch_color("#123456"));
        assert_eq!(c.palette()[5], [0x12, 0x34, 0x56, 0xff]);
        assert_eq!(c.current_color(), [0x12, 0x34, 0x56, 0xff]);

        let script = {
            let dir = std::env::temp_dir().join(format!("pixelcad-swatch-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let path = dir.join("s.pxc");
            c.save_script(&path).unwrap();
            let text = std::fs::read_to_string(&path).unwrap();
            let _ = std::fs::remove_dir_all(&dir);
            text
        };
        assert!(
            script.contains(r##"palette.set index=5 color="#123456ff""##),
            "the edit must be in the replayable script; got:\n{script}"
        );
    }

    #[test]
    fn an_invalid_swatch_hex_is_rejected_without_changing_anything() {
        let mut c = Controller::new(8, 8);
        let before = c.palette()[0];
        assert!(!c.set_swatch_color("not a color"));
        assert_eq!(c.palette()[0], before);
        assert!(!c.can_undo(), "a rejected edit must not create an undo step");
    }

    #[test]
    fn zoom_clamps_to_bounds() {
        let mut c = Controller::new(8, 8);
        for _ in 0..100 {
            c.zoom_in();
        }
        assert_eq!(c.zoom(), MAX_ZOOM);
        for _ in 0..100 {
            c.zoom_out();
        }
        assert_eq!(c.zoom(), MIN_ZOOM);
    }

    #[test]
    fn brush_size_clamps_to_bounds() {
        let mut c = Controller::new(8, 8);
        for _ in 0..100 {
            c.increase_brush();
        }
        assert_eq!(c.brush_size(), MAX_BRUSH);
        for _ in 0..100 {
            c.decrease_brush();
        }
        assert_eq!(c.brush_size(), MIN_BRUSH);
    }

    #[test]
    fn grid_visible_only_at_or_above_threshold() {
        let mut c = Controller::new(8, 8);
        for _ in 0..7 {
            c.zoom_out();
        } // zoom = 1
        assert!(!c.should_draw_grid());
        for _ in 0..7 {
            c.zoom_in();
        } // zoom = 8
        assert!(c.should_draw_grid());
    }

    #[test]
    fn every_tool_has_a_unique_shortcut_that_maps_back() {
        let mut seen = Vec::new();
        for tool in Tool::ALL {
            let key = tool.shortcut();
            assert!(!seen.contains(&key), "shortcut {key:?} is used twice");
            seen.push(key);
            assert_eq!(Tool::from_shortcut(key), Some(tool));
            assert_eq!(
                Tool::from_shortcut(key.to_ascii_uppercase()),
                Some(tool),
                "shortcuts must be case-insensitive"
            );
        }
        assert_eq!(Tool::from_shortcut('z'), None);
    }

    #[test]
    fn click_draws_a_single_pixel_with_the_brush() {
        let mut c = Controller::new(8, 8);
        c.select_palette(4); // red, per DEFAULT_PALETTE
                             // zoom=8, pan=(0,0): screen (20,20) -> doc pixel (2, 2)
        c.begin_stroke(20.0, 20.0).unwrap();
        c.end_drag().unwrap();
        assert_eq!(c.document().get_pixel(2, 2).unwrap(), c.palette()[4]);
        assert!(c.can_undo());
    }

    #[test]
    fn a_whole_drag_is_a_single_undo_step() {
        let mut c = Controller::new(8, 8);
        c.begin_stroke(4.0, 4.0).unwrap();
        c.continue_drag(12.0, 4.0).unwrap();
        c.continue_drag(20.0, 4.0).unwrap();
        c.continue_drag(28.0, 4.0).unwrap();
        c.end_drag().unwrap();
        assert_eq!(painted(&c).len(), 4, "the stroke covers four pixels");

        assert!(c.undo());
        assert_eq!(painted(&c).len(), 0, "one undo must remove the entire stroke");
        assert!(!c.can_undo(), "and land exactly on the empty canvas");
    }

    #[test]
    fn a_wide_brush_paints_a_block() {
        let mut c = Controller::new(16, 16);
        c.set_brush_size(3);
        c.begin_stroke(40.0, 40.0).unwrap(); // doc (5,5)
        c.end_drag().unwrap();
        assert_eq!(painted(&c).len(), 9);
    }

    #[test]
    fn the_eraser_removes_pixels_the_brush_drew() {
        let mut c = Controller::new(8, 8);
        c.begin_stroke(4.0, 4.0).unwrap();
        c.continue_drag(28.0, 4.0).unwrap();
        c.end_drag().unwrap();
        let drawn = painted(&c).len();
        assert!(drawn >= 4);

        c.set_tool(Tool::Eraser);
        c.begin_stroke(4.0, 4.0).unwrap();
        c.end_drag().unwrap();
        assert_eq!(painted(&c).len(), drawn - 1, "exactly one pixel erased");
    }

    #[test]
    fn the_fill_tool_floods_on_click() {
        let mut c = Controller::new(8, 8);
        c.set_tool(Tool::Fill);
        c.select_palette(4);
        c.begin_stroke(4.0, 4.0).unwrap();
        c.end_drag().unwrap();
        assert_eq!(painted(&c).len(), 64, "an empty canvas fills entirely");
    }

    #[test]
    fn the_eyedropper_adopts_the_colour_under_the_pointer() {
        let mut c = Controller::new(8, 8);
        c.select_palette(4); // red
        let red = c.current_color();
        c.begin_stroke(4.0, 4.0).unwrap(); // paint doc (0,0)
        c.end_drag().unwrap();

        c.select_palette(1); // white
        assert_ne!(c.current_color(), red);

        c.set_tool(Tool::Eyedropper);
        c.begin_stroke(4.0, 4.0).unwrap();
        c.end_drag().unwrap();
        assert_eq!(c.current_color(), red, "the eyedropper picked the painted pixel");
    }

    #[test]
    fn the_line_tool_commits_only_on_release() {
        let mut c = Controller::new(8, 8);
        c.set_tool(Tool::Line);
        c.begin_stroke(4.0, 4.0).unwrap(); // doc (0,0)
        c.continue_drag(28.0, 4.0).unwrap(); // doc (3,0)
        assert_eq!(painted(&c).len(), 0, "nothing is drawn while dragging");

        c.end_drag().unwrap();
        assert_eq!(painted(&c), vec![(0, 0), (1, 0), (2, 0), (3, 0)]);
    }

    #[test]
    fn the_rectangle_tool_draws_an_outline_on_release() {
        let mut c = Controller::new(8, 8);
        c.set_tool(Tool::Rectangle);
        c.begin_stroke(4.0, 4.0).unwrap(); // doc (0,0)
        c.continue_drag(28.0, 28.0).unwrap(); // doc (3,3)
        c.end_drag().unwrap();
        assert_eq!(painted(&c).len(), 12, "a 4x4 outline is 12 pixels");
    }

    #[test]
    fn the_select_tool_sets_a_marquee_that_clips_later_drawing() {
        let mut c = Controller::new(8, 8);
        c.set_tool(Tool::Select);
        c.begin_stroke(16.0, 16.0).unwrap(); // doc (2,2)
        c.continue_drag(32.0, 32.0).unwrap(); // doc (4,4)
        c.end_drag().unwrap();
        assert_eq!(
            c.selection(),
            Some(&Selection::rect(2, 2, 3, 3)),
            "the marquee is inclusive of both corners"
        );

        c.set_tool(Tool::Fill);
        c.begin_stroke(0.0, 0.0).unwrap(); // click outside the marquee
        c.end_drag().unwrap();
        for (x, y) in painted(&c) {
            assert!((2..5).contains(&x) && (2..5).contains(&y), "({x},{y}) escaped");
        }

        c.clear_selection().unwrap();
        assert_eq!(c.selection(), None);
    }

    #[test]
    fn switching_tools_abandons_a_half_finished_gesture() {
        let mut c = Controller::new(8, 8);
        c.set_tool(Tool::Line);
        c.begin_stroke(4.0, 4.0).unwrap();
        c.continue_drag(28.0, 4.0).unwrap();
        c.set_tool(Tool::Brush); // user changes their mind mid-drag
        c.end_drag().unwrap();
        assert_eq!(painted(&c).len(), 0, "the abandoned line must not be committed");
    }

    #[test]
    fn pan_moves_the_view_without_touching_the_document() {
        let mut c = Controller::new(8, 8);
        let before = c.document().clone();
        c.begin_pan(10.0, 10.0);
        c.continue_drag(30.0, 15.0).unwrap();
        c.end_drag().unwrap();
        assert_eq!(c.pan(), (20.0, 5.0));
        assert_eq!(c.document(), &before, "panning must not mutate the document");
        assert!(!c.can_undo(), "panning must not be recorded as a command");
    }

    #[test]
    fn undo_redo_round_trip_through_controller() {
        let mut c = Controller::new(8, 8);
        c.begin_stroke(4.0, 4.0).unwrap();
        c.end_drag().unwrap();
        assert_eq!(c.document().get_pixel(0, 0).unwrap(), c.current_color());

        assert!(c.undo());
        assert_ne!(c.document().get_pixel(0, 0).unwrap(), c.current_color());
        assert!(c.can_redo());

        assert!(c.redo());
        assert_eq!(c.document().get_pixel(0, 0).unwrap(), c.current_color());
    }

    #[test]
    fn undo_cannot_remove_the_initial_canvas() {
        let mut c = Controller::new(8, 8);
        assert!(!c.undo(), "there must be nothing to undo yet");
        c.begin_stroke(4.0, 4.0).unwrap();
        c.end_drag().unwrap();
        assert!(c.undo());
        assert!(!c.undo(), "undo must stop right after canvas.new, never remove it");
    }

    #[test]
    fn layers_can_be_added_selected_hidden_and_removed() {
        let mut c = Controller::new(8, 8);
        assert_eq!(c.layer_names(), vec!["Layer 1"]);

        c.add_layer().unwrap();
        assert_eq!(c.layer_names(), vec!["Layer 1", "Layer 2"]);
        assert_eq!(c.active_layer_index(), 1);

        // Draw on the new layer; the layer below stays empty.
        c.begin_stroke(4.0, 4.0).unwrap();
        c.end_drag().unwrap();
        assert_eq!(c.document().get_pixel_on(0, 0, 0).unwrap(), [0, 0, 0, 0]);
        assert_ne!(c.document().get_pixel_on(1, 0, 0).unwrap(), [0, 0, 0, 0]);

        c.toggle_layer_visible(1).unwrap();
        assert!(!c.layer_visible(1));
        assert_eq!(painted(&c).len(), 0, "hiding the layer hides its pixels");

        c.toggle_layer_visible(1).unwrap();
        assert_eq!(painted(&c).len(), 1);

        c.select_layer(0).unwrap();
        assert_eq!(c.active_layer_index(), 0);

        c.select_layer(1).unwrap();
        c.remove_active_layer().unwrap();
        assert_eq!(c.layer_names(), vec!["Layer 1"]);
    }

    #[test]
    fn removing_the_last_layer_fails_without_damaging_the_document() {
        let mut c = Controller::new(4, 4);
        let before = c.document().clone();
        assert!(c.remove_active_layer().is_err());
        assert_eq!(c.document(), &before);
    }

    #[test]
    fn save_script_writes_replayable_pxc_text() {
        let mut c = Controller::new(4, 4);
        c.select_palette(4);
        c.begin_stroke(4.0, 4.0).unwrap();
        c.end_drag().unwrap();

        let dir = std::env::temp_dir().join(format!("pixelcad-app-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("session.pxc");
        c.save_script(&path).unwrap();

        let text = std::fs::read_to_string(&path).unwrap();
        let commands = pixelcad_core::parse_script(&text).unwrap();
        assert_eq!(
            commands,
            vec![
                Command::CanvasNew { width: 4, height: 4 },
                Command::ColorSet { color: c.current_color() },
                Command::BrushStroke {
                    x0: 0,
                    y0: 0,
                    x1: 0,
                    y1: 0,
                    size: 1, shape: BrushShape::Square,
                    color: c.current_color()
                },
            ]
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_project_round_trips_through_save_and_open() {
        let dir = std::env::temp_dir().join(format!("pixelcad-proj-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("drawing.pxcproj");

        let mut c = Controller::new(8, 8);
        c.add_layer().unwrap();
        c.select_palette(4);
        c.begin_stroke(4.0, 4.0).unwrap();
        c.continue_drag(28.0, 28.0).unwrap();
        c.end_drag().unwrap();
        let expected_hash = c.document().content_hash();
        c.save_project(&path).unwrap();

        let mut fresh = Controller::new(2, 2);
        fresh.open_project_file(&path).unwrap();
        assert_eq!(fresh.document().content_hash(), expected_hash);
        assert_eq!(fresh.layer_names(), vec!["Layer 1", "Layer 2"]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn opening_a_broken_project_leaves_the_open_document_untouched() {
        let dir = std::env::temp_dir().join(format!("pixelcad-broken-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("future.pxcproj");
        std::fs::write(&path, "pixelcad.project version=99\ncanvas.new width=4 height=4\n")
            .unwrap();

        let mut c = Controller::new(8, 8);
        c.begin_stroke(4.0, 4.0).unwrap();
        c.end_drag().unwrap();
        let before = c.document().clone();

        let err = c.open_project_file(&path).unwrap_err();
        assert!(err.contains("99"), "the error must be actionable; got {err:?}");
        assert_eq!(c.document(), &before, "the user's open work must survive a failed open");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
