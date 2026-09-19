//! Pure application logic for `pixelcad-app`, kept free of any Slint type so
//! it can be unit-tested without a window, event loop, or platform backend.
//!
//! `main.rs` is the only place that touches Slint: it owns a `Controller`,
//! forwards UI callbacks into it, and re-renders after every mutation.

use std::io;
use std::path::Path;

use pixelcad_core::{Color, Command, Document, Engine, EngineError, PALETTE_SIZE};

/// Minimum and maximum zoom levels (document pixels per screen pixel block).
pub const MIN_ZOOM: u32 = 1;
pub const MAX_ZOOM: u32 = 32;

/// What an in-progress pointer drag on the canvas is doing.
#[derive(Debug, Clone, Copy, PartialEq)]
enum DragMode {
    Idle,
    /// Drawing a pencil stroke; carries the last document pixel touched, so
    /// each new point can be connected to it with `line.draw` (no gaps on a
    /// fast mouse move).
    Drawing { last: (i64, i64) },
    /// Panning the view; carries the pointer position and pan offset at the
    /// moment the drag started.
    Panning { anchor_pointer: (f32, f32), anchor_pan: (f32, f32) },
}

/// Owns the [`Engine`] plus all view/tool state for the GUI: pan, zoom,
/// current color, and the active drag gesture.
pub struct Controller {
    engine: Engine,
    pan: (f32, f32),
    zoom: u32,
    selected_palette_index: usize,
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

    pub fn current_color(&self) -> Color {
        self.palette()[self.selected_palette_index]
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
    /// pencil strokes on top of it.
    pub fn can_undo(&self) -> bool {
        self.engine.history().len() > 1
    }

    pub fn can_redo(&self) -> bool {
        self.engine.can_redo()
    }

    pub fn select_palette(&mut self, index: usize) {
        if index < PALETTE_SIZE {
            self.selected_palette_index = index;
        }
    }

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

    /// Begins a pan gesture anchored at `(screen_x, screen_y)`.
    pub fn begin_pan(&mut self, screen_x: f32, screen_y: f32) {
        self.drag = DragMode::Panning { anchor_pointer: (screen_x, screen_y), anchor_pan: self.pan };
    }

    /// Begins a pencil stroke: sets the pixel under `(screen_x, screen_y)`
    /// to the current color.
    pub fn begin_stroke(&mut self, screen_x: f32, screen_y: f32) -> Result<(), EngineError> {
        let (x, y) = self.screen_to_doc_clamped(screen_x, screen_y);
        let color = self.current_color();
        self.engine.execute(Command::PixelSet { x, y, color })?;
        self.drag = DragMode::Drawing { last: (x, y) };
        Ok(())
    }

    /// Continues whichever gesture is active (drawing or panning). No-op if
    /// nothing is active.
    pub fn continue_drag(&mut self, screen_x: f32, screen_y: f32) -> Result<(), EngineError> {
        match self.drag {
            DragMode::Idle => Ok(()),
            DragMode::Drawing { last } => {
                let (x, y) = self.screen_to_doc_clamped(screen_x, screen_y);
                if (x, y) != last {
                    let color = self.current_color();
                    self.engine.execute(Command::LineDraw {
                        x0: last.0,
                        y0: last.1,
                        x1: x,
                        y1: y,
                        color,
                    })?;
                    self.drag = DragMode::Drawing { last: (x, y) };
                }
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

    /// Ends whatever gesture is active.
    pub fn end_drag(&mut self) {
        self.drag = DragMode::Idle;
    }

    pub fn undo(&mut self) -> bool {
        if !self.can_undo() {
            return false;
        }
        self.engine.undo()
    }

    pub fn redo(&mut self) -> bool {
        self.engine.redo()
    }

    /// Writes the currently active command history to `path` as `.pxc` text.
    pub fn save_script(&self, path: &Path) -> io::Result<()> {
        std::fs::write(path, self.engine.save_script())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_controller_has_a_canvas_and_default_color() {
        let c = Controller::new(16, 16);
        assert_eq!((c.document().width(), c.document().height()), (16, 16));
        assert_eq!(c.current_color(), c.palette()[0]);
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
    fn grid_visible_only_at_or_above_threshold() {
        let mut c = Controller::new(8, 8);
        c.zoom_out();
        c.zoom_out();
        c.zoom_out();
        c.zoom_out();
        c.zoom_out();
        c.zoom_out();
        c.zoom_out(); // zoom = 1
        assert!(!c.should_draw_grid());
        for _ in 0..7 {
            c.zoom_in();
        } // zoom = 8
        assert!(c.should_draw_grid());
    }

    #[test]
    fn click_draws_a_single_pixel() {
        let mut c = Controller::new(8, 8);
        c.select_palette(4); // red, per DEFAULT_PALETTE
        // zoom=8, pan=(0,0): screen (20,20) -> doc pixel (2, 2)
        c.begin_stroke(20.0, 20.0).unwrap();
        c.end_drag();
        assert_eq!(c.document().get_pixel(2, 2).unwrap(), c.palette()[4]);
        assert!(c.can_undo());
    }

    #[test]
    fn drag_connects_points_with_a_line() {
        let mut c = Controller::new(8, 8);
        // zoom=8: doc (0,0) -> screen (4,4); doc (3,0) -> screen (28,4)
        c.begin_stroke(4.0, 4.0).unwrap();
        c.continue_drag(28.0, 4.0).unwrap();
        c.end_drag();
        for x in 0..=3 {
            assert_eq!(c.document().get_pixel(x, 0).unwrap(), c.current_color());
        }
    }

    #[test]
    fn pan_moves_the_view_without_touching_the_document() {
        let mut c = Controller::new(8, 8);
        let before = c.document().clone();
        c.begin_pan(10.0, 10.0);
        c.continue_drag(30.0, 15.0).unwrap();
        c.end_drag();
        assert_eq!(c.pan(), (20.0, 5.0));
        assert_eq!(c.document(), &before, "panning must not mutate the document");
        assert!(!c.can_undo(), "panning must not be recorded as a command");
    }

    #[test]
    fn undo_redo_round_trip_through_controller() {
        let mut c = Controller::new(8, 8);
        c.begin_stroke(4.0, 4.0).unwrap();
        c.end_drag();
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
        c.end_drag();
        assert!(c.undo());
        assert!(!c.undo(), "undo must stop right after canvas.new, never remove it");
    }

    #[test]
    fn save_script_writes_replayable_pxc_text() {
        let mut c = Controller::new(4, 4);
        c.select_palette(4);
        c.begin_stroke(4.0, 4.0).unwrap();
        c.end_drag();

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
                Command::PixelSet { x: 0, y: 0, color: c.current_color() },
            ]
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
