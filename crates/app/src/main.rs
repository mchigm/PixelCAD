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
mod settings;

slint::include_modules!();

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use slint::{
    Color as SlintColor, ComponentHandle, Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, VecModel,
};

use controller::{Controller, Tool};
use pixelcad_core::{Axis, BrushShape};

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
    app.set_grid_on(controller.grid_enabled());
    app.set_has_clipboard(controller.clipboard().is_some());
    app.set_zoom_level(controller.zoom() as i32);
    let recent: Vec<SlintColor> =
        controller.recent_colors().iter().copied().map(to_slint_color).collect();
    app.set_recent_colors(ModelRc::new(VecModel::from(recent)));
    app.set_brush_size(controller.brush_size() as i32);
    app.set_brush_shape(controller.brush_shape().name().into());
    app.set_tolerance(controller.tolerance() as i32);
    app.set_stroke_opacity(controller.opacity() as i32);
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

    // Named keys arrive as multi-character strings, so they are matched
    // before the single-character table.
    match text {
        "\u{7f}" | "\u{8}" => {
            // Delete / Backspace clear the selection's pixels. With no
            // selection this is deliberately a no-op rather than "erase
            // everything", which would be an unrecoverable surprise.
            let _ = controller.delete_selection();
            return true;
        }
        "\u{f700}" => return nudge(controller, 0, -1),
        "\u{f701}" => return nudge(controller, 0, 1),
        "\u{f702}" => return nudge(controller, -1, 0),
        "\u{f703}" => return nudge(controller, 1, 0),
        _ => {}
    }

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
            'a' => {
                let _ = controller.select_all();
                true
            }
            // Shift+Cmd/Ctrl+C is Paint's "copy visible layers", a
            // genuinely different operation from an ordinary copy.
            'c' if ch.is_uppercase() => {
                controller.copy_composite();
                true
            }
            'c' => {
                controller.copy();
                true
            }
            'x' => {
                let _ = controller.cut();
                true
            }
            'v' => {
                let _ = controller.paste();
                true
            }
            'd' if ch.is_uppercase() => {
                let _ = controller.clear_selection();
                true
            }
            'd' => {
                let _ = controller.duplicate_selection();
                true
            }
            'j' => {
                let _ = controller.duplicate_active_layer();
                true
            }
            'e' => {
                let _ = controller.merge_active_layer_down();
                true
            }
            'n' => {
                let _ = controller.add_layer();
                true
            }
            'h' => {
                let _ = controller.flip_selection(Axis::Horizontal);
                true
            }
            'u' => {
                let _ = controller.flip_selection(Axis::Vertical);
                true
            }
            'r' => {
                let _ = controller.rotate_selection(90);
                true
            }
            '0' => {
                controller.zoom_to_actual_size();
                true
            }
            '9' => {
                controller.zoom_to_fit();
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
        'g' | 'G' => {
            // Note: 'g' is the fill tool's shortcut and is consumed above,
            // so this is only reached for the uppercase form.
            controller.toggle_grid();
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

/// Arrow-key nudge: moves the selected pixels by one.
fn nudge(controller: &mut Controller, dx: i64, dy: i64) -> bool {
    controller.nudge(dx, dy).is_ok()
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
    on!(on_brush_shape_toggled, |c| {
        let next = match c.brush_shape() {
            BrushShape::Square => BrushShape::Round,
            BrushShape::Round => BrushShape::Square,
        };
        c.set_brush_shape(next);
    });
    on!(on_tolerance_changed, |c, value: i32| {
        c.set_tolerance(value.clamp(0, 255) as u8);
    });
    on!(on_stroke_opacity_changed, |c, value: i32| {
        c.set_opacity(value.clamp(0, 255) as u8);
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
    on!(on_select_all_clicked, |c| {
        let _ = c.select_all();
    });
    on!(on_delete_clicked, |c| {
        let _ = c.delete_selection();
    });
    on!(on_cut_clicked, |c| {
        let _ = c.cut();
    });
    on!(on_copy_clicked, |c| {
        c.copy();
    });
    on!(on_copy_composite_clicked, |c| {
        c.copy_composite();
    });
    on!(on_paste_clicked, |c| {
        let _ = c.paste();
    });
    on!(on_rotate_clicked, |c| {
        let _ = c.rotate_selection(90);
    });
    on!(on_crop_clicked, |c| {
        let _ = c.crop_to_selection();
    });
    on!(on_grid_toggled, |c| {
        c.toggle_grid();
    });
    on!(on_zoom_fit_clicked, |c| {
        c.zoom_to_fit();
    });
    on!(on_viewport_resized, |c, w: f32, h: f32| {
        // Pure view state, never recorded. Kept current so zoom-to-fit uses
        // the real viewport instead of a guess. Slint fires size changes on
        // every layout pass, so ignore the ones that change nothing.
        let size = (w.max(1.0) as u32, h.max(1.0) as u32);
        if c.viewport() != size {
            c.set_viewport(size.0, size.1);
        }
    });
    on!(on_zoom_actual_clicked, |c| {
        c.zoom_to_actual_size();
    });
    on!(on_layer_duplicate_clicked, |c| {
        let _ = c.duplicate_active_layer();
    });
    on!(on_layer_merge_clicked, |c| {
        let _ = c.merge_active_layer_down();
    });
    on!(on_layer_up_clicked, |c| {
        let to = c.active_layer_index() + 1;
        let _ = c.move_active_layer(to);
    });
    on!(on_layer_down_clicked, |c| {
        let to = c.active_layer_index().saturating_sub(1);
        let _ = c.move_active_layer(to);
    });
    on!(on_recent_clicked, |c, index: i32| {
        if let Some(color) = c.recent_colors().get(index.max(0) as usize).copied() {
            let _ = c.set_color(color);
        }
    });

    // Callbacks that also report a status message need the app handle
    // inside the body, so they are written out longhand.
    {
        let controller = controller.clone();
        let app_weak = app.as_weak();
        app.on_resize_canvas_clicked(move |w, h| {
            let message = match parse_size(&w, &h) {
                Some((w, h)) => match controller.borrow_mut().resize_canvas(w, h) {
                    Ok(()) => format!("Resized to {w}x{h}"),
                    Err(e) => format!("Resize failed: {e}"),
                },
                None => "Width and height must be positive whole numbers".to_string(),
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
        app.on_scale_selection_clicked(move |w, h| {
            let message = match parse_size(&w, &h) {
                Some((w, h)) => match controller.borrow_mut().scale_selection(w, h) {
                    Ok(()) => format!("Selection scaled to {w}x{h}"),
                    Err(e) => format!("Scale failed: {e}"),
                },
                None => "Width and height must be positive whole numbers".to_string(),
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
        app.on_flip_clicked(move |axis| {
            let axis = if axis.as_str() == "vertical" { Axis::Vertical } else { Axis::Horizontal };
            let _ = controller.borrow_mut().flip_selection(axis);
            if let Some(app) = app_weak.upgrade() {
                refresh(&app, &controller.borrow());
            }
        });
    }
    {
        let controller = controller.clone();
        app.on_text_changed(move |text| {
            // Pure tool state: no re-render needed and nothing recorded
            // until the text is actually stamped on the canvas.
            let mut c = controller.borrow_mut();
            if c.text_buffer() != text.as_str() {
                c.set_text_buffer(text.as_str());
            }
        });
    }
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

/// Parses the two size fields, rejecting anything that is not a positive
/// whole number. Returning `None` rather than clamping means a typo is
/// reported instead of silently resizing to something the user did not ask
/// for.
fn parse_size(w: &str, h: &str) -> Option<(u32, u32)> {
    let w: u32 = w.trim().parse().ok()?;
    let h: u32 = h.trim().parse().ok()?;
    (w > 0 && h > 0).then_some((w, h))
}

fn status_of(result: std::io::Result<()>, path: &str, verb: &str) -> String {
    match result {
        Ok(()) => format!("{verb} {path}"),
        Err(e) => format!("Save failed: {e}"),
    }
}

/// Pushes persisted settings into the UI: which chrome to render and how
/// it is themed.
fn apply_settings(app: &AppWindow, s: &settings::AppSettings) {
    app.set_ui_style(s.ui_style.as_ui_string().into());
    let theme = app.global::<Theme>();
    theme.set_scheme(s.theme.as_ui_string().into());
    theme.set_design(s.color_design.as_ui_string().into());
    theme.set_text_scale(s.text_style.scale());
    theme.set_mono_labels(s.mono_labels);
    theme.set_icon_size(s.icon_size as f32);
    // An empty or invalid accent means "follow the colour design", which
    // the Slint side represents as `transparent`.
    theme.set_accent_override(
        parse_hex_color(&s.accent_color).unwrap_or(SlintColor::from_argb_u8(0, 0, 0, 0)),
    );
}

/// `#rrggbb` -> Slint colour. Settings validation already guarantees the
/// shape, so this only has to handle the parse itself.
fn parse_hex_color(hex: &str) -> Option<SlintColor> {
    let body = hex.strip_prefix('#')?;
    if body.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&body[0..2], 16).ok()?;
    let g = u8::from_str_radix(&body[2..4], 16).ok()?;
    let b = u8::from_str_radix(&body[4..6], 16).ok()?;
    Some(SlintColor::from_rgb_u8(r, g, b))
}

/// Mirrors persisted settings into the Settings page's own properties, so
/// the page opens showing what is actually saved.
fn push_settings_form(app: &AppWindow, s: &settings::AppSettings) {
    app.set_set_ui_style(
        match s.ui_style {
            settings::UiStyle::Auto => "auto",
            settings::UiStyle::Linux => "linux",
            settings::UiStyle::MacOs => "macos",
        }
        .into(),
    );
    app.set_set_scheme(s.theme.as_ui_string().into());
    app.set_set_design(s.color_design.as_ui_string().into());
    app.set_set_text_style(
        match s.text_style {
            settings::TextStyle::Compact => "compact",
            settings::TextStyle::Normal => "normal",
            settings::TextStyle::Comfortable => "comfortable",
        }
        .into(),
    );
    app.set_set_mono_labels(s.mono_labels);
    app.set_set_accent(s.accent_color.clone().into());
    app.set_set_icon_size(s.icon_size.to_string().into());
    app.set_set_canvas_w(s.default_canvas_width.to_string().into());
    app.set_set_canvas_h(s.default_canvas_height.to_string().into());
    app.set_set_autosave(s.autosave_enabled);
    app.set_set_autosave_interval(s.autosave_interval_secs.to_string().into());
    app.set_set_confirm_exit(s.confirm_before_exit);
    app.set_set_recent_len(s.recent_files_len.to_string().into());
}

/// Reads the Settings page back into an `AppSettings`.
///
/// Unparseable numeric fields keep the previous value rather than resetting
/// to a default: the user is mid-edit, and a half-typed number must not
/// silently overwrite what they had.
fn collect_settings(app: &AppWindow, previous: &settings::AppSettings) -> settings::AppSettings {
    let mut s = previous.clone();
    s.ui_style = match app.get_set_ui_style().as_str() {
        "linux" => settings::UiStyle::Linux,
        "macos" => settings::UiStyle::MacOs,
        _ => settings::UiStyle::Auto,
    };
    s.theme = match app.get_set_scheme().as_str() {
        "light" => settings::Theme::Light,
        "high-contrast" => settings::Theme::HighContrast,
        _ => settings::Theme::Dark,
    };
    s.color_design = match app.get_set_design().as_str() {
        "graphite" => settings::ColorDesign::Graphite,
        "amber" => settings::ColorDesign::Amber,
        "phosphor" => settings::ColorDesign::Phosphor,
        _ => settings::ColorDesign::Cyanotype,
    };
    s.text_style = match app.get_set_text_style().as_str() {
        "compact" => settings::TextStyle::Compact,
        "comfortable" => settings::TextStyle::Comfortable,
        _ => settings::TextStyle::Normal,
    };
    s.mono_labels = app.get_set_mono_labels();
    s.accent_color = app.get_set_accent().trim().to_string();
    if let Ok(v) = app.get_set_icon_size().parse::<u32>() {
        s.icon_size = v;
    }
    if let Ok(v) = app.get_set_canvas_w().trim().parse::<u32>() {
        s.default_canvas_width = v;
    }
    if let Ok(v) = app.get_set_canvas_h().trim().parse::<u32>() {
        s.default_canvas_height = v;
    }
    s.autosave_enabled = app.get_set_autosave();
    if let Ok(v) = app.get_set_autosave_interval().trim().parse::<u32>() {
        s.autosave_interval_secs = v;
    }
    s.confirm_before_exit = app.get_set_confirm_exit();
    if let Ok(v) = app.get_set_recent_len().trim().parse::<u32>() {
        s.recent_files_len = v;
    }
    s.sanitize();
    s
}

/// Wires the Settings page and the first-run platform dialog.
///
/// `save` is injected rather than called directly so the headless tests can
/// persist into a temporary directory instead of the real user config.
fn wire_settings(
    app: &AppWindow,
    initial: settings::AppSettings,
    save: Rc<dyn Fn(&settings::AppSettings)>,
) {
    let current = Rc::new(RefCell::new(initial));

    {
        let app_weak = app.as_weak();
        let current = current.clone();
        let save = save.clone();
        app.on_settings_changed(move || {
            let Some(app) = app_weak.upgrade() else { return };
            let next = collect_settings(&app, &current.borrow());
            apply_settings(&app, &next);
            // Write the sanitised values back so the form cannot keep
            // showing something that was rejected.
            push_settings_form(&app, &next);
            save(&next);
            *current.borrow_mut() = next;
        });
    }

    let app_weak = app.as_weak();
    app.on_first_run_answered(move |choice| {
        let Some(app) = app_weak.upgrade() else { return };
        let mut next = current.borrow().clone();
        next.ui_style = match choice.as_str() {
            "linux" => settings::UiStyle::Linux,
            "macos" => settings::UiStyle::MacOs,
            // "detected" keeps Auto, so the choice still follows the OS if
            // the same config is later used on another machine.
            _ => settings::UiStyle::Auto,
        };
        next.first_run_completed = true;
        apply_settings(&app, &next);
        push_settings_form(&app, &next);
        save(&next);
        *current.borrow_mut() = next;
        app.set_first_run_open(false);
    });
}

/// Wires the real fullscreen toggle.
///
/// Slint exposes fullscreen only on the Rust `Window`, not as a `.slint`
/// property, so the flow is: UI asks -> Rust flips the real window ->
/// Rust mirrors `is_fullscreen()` back into the `is-fullscreen` property
/// the macOS chrome switches its layout on. Mirroring the *queried* state
/// rather than the requested one means the UI cannot drift out of step
/// with a window manager that refused the request.
fn wire_fullscreen(app: &AppWindow) {
    let app_weak = app.as_weak();
    app.on_fullscreen_toggled(move || {
        if let Some(app) = app_weak.upgrade() {
            let window = app.window();
            window.set_fullscreen(!window.is_fullscreen());
            app.set_is_fullscreen(window.is_fullscreen());
        }
    });
}

fn main() -> Result<(), slint::PlatformError> {
    let app = AppWindow::new()?;
    let controller =
        Rc::new(RefCell::new(Controller::new(DEFAULT_CANVAS_WIDTH, DEFAULT_CANVAS_HEIGHT)));

    let tool_labels: Vec<slint::SharedString> =
        Tool::ALL.iter().map(|t| slint::SharedString::from(t.label())).collect();
    app.set_tool_labels(ModelRc::new(VecModel::from(tool_labels)));

    let (loaded, outcome) = settings::load();
    apply_settings(&app, &loaded);
    push_settings_form(&app, &loaded);
    if let settings::LoadOutcome::Corrupt(reason) = &outcome {
        // Never silently discard a user's configuration.
        app.set_status_message(format!("Settings could not be read ({reason}); using defaults").into());
    }

    // Detected but confirmed, never silently imposed.
    let detection = settings::detect_platform();
    app.set_detected_platform(detection.platform.to_string().into());
    app.set_windows_fallback(detection.windows_fallback);
    app.set_first_run_open(!loaded.first_run_completed);

    {
        let settings_open = app.as_weak();
        app.on_settings_clicked(move || {
            if let Some(app) = settings_open.upgrade() {
                app.set_settings_open(true);
            }
        });
    }

    refresh(&app, &controller.borrow());
    wire_callbacks(&app, controller);
    wire_fullscreen(&app);
    wire_settings(&app, loaded, Rc::new(|s| {
        if let Err(e) = settings::save(s) {
            eprintln!("could not save settings: {e}");
        }
    }));

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
        // The chrome is layout-driven, and Slint only assigns geometry once
        // the window has a size. Without this, most controls stay 0x0 and
        // are absent from the element tree the tests search.
        app.window().set_size(slint::PhysicalSize::new(1600, 1000));
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

    /// Number of non-transparent pixels in the composited document.
    fn painted_count(controller: &Rc<RefCell<Controller>>) -> usize {
        controller.borrow().document().composite().chunks(4).filter(|p| p[3] != 0).count()
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
            ElementHandle::find_by_element_id(&app, "HomeToolbar::swatch-row1").collect();
        swatches.extend(ElementHandle::find_by_element_id(&app, "HomeToolbar::swatch-row2"));
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
        let canvas = find_one(&app, "CanvasArea::canvas-touch");
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
        let canvas = find_one(&app, "CanvasArea::canvas-touch");
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

        find_one(&app, "LinuxChrome::undo-btn").mock_single_click(PointerEventButton::Left);

        assert_eq!(painted(&controller.borrow()), 0, "one Undo removed the whole drag");
        assert!(!app.get_can_undo(), "and landed exactly on the empty canvas");
        assert!(app.get_can_redo());
    }

    #[test]
    fn right_click_drag_pans_the_view_without_touching_the_document() {
        let (app, controller) = new_app_with_controller();
        let canvas = find_one(&app, "CanvasArea::canvas-touch");
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
        let canvas = find_one(&app, "CanvasArea::canvas-touch");
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

        find_one(&app, "LinuxChrome::undo-btn").mock_single_click(PointerEventButton::Left);
        assert!(!app.get_can_undo());
        assert!(app.get_can_redo());
        assert_ne!(controller.borrow().document(), &after_draw);

        find_one(&app, "LinuxChrome::redo-btn").mock_single_click(PointerEventButton::Left);
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
            ElementHandle::find_by_element_id(&app, "ToolButton::tool-btn").collect();
        assert_eq!(buttons.len(), Tool::ALL.len(), "one button per tool in Tool::ALL");

        // The redesign groups the tools visually (select / stationaries /
        // shapes), so document order no longer matches `Tool::ALL` order.
        // Locating by accessible label keeps the original assertion —
        // "clicking the Fill button selects the Fill tool" — while being
        // independent of where the group happens to sit in the toolbar.
        let fill = ElementHandle::find_by_accessible_label(&app, Tool::Fill.label())
            .next()
            .expect("the Fill tool button must be reachable by its accessible label");
        fill.mock_single_click(PointerEventButton::Left);
        assert_eq!(controller.borrow().tool(), Tool::Fill);
        assert_eq!(app.get_selected_tool(), 2);
    }

    /// Counts elements carrying an exact accessible label.
    fn labelled(app: &AppWindow, label: &str) -> usize {
        ElementHandle::find_by_accessible_label(app, label).count()
    }

    fn switch_to_macos(app: &AppWindow) {
        app.set_ui_style("macos".into());
        relayout(app);
    }

    /// Forces a fresh layout pass. Conditional chrome (`if ui-style ==`,
    /// `if is-fullscreen`) is instantiated lazily, and newly created items
    /// have no geometry — and therefore do not appear in the element tree —
    /// until layout runs again. Setting a *different* size guarantees that.
    fn relayout(app: &AppWindow) {
        // Two *different* sizes, unconditionally: a single set_size that
        // happens to match the current size is a no-op, and then the newly
        // instantiated items never receive geometry.
        app.window().set_size(slint::PhysicalSize::new(1280, 900));
        app.window().set_size(slint::PhysicalSize::new(1600, 1000));
    }

    #[test]
    fn the_ui_style_property_switches_between_the_two_chrome_trees() {
        let (app, _c) = new_app_with_controller();
        // Linux-only: the ribbon tab strip.
        assert_eq!(labelled(&app, "Home"), 1, "Linux chrome shows the ribbon tabs");
        assert_eq!(labelled(&app, "Files"), 0, "the macOS Files menu is not in the Linux tree");

        switch_to_macos(&app);

        assert_eq!(labelled(&app, "Files"), 1, "macOS chrome shows the Files menu");
        assert_eq!(labelled(&app, "Home"), 0, "the Linux ribbon is gone once macOS renders");
    }

    #[test]
    fn the_macos_chrome_exposes_the_horizontal_tab_strip_from_the_sketch() {
        // Per the maintainer's spec the macOS tabs are icons in a horizontal
        // strip along the bottom edge, not a left-hand rail.
        let (app, _c) = new_app_with_controller();
        switch_to_macos(&app);
        for tab in ["File", "Main", "Style", "Secondary", "Tool", "Command", "Layers", "AI"] {
            assert!(labelled(&app, tab) >= 1, "macOS tab icon {tab:?} must be present");
        }
    }

    #[test]
    fn clicking_a_tab_icon_expands_its_panel_upward_and_closing_collapses_it() {
        let (app, _c) = new_app_with_controller();
        switch_to_macos(&app);

        // Nothing expanded yet: the Layers panel is not in the tree.
        assert_eq!(
            ElementHandle::find_by_element_id(&app, "LayersPanel::layer-add-btn").count(),
            0,
            "no tab panel before any icon is clicked"
        );

        // Clicking the Layers icon expands its panel upward.
        let layers_icon = ElementHandle::find_by_accessible_label(&app, "Layers")
            .next()
            .expect("the Layers tab icon must exist");
        layers_icon.mock_single_click(PointerEventButton::Left);
        relayout(&app);
        assert_eq!(
            ElementHandle::find_by_element_id(&app, "LayersPanel::layer-add-btn").count(),
            1,
            "the Layers panel expanded and shows its real layer controls"
        );

        // Switching tabs swaps the content in place.
        let tool_icon = ElementHandle::find_by_accessible_label(&app, "Tool")
            .next()
            .expect("the Tool tab icon must exist");
        tool_icon.mock_single_click(PointerEventButton::Left);
        relayout(&app);
        assert_eq!(
            ElementHandle::find_by_element_id(&app, "LayersPanel::layer-add-btn").count(),
            0,
            "switching tabs replaced the Layers content"
        );

        // The panel's own close button collapses it.
        ElementHandle::find_by_accessible_label(&app, "Close panel")
            .next()
            .expect("the expanded panel has a close button")
            .mock_single_click(PointerEventButton::Left);
        relayout(&app);
        assert_eq!(
            labelled(&app, "Close panel"),
            0,
            "closing the panel removes it from the tree"
        );

        // ...and clicking the same icon a second time also collapses it.
        layers_icon.mock_single_click(PointerEventButton::Left);
        relayout(&app);
        assert_eq!(
            ElementHandle::find_by_element_id(&app, "LayersPanel::layer-add-btn").count(),
            1,
            "re-clicking the Layers icon re-expands it"
        );
        layers_icon.mock_single_click(PointerEventButton::Left);
        relayout(&app);
        assert_eq!(
            ElementHandle::find_by_element_id(&app, "LayersPanel::layer-add-btn").count(),
            0,
            "a second click on the same icon collapses it again"
        );
    }

    #[test]
    fn fullscreen_visibly_restructures_the_macos_chrome() {
        // The maintainer asked to be able to *see* that windowed and
        // fullscreen differ. Windowed merges Secondary into the Head bar
        // and hides the pop-out affordances; fullscreen splits them out and
        // reveals a pop-out control per vertical tab.
        let (app, _c) = new_app_with_controller();
        switch_to_macos(&app);

        const POPOUT: &str = "Layers pop-out — WIP (not implemented yet)";

        app.set_is_fullscreen(false);
        relayout(&app);
        assert_eq!(labelled(&app, POPOUT), 0, "no pop-out affordance while windowed");

        app.set_is_fullscreen(true);
        relayout(&app);
        assert_eq!(labelled(&app, POPOUT), 1, "fullscreen reveals the pop-out affordance");

        // ...and it goes away again, so this is a live binding rather than
        // a one-way build-time branch.
        app.set_is_fullscreen(false);
        relayout(&app);
        assert_eq!(labelled(&app, POPOUT), 0);
    }

    #[test]
    fn the_fullscreen_toggle_drives_the_real_window_state() {
        // Proves the toggle is wired to slint::Window rather than only
        // flipping a UI flag: the property mirrors what the window reports.
        let (app, _c) = new_app_with_controller();
        wire_fullscreen(&app);
        switch_to_macos(&app);

        let before = app.window().is_fullscreen();
        app.invoke_fullscreen_toggled();
        assert_eq!(
            app.get_is_fullscreen(),
            app.window().is_fullscreen(),
            "the UI property must mirror the window's actual state"
        );
        assert_ne!(app.window().is_fullscreen(), before, "the window state changed");
    }

    /// Wires settings with an in-memory "disk" so nothing touches the real
    /// user config, and returns the handle to inspect what was saved.
    fn wire_settings_capturing(
        app: &AppWindow,
        initial: settings::AppSettings,
    ) -> Rc<RefCell<Vec<settings::AppSettings>>> {
        let saved = Rc::new(RefCell::new(Vec::new()));
        let sink = saved.clone();
        wire_settings(app, initial, Rc::new(move |s| sink.borrow_mut().push(s.clone())));
        saved
    }

    #[test]
    fn the_settings_page_opens_and_closes() {
        let (app, _c) = new_app_with_controller();
        assert!(!app.get_settings_open());

        // The chrome's Settings button raises `settings-clicked`; main()
        // binds that to opening the panel, so bind it the same way here.
        let w = app.as_weak();
        app.on_settings_clicked(move || {
            if let Some(a) = w.upgrade() {
                a.set_settings_open(true);
            }
        });
        ElementHandle::find_by_accessible_label(&app, "Settings")
            .next()
            .expect("a Settings button must exist in the chrome")
            .mock_single_click(PointerEventButton::Left);
        assert!(app.get_settings_open(), "clicking Settings opens the page");

        relayout(&app);
        ElementHandle::find_by_accessible_label(&app, "Close settings")
            .next()
            .expect("the settings page must have a close button")
            .mock_single_click(PointerEventButton::Left);
        assert!(!app.get_settings_open(), "closing the page hides it");
    }

    #[test]
    fn changing_the_colour_scheme_design_and_text_style_applies_and_persists() {
        let (app, _c) = new_app_with_controller();
        let saved = wire_settings_capturing(&app, settings::AppSettings::default());
        let theme = app.global::<Theme>();

        let base_dark = theme.get_surface_base();
        let accent_cyan = theme.get_accent();

        // Colour scheme: dark -> light must change the surfaces.
        app.set_set_scheme("light".into());
        app.invoke_settings_changed();
        assert_ne!(theme.get_surface_base(), base_dark, "light scheme changes surfaces");

        // Colour design: cyanotype -> phosphor must change the accent hue.
        app.set_set_design("phosphor".into());
        app.invoke_settings_changed();
        assert_ne!(theme.get_accent(), accent_cyan, "a new colour design changes the accent");

        // Text style: the scale drives both text and row heights.
        let body_normal = theme.get_text_body();
        let row_normal = theme.get_row_toolbar();
        app.set_set_text_style("comfortable".into());
        app.invoke_settings_changed();
        assert!(theme.get_text_body() > body_normal, "comfortable text is larger");
        assert!(theme.get_row_toolbar() > row_normal, "rows grow so larger text cannot clip");

        // Monospace labels.
        app.set_set_mono_labels(true);
        app.invoke_settings_changed();
        assert_eq!(theme.get_label_font(), "monospace");

        // Everything above was persisted, and the last write has all of it.
        let last = saved.borrow().last().cloned().expect("settings were saved");
        assert_eq!(last.theme, settings::Theme::Light);
        assert_eq!(last.color_design, settings::ColorDesign::Phosphor);
        assert_eq!(last.text_style, settings::TextStyle::Comfortable);
        assert!(last.mono_labels);
    }

    #[test]
    fn an_explicit_accent_overrides_the_design_and_an_empty_one_restores_it() {
        let (app, _c) = new_app_with_controller();
        let _saved = wire_settings_capturing(&app, settings::AppSettings::default());
        let theme = app.global::<Theme>();
        let design_accent = theme.get_accent();

        app.set_set_accent("#FF0000".into());
        app.invoke_settings_changed();
        assert_eq!(theme.get_accent(), slint::Color::from_rgb_u8(255, 0, 0));

        app.set_set_accent("".into());
        app.invoke_settings_changed();
        assert_eq!(theme.get_accent(), design_accent, "an empty accent follows the design again");
    }

    #[test]
    fn a_nonsense_accent_is_discarded_without_disturbing_the_rest() {
        let (app, _c) = new_app_with_controller();
        let saved = wire_settings_capturing(&app, settings::AppSettings::default());

        app.set_set_accent("not a colour".into());
        app.set_set_design("amber".into());
        app.invoke_settings_changed();

        let last = saved.borrow().last().cloned().unwrap();
        assert_eq!(last.accent_color, "", "the invalid accent was dropped");
        assert_eq!(last.color_design, settings::ColorDesign::Amber, "the valid change survived");
        // The form is rewritten with the sanitised value so it cannot keep
        // displaying something that was rejected.
        assert_eq!(app.get_set_accent(), "");
    }

    #[test]
    fn switching_interface_style_in_settings_swaps_the_chrome_live() {
        let (app, _c) = new_app_with_controller();
        let _saved = wire_settings_capturing(&app, settings::AppSettings::default());

        app.set_set_ui_style("linux".into());
        app.invoke_settings_changed();
        relayout(&app);
        assert_eq!(app.get_ui_style(), "linux");
        assert_eq!(labelled(&app, "Home"), 1, "the Linux ribbon is showing");

        app.set_set_ui_style("macos".into());
        app.invoke_settings_changed();
        relayout(&app);
        assert_eq!(app.get_ui_style(), "macos");
        assert_eq!(labelled(&app, "Files"), 1, "the macOS chrome replaced it without a restart");
    }

    #[test]
    fn the_first_run_dialog_appears_once_and_its_answer_is_persisted() {
        let (app, _c) = new_app_with_controller();
        let saved = wire_settings_capturing(&app, settings::AppSettings::default());

        // A fresh install: no settings file means first run.
        app.set_detected_platform("macOS".into());
        app.set_first_run_open(true);
        relayout(&app);
        ElementHandle::find_by_accessible_label(&app, "Use macOS")
            .next()
            .expect("the dialog offers the detected platform")
            .mock_single_click(PointerEventButton::Left);

        assert!(!app.get_first_run_open(), "answering dismisses the dialog");
        let last = saved.borrow().last().cloned().expect("the answer was persisted");
        assert!(last.first_run_completed, "so it is never asked again");
        assert_eq!(
            last.ui_style,
            settings::UiStyle::Auto,
            "accepting the detected platform keeps Auto, so the same config still \
             follows the OS on a different machine"
        );
    }

    #[test]
    fn picking_a_layout_explicitly_in_the_first_run_dialog_pins_it() {
        let (app, _c) = new_app_with_controller();
        let saved = wire_settings_capturing(&app, settings::AppSettings::default());
        app.set_first_run_open(true);
        relayout(&app);

        ElementHandle::find_by_accessible_label(&app, "Use macOS layout")
            .next()
            .expect("the dialog offers an explicit macOS choice")
            .mock_single_click(PointerEventButton::Left);

        let last = saved.borrow().last().cloned().unwrap();
        assert_eq!(last.ui_style, settings::UiStyle::MacOs, "an explicit pick is stored as such");
        assert!(last.first_run_completed);
    }

    #[test]
    fn the_settings_page_shows_every_documented_control() {
        // Stands in for eyeballing the overlay: if a row or an option is
        // dropped by a layout change, this fails instead of silently
        // shipping a settings page with a missing control.
        let (app, _c) = new_app_with_controller();
        app.set_settings_open(true);

        app.set_settings_tab(0);
        relayout(&app);
        for label in ["Basic", "Style", "Layout", "Close settings", "On", "Off"] {
            assert!(labelled(&app, label) >= 1, "Basic tab is missing {label:?}");
        }

        app.set_settings_tab(1);
        relayout(&app);
        for label in [
            "Auto", "Linux", "macOS",
            "Dark", "Light", "High contrast",
            "Cyanotype", "Graphite", "Amber", "Phosphor",
            "Compact", "Normal", "Comfortable",
            "14", "16", "20",
        ] {
            assert!(labelled(&app, label) >= 1, "Style tab is missing {label:?}");
        }
    }

    #[test]
    fn every_tool_button_is_labelled_for_assistive_technology() {
        // The grouped layout is only navigable if each tool carries its
        // name; this also guards the lookup the test above depends on.
        let (app, _controller) = new_app_with_controller();
        for tool in Tool::ALL {
            assert_eq!(
                ElementHandle::find_by_accessible_label(&app, tool.label()).count(),
                1,
                "exactly one labelled button for {:?}",
                tool
            );
        }
    }

    #[test]
    fn the_brush_size_buttons_clamp_at_one() {
        let (app, controller) = new_app_with_controller();
        find_one(&app, "HomeToolbar::brush-up-btn").mock_single_click(PointerEventButton::Left);
        assert_eq!(controller.borrow().brush_size(), 2);
        for _ in 0..5 {
            find_one(&app, "HomeToolbar::brush-down-btn")
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
        find_one(&app, "HomeToolbar::apply-swatch-btn").mock_single_click(PointerEventButton::Left);

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
        find_one(&app, "HomeToolbar::apply-swatch-btn").mock_single_click(PointerEventButton::Left);

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

        find_one(&app, "LayersPanel::layer-add-btn").mock_single_click(PointerEventButton::Left);
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
            ElementHandle::find_by_element_id(&app, "LayersPanel::layer-visible-box").collect();
        assert_eq!(boxes.len(), 2);
        boxes[0].mock_single_click(PointerEventButton::Left);
        assert!(!controller.borrow().layer_visible(1));
        assert!(
            controller.borrow().document().composite().chunks(4).all(|p| p[3] == 0),
            "hiding the layer must hide its pixels from the composite"
        );

        find_one(&app, "LayersPanel::layer-remove-btn").mock_single_click(PointerEventButton::Left);
        assert_eq!(app.get_layers().row_count(), 1);
        assert!(app.get_status_message().contains("Layer deleted"));
    }

    #[test]
    fn selecting_a_layer_row_makes_it_active() {
        let (app, controller) = new_app_with_controller();
        find_one(&app, "LayersPanel::layer-add-btn").mock_single_click(PointerEventButton::Left);
        assert_eq!(controller.borrow().active_layer_index(), 1);

        // Row 1 in the top-first list is the bottom layer (index 0).
        let rows: Vec<ElementHandle> =
            ElementHandle::find_by_element_id(&app, "LayersPanel::layer-row-touch").collect();
        rows[1].mock_single_click(PointerEventButton::Left);
        assert_eq!(controller.borrow().active_layer_index(), 0);
        assert_eq!(app.get_active_layer(), 0);
    }

    #[test]
    fn a_project_saved_from_the_ui_reopens_identically() {
        let (app, controller) = new_app_with_controller();
        find_one(&app, "LayersPanel::layer-add-btn").mock_single_click(PointerEventButton::Left);
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

        let canvas = find_one(&app, "CanvasArea::canvas-touch");
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
        find_one(&app, "HomeToolbar::deselect-btn").mock_single_click(PointerEventButton::Left);
        assert!(!app.get_has_selection());
    }


    // ------------------------------------------------ Phase 1.5 GUI tests

    #[test]
    fn select_all_and_delete_work_from_the_toolbar() {
        let (app, controller) = new_app_with_controller();
        draw_one_pixel(&app, &controller);
        assert_eq!(painted_count(&controller), 1);

        find_one(&app, "HomeToolbar::select-all-btn").mock_single_click(PointerEventButton::Left);
        assert!(app.get_has_selection());
        assert_eq!(controller.borrow().selection().unwrap().count(), 64 * 64);

        find_one(&app, "HomeToolbar::delete-btn").mock_single_click(PointerEventButton::Left);
        assert_eq!(painted_count(&controller), 0, "delete clears the selected pixels");
    }

    #[test]
    fn the_accelerator_selects_all_and_delete_clears() {
        let (app, controller) = new_app_with_controller();
        draw_one_pixel(&app, &controller);
        press_accel(&app, "a");
        assert!(app.get_has_selection());
        press_key(&app, "\u{7f}");
        assert_eq!(painted_count(&controller), 0);
    }

    #[test]
    fn arrow_keys_nudge_the_selected_pixels() {
        let (app, controller) = new_app_with_controller();
        draw_one_pixel(&app, &controller); // doc (0,0)
        {
            let mut c = controller.borrow_mut();
            c.select_all().unwrap();
        }
        refresh(&app, &controller.borrow());

        press_key(&app, "\u{f703}"); // right
        press_key(&app, "\u{f701}"); // down
        let doc = controller.borrow().document().clone();
        assert_ne!(doc.get_pixel(1, 1).unwrap()[3], 0, "the pixel moved to (1,1)");
        assert_eq!(doc.get_pixel(0, 0).unwrap()[3], 0, "and left its old cell empty");
    }

    #[test]
    fn cut_copy_and_paste_round_trip_through_the_toolbar() {
        let (app, controller) = new_app_with_controller();
        {
            let mut c = controller.borrow_mut();
            c.select_palette(4); // red
            c.begin_stroke(4.0, 4.0).unwrap(); // doc (0,0)
            c.end_drag().unwrap();
            c.engine_select_rect(0, 0, 1, 1);
        }
        refresh(&app, &controller.borrow());
        let ink = controller.borrow().document().get_pixel(0, 0).unwrap();

        find_one(&app, "HomeToolbar::cut-btn").mock_single_click(PointerEventButton::Left);
        assert!(app.get_has_clipboard());
        assert_eq!(painted_count(&controller), 0, "cut removes the source pixels");

        // Paste somewhere else.
        {
            let mut c = controller.borrow_mut();
            c.paste_at(5, 5).unwrap();
        }
        refresh(&app, &controller.borrow());
        assert_eq!(
            controller.borrow().document().get_pixel(5, 5).unwrap(),
            ink,
            "paste reproduces the cut pixels exactly"
        );
    }

    #[test]
    fn copy_composite_sees_through_the_layer_stack() {
        let (app, controller) = new_app_with_controller();
        draw_one_pixel(&app, &controller);
        find_one(&app, "LayersPanel::layer-add-btn").mock_single_click(PointerEventButton::Left);

        // The active (new, empty) layer has nothing; an ordinary copy sees
        // nothing, a composite copy sees the layer underneath.
        find_one(&app, "HomeToolbar::copy-btn").mock_single_click(PointerEventButton::Left);
        let plain = controller.borrow().clipboard().unwrap().pixels.iter().any(|b| *b != 0);
        find_one(&app, "HomeToolbar::copy-composite-btn")
            .mock_single_click(PointerEventButton::Left);
        let composite = controller.borrow().clipboard().unwrap().pixels.iter().any(|b| *b != 0);

        assert!(!plain, "an ordinary copy reads only the active layer");
        assert!(composite, "a composite copy reads what is visible");
    }

    #[test]
    fn duplicate_and_merge_layers_from_the_panel() {
        let (app, controller) = new_app_with_controller();
        draw_one_pixel(&app, &controller);

        find_one(&app, "LayersPanel::layer-duplicate-btn")
            .mock_single_click(PointerEventButton::Left);
        assert_eq!(app.get_layers().row_count(), 2);

        let before = controller.borrow().document().composite();
        find_one(&app, "LayersPanel::layer-merge-btn").mock_single_click(PointerEventButton::Left);
        assert_eq!(app.get_layers().row_count(), 1);
        assert_eq!(
            controller.borrow().document().composite(),
            before,
            "merging must not change the image"
        );
    }

    #[test]
    fn flip_rotate_and_crop_are_reachable_from_the_toolbar() {
        let (app, controller) = new_app_with_controller();
        draw_one_pixel(&app, &controller);
        {
            let mut c = controller.borrow_mut();
            c.engine_select_rect(0, 0, 4, 4);
        }
        refresh(&app, &controller.borrow());

        find_one(&app, "HomeToolbar::flip-h-btn").mock_single_click(PointerEventButton::Left);
        assert_ne!(
            controller.borrow().document().get_pixel(3, 0).unwrap()[3],
            0,
            "flip moved the pixel across the selection box"
        );

        find_one(&app, "HomeToolbar::rotate-btn").mock_single_click(PointerEventButton::Left);
        find_one(&app, "HomeToolbar::crop-btn").mock_single_click(PointerEventButton::Left);
        let doc = controller.borrow().document().clone();
        assert_eq!((doc.width(), doc.height()), (4, 4), "crop resized the canvas");
    }

    #[test]
    fn the_grid_toggle_and_zoom_presets_change_view_state_only() {
        let (app, controller) = new_app_with_controller();
        draw_one_pixel(&app, &controller);
        let before = controller.borrow().document().clone();

        assert!(app.get_grid_on());
        find_one(&app, "LinuxChrome::grid-btn").mock_single_click(PointerEventButton::Left);
        assert!(!app.get_grid_on());
        assert!(!controller.borrow().should_draw_grid(), "the grid is off even at high zoom");

        find_one(&app, "LinuxChrome::zoom-actual-btn").mock_single_click(PointerEventButton::Left);
        assert_eq!(controller.borrow().zoom(), 1);
        assert_eq!(app.get_zoom_level(), 1);

        controller.borrow_mut().set_viewport(512, 512);
        find_one(&app, "LinuxChrome::zoom-fit-btn").mock_single_click(PointerEventButton::Left);
        assert_eq!(controller.borrow().viewport(), (512, 512));
        assert_eq!(controller.borrow().zoom(), 8, "64px canvas in a 512px viewport fits at 8x");

        assert_eq!(controller.borrow().document(), &before, "no view change touches the document");
        assert!(!app.get_can_redo());
    }

    #[test]
    fn every_new_tool_has_a_working_shortcut() {
        let (app, controller) = new_app_with_controller();
        for (index, tool) in Tool::ALL.iter().enumerate() {
            press_key(&app, &tool.shortcut().to_string());
            assert_eq!(controller.borrow().tool(), *tool, "key {:?}", tool.shortcut());
            assert_eq!(app.get_selected_tool(), index as i32);
        }
        assert_eq!(Tool::ALL.len(), 12, "all twelve tools are reachable");
    }

    #[test]
    fn the_recent_colour_strip_fills_as_colours_are_used() {
        let (app, controller) = new_app_with_controller();
        assert_eq!(app.get_recent_colors().row_count(), 0);

        app.invoke_palette_clicked(4);
        app.invoke_palette_clicked(6);
        assert_eq!(app.get_recent_colors().row_count(), 2);

        // Re-using a colour moves it to the front rather than duplicating.
        app.invoke_palette_clicked(4);
        assert_eq!(app.get_recent_colors().row_count(), 2);
        assert_eq!(controller.borrow().recent_colors()[0], controller.borrow().palette()[4]);
    }

    #[test]
    fn the_text_tool_stamps_the_typed_buffer() {
        let (app, controller) = new_app_with_controller();
        app.invoke_text_changed("HI".into());
        assert_eq!(controller.borrow().text_buffer(), "HI");
        press_key(&app, "t");
        assert_eq!(controller.borrow().tool(), Tool::Text);

        {
            let mut c = controller.borrow_mut();
            c.begin_stroke(80.0, 80.0).unwrap();
            c.end_drag().unwrap();
        }
        refresh(&app, &controller.borrow());
        assert!(painted_count(&controller) > 0, "text must put ink on the canvas");
    }

    #[test]
    fn a_lasso_drag_produces_a_non_rectangular_selection() {
        let (app, controller) = new_app_with_controller();
        press_key(&app, "f"); // lasso
        {
            let mut c = controller.borrow_mut();
            c.begin_stroke(0.0, 0.0).unwrap();
            c.continue_drag(80.0, 0.0).unwrap();
            c.continue_drag(0.0, 80.0).unwrap();
            c.end_drag().unwrap();
        }
        refresh(&app, &controller.borrow());
        let c = controller.borrow();
        let s = c.selection().expect("the lasso must have selected something");
        assert!(!s.is_rect(), "a traced triangle is not a rectangle");
        assert!(app.get_has_selection());
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



