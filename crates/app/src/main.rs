//! `pixelcad-app` — the Slint desktop shell.
//!
//! This binary contains no document or command logic of its own. It owns a
//! [`controller::Controller`], forwards UI events into it, and re-renders
//! the canvas image after every mutation. All drawing/undo/redo/save
//! behaviour lives in `controller.rs` (Slint-independent, unit-tested) and
//! `render.rs` (nearest-neighbour scaling + grid overlay, also
//! Slint-independent and unit-tested). This file is intentionally thin
//! plumbing: `wire_callbacks` is the one place UI events turn into
//! `Controller` calls, and it is exercised directly (not just informally)
//! by the headless GUI tests in `#[cfg(test)] mod gui_tests` below, which
//! dispatch real pointer/scroll events through Slint's testing backend.

mod controller;
mod render;

slint::include_modules!();

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use slint::{
    Color as SlintColor, ComponentHandle, Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, VecModel,
};

use controller::Controller;

const DEFAULT_CANVAS_WIDTH: u32 = 64;
const DEFAULT_CANVAS_HEIGHT: u32 = 64;

fn to_slint_color(c: pixelcad_core::Color) -> SlintColor {
    SlintColor::from_argb_u8(c[3], c[0], c[1], c[2])
}

/// Renders the controller's document (nearest-neighbour scaled, with a grid
/// overlay when zoomed in enough) into a `slint::Image`.
fn render_image(controller: &Controller) -> (Image, u32, u32) {
    let (w, h, bytes) = render::render_display_buffer(
        controller.document(),
        controller.zoom(),
        controller.should_draw_grid(),
    );
    let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(w, h);
    buffer.make_mut_bytes().copy_from_slice(&bytes);
    (Image::from_rgba8(buffer), w, h)
}

/// Pushes the full controller state to the UI. Called after every mutation;
/// simple and correct beats incremental diffing at this scale.
fn refresh(app: &AppWindow, controller: &Controller) {
    let (image, w, h) = render_image(controller);
    app.set_canvas_image(image);
    app.set_canvas_display_width(w as i32);
    app.set_canvas_display_height(h as i32);

    let (pan_x, pan_y) = controller.pan();
    app.set_pan_x(pan_x);
    app.set_pan_y(pan_y);

    app.set_selected_index(controller.selected_palette_index() as i32);
    app.set_can_undo(controller.can_undo());
    app.set_can_redo(controller.can_redo());

    let palette_colors: Vec<SlintColor> =
        controller.palette().iter().copied().map(to_slint_color).collect();
    app.set_palette_colors(ModelRc::new(VecModel::from(palette_colors)));
}

/// Registers every UI -> `Controller` callback. Shared between `main()` and
/// the headless GUI tests so both exercise identical wiring.
fn wire_callbacks(app: &AppWindow, controller: Rc<RefCell<Controller>>) {
    {
        let controller = controller.clone();
        let app_weak = app.as_weak();
        app.on_pencil_down(move |x, y| {
            let mut c = controller.borrow_mut();
            let _ = c.begin_stroke(x, y);
            if let Some(app) = app_weak.upgrade() {
                refresh(&app, &c);
            }
        });
    }
    {
        let controller = controller.clone();
        let app_weak = app.as_weak();
        app.on_pan_down(move |x, y| {
            let mut c = controller.borrow_mut();
            c.begin_pan(x, y);
            if let Some(app) = app_weak.upgrade() {
                refresh(&app, &c);
            }
        });
    }
    {
        let controller = controller.clone();
        let app_weak = app.as_weak();
        app.on_pointer_moved(move |x, y| {
            let mut c = controller.borrow_mut();
            let _ = c.continue_drag(x, y);
            if let Some(app) = app_weak.upgrade() {
                refresh(&app, &c);
            }
        });
    }
    {
        let controller = controller.clone();
        let app_weak = app.as_weak();
        app.on_pointer_released(move || {
            let mut c = controller.borrow_mut();
            c.end_drag();
            if let Some(app) = app_weak.upgrade() {
                refresh(&app, &c);
            }
        });
    }
    {
        let controller = controller.clone();
        let app_weak = app.as_weak();
        app.on_canvas_scrolled(move |delta_y| {
            let mut c = controller.borrow_mut();
            c.handle_scroll(delta_y);
            if let Some(app) = app_weak.upgrade() {
                refresh(&app, &c);
            }
        });
    }
    {
        let controller = controller.clone();
        let app_weak = app.as_weak();
        app.on_palette_clicked(move |index| {
            let mut c = controller.borrow_mut();
            c.select_palette(index as usize);
            if let Some(app) = app_weak.upgrade() {
                refresh(&app, &c);
            }
        });
    }
    {
        let controller = controller.clone();
        let app_weak = app.as_weak();
        app.on_undo_clicked(move || {
            let mut c = controller.borrow_mut();
            c.undo();
            if let Some(app) = app_weak.upgrade() {
                refresh(&app, &c);
            }
        });
    }
    {
        let controller = controller.clone();
        let app_weak = app.as_weak();
        app.on_redo_clicked(move || {
            let mut c = controller.borrow_mut();
            c.redo();
            if let Some(app) = app_weak.upgrade() {
                refresh(&app, &c);
            }
        });
    }
    {
        let controller = controller.clone();
        let app_weak = app.as_weak();
        app.on_save_clicked(move |path| {
            let c = controller.borrow();
            let result = c.save_script(Path::new(path.as_str()));
            if let Some(app) = app_weak.upgrade() {
                let message = match result {
                    Ok(()) => format!("Saved to {path}"),
                    Err(e) => format!("Save failed: {e}"),
                };
                app.set_status_message(message.into());
            }
        });
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let app = AppWindow::new()?;
    let controller = Rc::new(RefCell::new(Controller::new(DEFAULT_CANVAS_WIDTH, DEFAULT_CANVAS_HEIGHT)));

    refresh(&app, &controller.borrow());
    wire_callbacks(&app, controller);

    app.run()
}

/// Headless GUI behaviour tests: real `AppWindow` instances, real
/// `wire_callbacks`, real Slint input dispatch (via
/// `i-slint-backend-testing`) — no visible window or real display required.
/// These exercise the acceptance criteria from PLAN.md Task 5 end-to-end
/// through the same code path `main()` uses.
#[cfg(test)]
mod gui_tests {
    use super::*;
    use i_slint_backend_testing::ElementHandle;
    use slint::platform::{PointerEventButton, WindowEvent};
    use slint::{LogicalPosition, Model};

    /// `init_no_event_loop` may only be called once per *thread*, and each
    /// `#[test]` function runs on its own thread by default, so call this at
    /// the top of every test rather than sharing a `static Once`.
    fn new_app_with_controller() -> (AppWindow, Rc<RefCell<Controller>>) {
        i_slint_backend_testing::init_no_event_loop();
        let app = AppWindow::new().unwrap();
        let controller = Rc::new(RefCell::new(Controller::new(DEFAULT_CANVAS_WIDTH, DEFAULT_CANVAS_HEIGHT)));
        refresh(&app, &controller.borrow());
        wire_callbacks(&app, controller.clone());
        (app, controller)
    }

    fn find_one(app: &AppWindow, id: &str) -> ElementHandle {
        let mut it = ElementHandle::find_by_element_id(app, id);
        let handle = it.next().unwrap_or_else(|| panic!("no element found with id {id:?}"));
        handle
    }

    #[test]
    fn window_launches_with_expected_default_state() {
        let (app, _controller) = new_app_with_controller();
        // zoom defaults to 8, canvas defaults to 64x64 -> 512x512 display px.
        assert_eq!(app.get_canvas_display_width(), 512);
        assert_eq!(app.get_canvas_display_height(), 512);
        assert!(!app.get_can_undo());
        assert!(!app.get_can_redo());
        assert_eq!(app.get_selected_index(), 0);
        assert_eq!(app.get_palette_colors().row_count(), pixelcad_core::PALETTE_SIZE);
    }

    #[test]
    fn clicking_a_palette_swatch_selects_it() {
        let (app, controller) = new_app_with_controller();
        let mut swatches: Vec<ElementHandle> =
            ElementHandle::find_by_element_id(&app, "AppWindow::swatch-row1").collect();
        swatches.extend(ElementHandle::find_by_element_id(&app, "AppWindow::swatch-row2"));
        assert_eq!(swatches.len(), pixelcad_core::PALETTE_SIZE, "expected all 16 swatches");

        swatches[4].mock_single_click(PointerEventButton::Left);

        assert_eq!(app.get_selected_index(), 4);
        assert_eq!(controller.borrow().selected_palette_index(), 4);
    }

    #[test]
    fn dragging_on_the_canvas_draws_a_connected_stroke_and_enables_undo() {
        let (app, controller) = new_app_with_controller();
        let canvas = find_one(&app, "AppWindow::canvas-touch");
        let top_left = canvas.absolute_position();
        let window = app.window();

        // zoom = 8: doc (2,2) -> canvas-local (20,20); doc (5,2) -> canvas-local (44,20).
        let start = LogicalPosition::new(top_left.x + 20.0, top_left.y + 20.0);
        let mid = LogicalPosition::new(top_left.x + 44.0, top_left.y + 20.0);

        window.dispatch_event(WindowEvent::PointerMoved { position: start });
        window.dispatch_event(WindowEvent::PointerPressed { position: start, button: PointerEventButton::Left });
        window.dispatch_event(WindowEvent::PointerMoved { position: mid });
        window.dispatch_event(WindowEvent::PointerReleased { position: mid, button: PointerEventButton::Left });

        let doc = controller.borrow().document().clone();
        let expected_color = controller.borrow().current_color();
        for x in 2..=5 {
            assert_eq!(doc.get_pixel(x, 2).unwrap(), expected_color, "pixel ({x}, 2) should be on the drawn line");
        }
        assert!(app.get_can_undo());
    }

    #[test]
    fn right_click_drag_pans_the_view_without_touching_the_document() {
        let (app, controller) = new_app_with_controller();
        let canvas = find_one(&app, "AppWindow::canvas-touch");
        let top_left = canvas.absolute_position();
        let window = app.window();
        let before = controller.borrow().document().clone();

        let start = LogicalPosition::new(top_left.x + 50.0, top_left.y + 50.0);
        let end = LogicalPosition::new(top_left.x + 70.0, top_left.y + 65.0);

        window.dispatch_event(WindowEvent::PointerMoved { position: start });
        window.dispatch_event(WindowEvent::PointerPressed { position: start, button: PointerEventButton::Right });
        window.dispatch_event(WindowEvent::PointerMoved { position: end });
        window.dispatch_event(WindowEvent::PointerReleased { position: end, button: PointerEventButton::Right });

        assert_eq!(controller.borrow().pan(), (20.0, 15.0));
        assert_eq!(controller.borrow().document(), &before, "panning must never mutate the document");
        assert!(!app.get_can_undo(), "panning must not be recorded as a command");
        assert_eq!((app.get_pan_x(), app.get_pan_y()), (20.0, 15.0));
    }

    #[test]
    fn scroll_event_zooms_the_canvas() {
        let (app, controller) = new_app_with_controller();
        let canvas = find_one(&app, "AppWindow::canvas-touch");
        let top_left = canvas.absolute_position();
        let size = canvas.size();
        let center = LogicalPosition::new(top_left.x + size.width / 2.0, top_left.y + size.height / 2.0);

        assert_eq!(controller.borrow().zoom(), 8);

        // Negative delta_y zooms in, per Controller::handle_scroll's documented convention.
        app.window().dispatch_event(WindowEvent::PointerScrolled {
            position: center,
            delta_x: 0.0,
            delta_y: -1.0,
        });
        assert_eq!(controller.borrow().zoom(), 9);
        assert_eq!(app.get_canvas_display_width(), 64 * 9);

        app.window().dispatch_event(WindowEvent::PointerScrolled {
            position: center,
            delta_x: 0.0,
            delta_y: 1.0,
        });
        assert_eq!(controller.borrow().zoom(), 8);
    }

    #[test]
    fn undo_and_redo_buttons_round_trip_through_the_engine() {
        let (app, controller) = new_app_with_controller();
        {
            let mut c = controller.borrow_mut();
            c.begin_stroke(4.0, 4.0).unwrap();
            c.end_drag();
        }
        refresh(&app, &controller.borrow());
        assert!(app.get_can_undo());

        let after_draw = controller.borrow().document().clone();

        find_one(&app, "AppWindow::undo-btn").mock_single_click(PointerEventButton::Left);
        assert!(!app.get_can_undo());
        assert!(app.get_can_redo());
        assert_ne!(controller.borrow().document(), &after_draw);

        find_one(&app, "AppWindow::redo-btn").mock_single_click(PointerEventButton::Left);
        assert!(app.get_can_undo());
        assert!(!app.get_can_redo());
        assert_eq!(controller.borrow().document(), &after_draw);
    }

    #[test]
    fn save_button_writes_a_pxc_file_the_cli_can_replay() {
        let (app, controller) = new_app_with_controller();
        {
            let mut c = controller.borrow_mut();
            c.select_palette(4);
            c.begin_stroke(4.0, 4.0).unwrap();
            c.end_drag();
        }
        refresh(&app, &controller.borrow());

        let dir = std::env::temp_dir().join(format!("pixelcad-app-gui-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("saved.pxc");
        app.set_save_path(path.to_str().unwrap().into());

        app.invoke_save_clicked(path.to_str().unwrap().into());

        let text = std::fs::read_to_string(&path).unwrap();
        let commands = pixelcad_core::parse_script(&text).unwrap();
        assert_eq!(
            commands,
            vec![
                pixelcad_core::Command::CanvasNew { width: 64, height: 64 },
                pixelcad_core::Command::PixelSet { x: 0, y: 0, color: controller.borrow().current_color() },
            ]
        );
        assert!(app.get_status_message().contains("Saved to"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
