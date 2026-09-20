//! `pixelcad-app` — the Slint desktop shell.
//!
//! This binary contains no document or command logic of its own. It owns a
//! [`controller::Controller`], forwards UI events into it, and re-renders
//! the canvas image after every mutation. All tool/undo/layer/save
//! behaviour lives in `controller.rs` (Slint-independent, unit-tested) and
//! `render.rs` (nearest-neighbour scaling + grid overlay, also
//! Slint-independent and unit-tested). This file is intentionally thin
//! plumbing: `wire_callbacks` is the one place UI events turn into
//! `Controller` calls, and it is exercised directly (not just informally)
//! by the headless GUI tests in `#[cfg(test)] mod gui_tests` below, which
//! dispatch real pointer, scroll and key events through Slint's testing
//! backend.

mod controller;
mod render;

slint::include_modules!();

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use slint::{
    Color as SlintColor, ComponentHandle, Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, VecModel,
};

use controller::{Controller, Tool};

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
    app.set_has_selection(controller.selection().is_some());
    app.set_brush_size(controller.brush_size() as i32);
    app.set_selected_tool(
        Tool::ALL.iter().position(|t| *t == controller.tool()).unwrap_or(0) as i32,
    );

    let palette_colors: Vec<SlintColor> =
        controller.palette().iter().copied().map(to_slint_color).collect();
    app.set_palette_colors(ModelRc::new(VecModel::from(palette_colors)));

    // The UI lists layers top-first (as every editor does) while the model
    // stores them bottom-first, so the order is reversed here and the index
    // is converted back in the callbacks.
    let layers: Vec<LayerRow> = controller
        .layer_names()
        .into_iter()
        .enumerate()
        .map(|(i, name)| LayerRow {
            name: name.as_str().into(),
            visible: controller.layer_visible(i),
        })
        .rev()
        .collect();
    app.set_layers(ModelRc::new(VecModel::from(layers)));
    app.set_active_layer(controller.active_layer_index() as i32);
}

/// Applies a keyboard shortcut. Returns whether the key was consumed.
///
/// The whole table lives here, in plain Rust, rather than in `.slint`, so
/// it can be unit-tested and so the platform difference between Ctrl and
/// Cmd is collapsed into one `accel` flag by the UI layer.
fn handle_shortcut(controller: &mut Controller, text: &str, accel: bool) -> bool {
    let Some(ch) = text.chars().next() else {
        return false;
    };

    if accel {
        return match ch.to_ascii_lowercase() {
            // Shift+Cmd/Ctrl+Z arrives as an uppercase 'Z'.
            'z' if ch.is_uppercase() => {
                controller.redo();
                true
            }
            'z' => {
                controller.undo();
                true
            }
            'd' => {
                let _ = controller.clear_selection();
                true
            }
            _ => false,
        };
    }

    if let Some(tool) = Tool::from_shortcut(ch) {
        controller.set_tool(tool);
        return true;
    }

    match ch {
        '[' => {
            controller.decrease_brush();
            true
        }
        ']' => {
            controller.increase_brush();
            true
        }
        '+' | '=' => {
            controller.zoom_in();
            true
        }
        '-' | '_' => {
            controller.zoom_out();
            true
        }
        // Digits 1-8 pick the first palette row.
        '1'..='8' => {
            controller.select_palette(ch as usize - '1' as usize);
            true
        }
        _ => false,
    }
}

/// Registers every UI -> `Controller` callback. Shared between `main()` and
/// the headless GUI tests so both exercise identical wiring.
fn wire_callbacks(app: &AppWindow, controller: Rc<RefCell<Controller>>) {
    /// Boilerplate killer: borrow the controller, run `body`, re-render.
    macro_rules! on {
        ($setter:ident, |$c:ident $(, $arg:ident : $ty:ty)*| $body:block) => {{
            let controller = controller.clone();
            let app_weak = app.as_weak();
            app.$setter(move |$($arg: $ty),*| {
                {
                    let mut $c = controller.borrow_mut();
                    $body
                }
                if let Some(app) = app_weak.upgrade() {
                    refresh(&app, &controller.borrow());
                }
            });
        }};
    }

    on!(on_pencil_down, |c, x: f32, y: f32| {
        let _ = c.begin_stroke(x, y);
    });
    on!(on_pan_down, |c, x: f32, y: f32| {
        c.begin_pan(x, y);
    });
    on!(on_pointer_moved, |c, x: f32, y: f32| {
        let _ = c.continue_drag(x, y);
    });
    on!(on_pointer_released, |c| {
        let _ = c.end_drag();
    });
    on!(on_canvas_scrolled, |c, delta_y: f32| {
        c.handle_scroll(delta_y);
    });
    on!(on_palette_clicked, |c, index: i32| {
        c.select_palette(index.max(0) as usize);
    });
    on!(on_tool_selected, |c, index: i32| {
        if let Some(tool) = Tool::ALL.get(index.max(0) as usize) {
            c.set_tool(*tool);
        }
    });
    on!(on_brush_size_changed, |c, size: i32| {
        c.set_brush_size(size.max(0) as u32);
    });
    on!(on_undo_clicked, |c| {
        c.undo();
    });
    on!(on_redo_clicked, |c| {
        c.redo();
    });
    on!(on_layer_add_clicked, |c| {
        let _ = c.add_layer();
    });
    on!(on_layer_selected, |c, index: i32| {
        let _ = c.select_layer(index.max(0) as usize);
    });
    on!(on_layer_visibility_toggled, |c, index: i32| {
        let _ = c.toggle_layer_visible(index.max(0) as usize);
    });
    on!(on_selection_cleared, |c| {
        let _ = c.clear_selection();
    });

    // Callbacks that also report a status message need the app handle
    // inside the body, so they are written out longhand.
    {
        let controller = controller.clone();
        let app_weak = app.as_weak();
        app.on_layer_remove_clicked(move || {
            let message = match controller.borrow_mut().remove_active_layer() {
                Ok(()) => "Layer deleted".to_string(),
                Err(e) => format!("Cannot delete layer: {e}"),
            };
            if let Some(app) = app_weak.upgrade() {
                app.set_status_message(message.into());
                refresh(&app, &controller.borrow());
            }
        });
    }
    {
        let controller = controller.clone();
        let app_weak = app.as_weak();
        app.on_swatch_applied(move |hex| {
            let ok = controller.borrow_mut().set_swatch_color(hex.as_str());
            if let Some(app) = app_weak.upgrade() {
                let message = if ok {
                    format!("Swatch set to {hex}")
                } else {
                    format!("Not a colour: {hex} (expected #rrggbb or #rrggbbaa)")
                };
                app.set_status_message(message.into());
                refresh(&app, &controller.borrow());
            }
        });
    }
    {
        let controller = controller.clone();
        let app_weak = app.as_weak();
        app.on_save_clicked(move |path| {
            let result = controller.borrow().save_script(Path::new(path.as_str()));
            if let Some(app) = app_weak.upgrade() {
                app.set_status_message(status_of(result, &path, "Saved script to").into());
            }
        });
    }
    {
        let controller = controller.clone();
        let app_weak = app.as_weak();
        app.on_save_project_clicked(move |path| {
            let result = controller.borrow().save_project(Path::new(path.as_str()));
            if let Some(app) = app_weak.upgrade() {
                app.set_status_message(status_of(result, &path, "Saved project to").into());
            }
        });
    }
    {
        let controller = controller.clone();
        let app_weak = app.as_weak();
        app.on_open_project_clicked(move |path| {
            let result = controller.borrow_mut().open_project_file(&PathBuf::from(path.as_str()));
            if let Some(app) = app_weak.upgrade() {
                let message = match result {
                    Ok(()) => format!("Opened {path}"),
                    // A failed open leaves the document untouched — see
                    // Controller::open_project_file.
                    Err(e) => format!("Open failed: {e}"),
                };
                app.set_status_message(message.into());
                refresh(&app, &controller.borrow());
            }
        });
    }
    {
        let controller = controller.clone();
        let app_weak = app.as_weak();
        app.on_key_pressed(move |text, accel| {
            let consumed = handle_shortcut(&mut controller.borrow_mut(), text.as_str(), accel);
            if consumed {
                if let Some(app) = app_weak.upgrade() {
                    refresh(&app, &controller.borrow());
                }
            }
            consumed
        });
    }
}

fn status_of(result: std::io::Result<()>, path: &str, verb: &str) -> String {
    match result {
        Ok(()) => format!("{verb} {path}"),
        Err(e) => format!("Save failed: {e}"),
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let app = AppWindow::new()?;
    let controller =
        Rc::new(RefCell::new(Controller::new(DEFAULT_CANVAS_WIDTH, DEFAULT_CANVAS_HEIGHT)));

    let tool_labels: Vec<slint::SharedString> =
        Tool::ALL.iter().map(|t| slint::SharedString::from(t.label())).collect();
    app.set_tool_labels(ModelRc::new(VecModel::from(tool_labels)));

    refresh(&app, &controller.borrow());
    wire_callbacks(&app, controller);

    app.run()
}

/// Headless GUI behaviour tests: real `AppWindow` instances, real
/// `wire_callbacks`, real Slint input dispatch (via
/// `i-slint-backend-testing`) — no visible window or real display required.
#[cfg(test)]
mod gui_tests {
    use super::*;
    use i_slint_backend_testing::ElementHandle;
    use slint::platform::{Key, PointerEventButton, WindowEvent};
    use slint::{LogicalPosition, Model, SharedString};

    /// `init_no_event_loop` may only be called once per *thread*, and each
    /// `#[test]` function runs on its own thread by default, so call this at
    /// the top of every test rather than sharing a `static Once`.
    fn new_app_with_controller() -> (AppWindow, Rc<RefCell<Controller>>) {
        i_slint_backend_testing::init_no_event_loop();
        let app = AppWindow::new().unwrap();
        let controller =
            Rc::new(RefCell::new(Controller::new(DEFAULT_CANVAS_WIDTH, DEFAULT_CANVAS_HEIGHT)));
        let tool_labels: Vec<SharedString> =
            Tool::ALL.iter().map(|t| SharedString::from(t.label())).collect();
        app.set_tool_labels(ModelRc::new(VecModel::from(tool_labels)));
        refresh(&app, &controller.borrow());
        wire_callbacks(&app, controller.clone());
        (app, controller)
    }

    fn find_one(app: &AppWindow, id: &str) -> ElementHandle {
        let mut it = ElementHandle::find_by_element_id(app, id);
        it.next().unwrap_or_else(|| panic!("no element found with id {id:?}"))
    }

    /// Dispatches a real key press/release pair through the window.
    fn press_key(app: &AppWindow, text: &str) {
        let window = app.window();
        window.dispatch_event(WindowEvent::KeyPressed { text: text.into() });
        window.dispatch_event(WindowEvent::KeyReleased { text: text.into() });
    }

    /// Dispatches a key press with the platform accelerator held.
    fn press_accel(app: &AppWindow, text: &str) {
        let window = app.window();
        let ctrl: SharedString = Key::Control.into();
        window.dispatch_event(WindowEvent::KeyPressed { text: ctrl.clone() });
        window.dispatch_event(WindowEvent::KeyPressed { text: text.into() });
        window.dispatch_event(WindowEvent::KeyReleased { text: text.into() });
        window.dispatch_event(WindowEvent::KeyReleased { text: ctrl });
    }

    /// Draws one pixel at document (0,0) through the controller directly.
    fn draw_one_pixel(app: &AppWindow, controller: &Rc<RefCell<Controller>>) {
        {
            let mut c = controller.borrow_mut();
            c.begin_stroke(4.0, 4.0).unwrap();
            c.end_drag().unwrap();
        }
        refresh(app, &controller.borrow());
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
        assert_eq!(app.get_tool_labels().row_count(), Tool::ALL.len());
        assert_eq!(app.get_selected_tool(), 0, "the brush is the default tool");
        assert_eq!(app.get_brush_size(), 1);
        assert_eq!(app.get_layers().row_count(), 1);
        assert!(!app.get_has_selection());
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
        assert_eq!(
            controller.borrow().current_color(),
            controller.borrow().palette()[4],
            "selecting a swatch must change the drawing colour"
        );
    }

    #[test]
    fn dragging_on_the_canvas_draws_a_connected_stroke_and_enables_undo() {
        let (app, controller) = new_app_with_controller();
        let canvas = find_one(&app, "AppWindow::canvas-touch");
        let top_left = canvas.absolute_position();
        let window = app.window();

        // zoom = 8: doc (2,2) -> canvas-local (20,20); doc (5,2) -> (44,20).
        let start = LogicalPosition::new(top_left.x + 20.0, top_left.y + 20.0);
        let mid = LogicalPosition::new(top_left.x + 44.0, top_left.y + 20.0);

        window.dispatch_event(WindowEvent::PointerMoved { position: start });
        window.dispatch_event(WindowEvent::PointerPressed {
            position: start,
            button: PointerEventButton::Left,
        });
        window.dispatch_event(WindowEvent::PointerMoved { position: mid });
        window.dispatch_event(WindowEvent::PointerReleased {
            position: mid,
            button: PointerEventButton::Left,
        });

        let doc = controller.borrow().document().clone();
        let expected_color = controller.borrow().current_color();
        for x in 2..=5 {
            assert_eq!(
                doc.get_pixel(x, 2).unwrap(),
                expected_color,
                "pixel ({x}, 2) should be on the drawn line"
            );
        }
        assert!(app.get_can_undo());
    }

    #[test]
    fn one_pointer_drag_is_undone_by_a_single_undo_click() {
        // AC13. This is the whole point of Task 6's grouping, verified at
        // the level the user actually experiences it.
        let (app, controller) = new_app_with_controller();
        let canvas = find_one(&app, "AppWindow::canvas-touch");
        let origin = canvas.absolute_position();
        let window = app.window();

        let at = |x: f32| LogicalPosition::new(origin.x + x, origin.y + 20.0);
        window.dispatch_event(WindowEvent::PointerMoved { position: at(20.0) });
        window.dispatch_event(WindowEvent::PointerPressed {
            position: at(20.0),
            button: PointerEventButton::Left,
        });
        for step in 1..=6 {
            window
                .dispatch_event(WindowEvent::PointerMoved { position: at(20.0 + 8.0 * step as f32) });
        }
        window.dispatch_event(WindowEvent::PointerReleased {
            position: at(68.0),
            button: PointerEventButton::Left,
        });

        let painted = |c: &Controller| {
            let doc = c.document();
            doc.composite().chunks(4).filter(|p| p[3] != 0).count()
        };
        assert_eq!(painted(&controller.borrow()), 7, "a 7-pixel stroke was drawn");
        assert!(app.get_can_undo());

        find_one(&app, "AppWindow::undo-btn").mock_single_click(PointerEventButton::Left);

        assert_eq!(painted(&controller.borrow()), 0, "one Undo removed the whole drag");
        assert!(!app.get_can_undo(), "and landed exactly on the empty canvas");
        assert!(app.get_can_redo());
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
        window.dispatch_event(WindowEvent::PointerPressed {
            position: start,
            button: PointerEventButton::Right,
        });
        window.dispatch_event(WindowEvent::PointerMoved { position: end });
        window.dispatch_event(WindowEvent::PointerReleased {
            position: end,
            button: PointerEventButton::Right,
        });

        assert_eq!(controller.borrow().pan(), (20.0, 15.0));
        assert_eq!(
            controller.borrow().document(),
            &before,
            "panning must never mutate the document"
        );
        assert!(!app.get_can_undo(), "panning must not be recorded as a command");
        assert_eq!((app.get_pan_x(), app.get_pan_y()), (20.0, 15.0));
    }

    #[test]
    fn scroll_event_zooms_the_canvas() {
        let (app, controller) = new_app_with_controller();
        let canvas = find_one(&app, "AppWindow::canvas-touch");
        let top_left = canvas.absolute_position();
        let size = canvas.size();
        let center =
            LogicalPosition::new(top_left.x + size.width / 2.0, top_left.y + size.height / 2.0);

        assert_eq!(controller.borrow().zoom(), 8);

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
        draw_one_pixel(&app, &controller);
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
    fn keyboard_shortcuts_select_every_tool() {
        // AC14, first half: real key events through the real FocusScope.
        let (app, controller) = new_app_with_controller();
        for (index, tool) in Tool::ALL.iter().enumerate() {
            press_key(&app, &tool.shortcut().to_string());
            assert_eq!(
                controller.borrow().tool(),
                *tool,
                "key {:?} should select {:?}",
                tool.shortcut(),
                tool
            );
            assert_eq!(app.get_selected_tool(), index as i32, "the UI must reflect the change");
        }
    }

    #[test]
    fn the_accelerator_shortcut_undoes_and_redoes() {
        // AC14, second half.
        let (app, controller) = new_app_with_controller();
        draw_one_pixel(&app, &controller);
        let after_draw = controller.borrow().document().clone();

        press_accel(&app, "z");
        assert_ne!(controller.borrow().document(), &after_draw, "Ctrl/Cmd+Z must undo");
        assert!(app.get_can_redo());

        press_accel(&app, "Z"); // Shift+Ctrl/Cmd+Z arrives uppercase
        assert_eq!(controller.borrow().document(), &after_draw, "Shift+Ctrl/Cmd+Z must redo");
    }

    #[test]
    fn bracket_keys_change_the_brush_size_and_digits_pick_swatches() {
        let (app, controller) = new_app_with_controller();
        press_key(&app, "]");
        press_key(&app, "]");
        assert_eq!(controller.borrow().brush_size(), 3);
        assert_eq!(app.get_brush_size(), 3);

        press_key(&app, "[");
        assert_eq!(controller.borrow().brush_size(), 2);

        press_key(&app, "5");
        assert_eq!(controller.borrow().selected_palette_index(), 4, "digit 5 -> index 4");
        assert_eq!(app.get_selected_index(), 4);
    }

    #[test]
    fn an_unbound_key_is_not_consumed() {
        let (app, controller) = new_app_with_controller();
        let before = controller.borrow().tool();
        press_key(&app, "q");
        assert_eq!(controller.borrow().tool(), before);
    }

    #[test]
    fn the_tool_buttons_select_tools() {
        let (app, controller) = new_app_with_controller();
        let buttons: Vec<ElementHandle> =
            ElementHandle::find_by_element_id(&app, "AppWindow::tool-btn").collect();
        assert_eq!(buttons.len(), Tool::ALL.len());

        buttons[2].mock_single_click(PointerEventButton::Left); // Fill
        assert_eq!(controller.borrow().tool(), Tool::Fill);
        assert_eq!(app.get_selected_tool(), 2);
    }

    #[test]
    fn the_brush_size_buttons_clamp_at_one() {
        let (app, controller) = new_app_with_controller();
        find_one(&app, "AppWindow::brush-up-btn").mock_single_click(PointerEventButton::Left);
        assert_eq!(controller.borrow().brush_size(), 2);
        for _ in 0..5 {
            find_one(&app, "AppWindow::brush-down-btn")
                .mock_single_click(PointerEventButton::Left);
        }
        assert_eq!(controller.borrow().brush_size(), 1, "brush size never drops below 1");
    }

    #[test]
    fn editing_a_swatch_through_the_ui_records_a_palette_command() {
        // AC15.
        let (app, controller) = new_app_with_controller();
        app.invoke_palette_clicked(3);
        app.set_swatch_hex("#0a141e".into());
        find_one(&app, "AppWindow::apply-swatch-btn").mock_single_click(PointerEventButton::Left);

        assert_eq!(controller.borrow().palette()[3], [0x0a, 0x14, 0x1e, 0xff]);
        assert_eq!(controller.borrow().current_color(), [0x0a, 0x14, 0x1e, 0xff]);

        let dir = std::env::temp_dir().join(format!("pixelcad-gui-swatch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("s.pxc");
        app.invoke_save_clicked(path.to_str().unwrap().into());
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.contains(r##"palette.set index=3 color="#0a141eff""##),
            "the swatch edit must be replayable; got:\n{text}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_invalid_swatch_hex_reports_an_error_and_changes_nothing() {
        let (app, controller) = new_app_with_controller();
        let before = controller.borrow().palette()[0];
        app.set_swatch_hex("lavender".into());
        find_one(&app, "AppWindow::apply-swatch-btn").mock_single_click(PointerEventButton::Left);

        assert_eq!(controller.borrow().palette()[0], before);
        assert!(
            app.get_status_message().contains("Not a colour"),
            "got: {}",
            app.get_status_message()
        );
    }

    #[test]
    fn layers_can_be_added_drawn_on_hidden_and_deleted_through_the_ui() {
        // AC16.
        let (app, controller) = new_app_with_controller();
        assert_eq!(app.get_layers().row_count(), 1);

        find_one(&app, "AppWindow::layer-add-btn").mock_single_click(PointerEventButton::Left);
        assert_eq!(app.get_layers().row_count(), 2);
        assert_eq!(app.get_active_layer(), 1);
        // Top-first display order: row 0 is the newest layer.
        assert_eq!(app.get_layers().row_data(0).unwrap().name, "Layer 2");

        draw_one_pixel(&app, &controller);
        assert_eq!(
            controller.borrow().document().get_pixel_on(0, 0, 0).unwrap(),
            [0, 0, 0, 0],
            "the drawing went to the new layer, not the old one"
        );

        // Hide the top layer via its checkbox (row 0 = layer index 1).
        let boxes: Vec<ElementHandle> =
            ElementHandle::find_by_element_id(&app, "AppWindow::layer-visible-box").collect();
        assert_eq!(boxes.len(), 2);
        boxes[0].mock_single_click(PointerEventButton::Left);
        assert!(!controller.borrow().layer_visible(1));
        assert!(
            controller.borrow().document().composite().chunks(4).all(|p| p[3] == 0),
            "hiding the layer must hide its pixels from the composite"
        );

        find_one(&app, "AppWindow::layer-remove-btn").mock_single_click(PointerEventButton::Left);
        assert_eq!(app.get_layers().row_count(), 1);
        assert!(app.get_status_message().contains("Layer deleted"));
    }

    #[test]
    fn selecting_a_layer_row_makes_it_active() {
        let (app, controller) = new_app_with_controller();
        find_one(&app, "AppWindow::layer-add-btn").mock_single_click(PointerEventButton::Left);
        assert_eq!(controller.borrow().active_layer_index(), 1);

        // Row 1 in the top-first list is the bottom layer (index 0).
        let rows: Vec<ElementHandle> =
            ElementHandle::find_by_element_id(&app, "AppWindow::layer-row-touch").collect();
        rows[1].mock_single_click(PointerEventButton::Left);
        assert_eq!(controller.borrow().active_layer_index(), 0);
        assert_eq!(app.get_active_layer(), 0);
    }

    #[test]
    fn a_project_saved_from_the_ui_reopens_identically() {
        let (app, controller) = new_app_with_controller();
        find_one(&app, "AppWindow::layer-add-btn").mock_single_click(PointerEventButton::Left);
        draw_one_pixel(&app, &controller);
        let expected = controller.borrow().document().content_hash();

        let dir = std::env::temp_dir().join(format!("pixelcad-gui-proj-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("drawing.pxcproj");
        let path_str: SharedString = path.to_str().unwrap().into();

        app.invoke_save_project_clicked(path_str.clone());
        assert!(app.get_status_message().contains("Saved project to"));
        assert!(std::fs::read_to_string(&path).unwrap().contains("pixelcad.project version=1"));

        // Wipe the session by drawing something else, then reopen.
        draw_one_pixel(&app, &controller);
        app.invoke_open_project_clicked(path_str);
        assert!(app.get_status_message().contains("Opened"));
        assert_eq!(controller.borrow().document().content_hash(), expected);
        assert_eq!(app.get_layers().row_count(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failed_open_reports_the_error_and_preserves_the_open_document() {
        let (app, controller) = new_app_with_controller();
        draw_one_pixel(&app, &controller);
        let before = controller.borrow().document().clone();

        app.invoke_open_project_clicked("/definitely/not/a/real/path.pxcproj".into());

        assert!(
            app.get_status_message().contains("Open failed"),
            "got: {}",
            app.get_status_message()
        );
        assert_eq!(
            controller.borrow().document(),
            &before,
            "a failed open must never damage the user's work"
        );
    }

    #[test]
    fn the_select_tool_sets_a_marquee_and_the_clear_button_removes_it() {
        let (app, controller) = new_app_with_controller();
        press_key(&app, "m");
        assert_eq!(controller.borrow().tool(), Tool::Select);
        assert!(!app.get_has_selection());

        let canvas = find_one(&app, "AppWindow::canvas-touch");
        let origin = canvas.absolute_position();
        let window = app.window();
        let start = LogicalPosition::new(origin.x + 16.0, origin.y + 16.0);
        let end = LogicalPosition::new(origin.x + 40.0, origin.y + 40.0);
        window.dispatch_event(WindowEvent::PointerMoved { position: start });
        window.dispatch_event(WindowEvent::PointerPressed {
            position: start,
            button: PointerEventButton::Left,
        });
        window.dispatch_event(WindowEvent::PointerMoved { position: end });
        window.dispatch_event(WindowEvent::PointerReleased {
            position: end,
            button: PointerEventButton::Left,
        });

        assert!(app.get_has_selection(), "the marquee must be reflected in the UI");
        find_one(&app, "AppWindow::deselect-btn").mock_single_click(PointerEventButton::Left);
        assert!(!app.get_has_selection());
    }

    /// The compiled `pixelcad-cli` binary, located relative to this test
    /// binary's own path (`target/<profile>/deps/pixelcad_app-<hash>` ->
    /// `target/<profile>/pixelcad-cli`). Precondition: something must have
    /// built the `pixelcad-cli` bin target in this profile already — true
    /// whenever this is run as part of `cargo test --workspace`.
    fn cli_binary_path() -> std::path::PathBuf {
        let mut path = std::env::current_exe().unwrap();
        path.pop(); // deps
        path.pop(); // <profile>
        path.push("pixelcad-cli");
        path
    }

    #[test]
    fn save_button_output_replays_through_the_real_cli_binary() {
        let (app, controller) = new_app_with_controller();
        {
            let mut c = controller.borrow_mut();
            c.select_palette(6);
            c.begin_stroke(4.0, 4.0).unwrap();
            c.continue_drag(60.0, 4.0).unwrap();
            c.end_drag().unwrap();
        }
        refresh(&app, &controller.borrow());

        let dir = std::env::temp_dir()
            .join(format!("pixelcad-app-cli-interop-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script_path = dir.join("gui_drawn.pxc");
        app.invoke_save_clicked(script_path.to_str().unwrap().into());

        let cli = cli_binary_path();
        assert!(
            cli.exists(),
            "pixelcad-cli binary not found at {}; run `cargo build --workspace` first",
            cli.display()
        );

        let out_path = dir.join("gui_drawn.png");
        let status = std::process::Command::new(&cli)
            .args(["run", script_path.to_str().unwrap(), "--out", out_path.to_str().unwrap()])
            .status()
            .expect("failed to launch pixelcad-cli");
        assert!(status.success(), "pixelcad-cli exited with failure");
        assert!(out_path.exists(), "pixelcad-cli did not produce a PNG");
        assert!(std::fs::metadata(&out_path).unwrap().len() > 0, "PNG must be non-empty");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_button_writes_a_pxc_file_the_cli_can_replay() {
        let (app, controller) = new_app_with_controller();
        {
            let mut c = controller.borrow_mut();
            c.select_palette(4);
            c.begin_stroke(4.0, 4.0).unwrap();
            c.end_drag().unwrap();
        }
        refresh(&app, &controller.borrow());

        let dir =
            std::env::temp_dir().join(format!("pixelcad-app-gui-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("saved.pxc");
        app.set_save_path(path.to_str().unwrap().into());

        app.invoke_save_clicked(path.to_str().unwrap().into());

        let text = std::fs::read_to_string(&path).unwrap();
        let commands = pixelcad_core::parse_script(&text).unwrap();
        let color = controller.borrow().current_color();
        assert_eq!(
            commands,
            vec![
                pixelcad_core::Command::CanvasNew { width: 64, height: 64 },
                pixelcad_core::Command::ColorSet { color },
                pixelcad_core::Command::BrushStroke {
                    x0: 0,
                    y0: 0,
                    x1: 0,
                    y1: 0,
                    size: 1, shape: pixelcad_core::BrushShape::Square,
                    color
                },
            ]
        );
        assert!(app.get_status_message().contains("Saved script to"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
