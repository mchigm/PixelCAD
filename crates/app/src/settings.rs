//! Persisted application settings and platform detection.
//!
//! This module is deliberately window-free: it has no Slint dependency and
//! every function here is testable headlessly, matching `pixelcad-core`'s
//! philosophy even though it lives in the shell crate.
//!
//! The file I/O is split from the path resolution ([`load_from`] /
//! [`save_to`] take an explicit path) so tests never touch the real user's
//! configuration directory.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Which chrome tree the shell renders.
///
/// `Auto` is a *stored* value, not a resolved one: the user asked to follow
/// the OS, so it must survive a restart on a different machine and resolve
/// again there. Call [`UiStyle::resolve`] to get something renderable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UiStyle {
    Auto,
    Linux,
    MacOs,
}

impl UiStyle {
    /// Collapses `Auto` into a concrete platform using the host OS.
    pub fn resolve(self) -> Platform {
        match self {
            UiStyle::Auto => detect_platform().platform,
            UiStyle::Linux => Platform::Linux,
            UiStyle::MacOs => Platform::MacOs,
        }
    }

    /// The string the `.slint` side switches on.
    pub fn as_ui_string(self) -> &'static str {
        match self.resolve() {
            Platform::Linux => "linux",
            Platform::MacOs => "macos",
        }
    }
}

/// A chrome tree that actually exists. There is no `Windows` variant by
/// design: per the maintainer's spec, Windows users run the Linux build
/// under WSL, so Windows resolves to [`Platform::Linux`] and is reported
/// through [`PlatformDetection::windows_fallback`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Linux,
    MacOs,
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Platform::Linux => write!(f, "Linux"),
            Platform::MacOs => write!(f, "macOS"),
        }
    }
}

/// The result of inspecting the host OS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformDetection {
    pub platform: Platform,
    /// True when the host is Windows and we fell back to the Linux chrome.
    /// The first-run dialog uses this to recommend the WSL build once.
    pub windows_fallback: bool,
}

/// Inspects the host OS. Windows falls back to the Linux chrome and is
/// flagged; anything else unknown (BSD, etc.) also takes the Linux chrome,
/// which is the correct default for an X11/Wayland desktop.
pub fn detect_platform() -> PlatformDetection {
    if cfg!(target_os = "macos") {
        PlatformDetection { platform: Platform::MacOs, windows_fallback: false }
    } else if cfg!(target_os = "windows") {
        PlatformDetection { platform: Platform::Linux, windows_fallback: true }
    } else {
        PlatformDetection { platform: Platform::Linux, windows_fallback: false }
    }
}

/// How light the chrome is, and how hard the contrast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Theme {
    Dark,
    Light,
    HighContrast,
}

impl Theme {
    /// The string the `.slint` `Theme.scheme` property switches on.
    pub fn as_ui_string(self) -> &'static str {
        match self {
            Theme::Dark => "dark",
            Theme::Light => "light",
            Theme::HighContrast => "high-contrast",
        }
    }
}

/// The hue family of the chrome. Orthogonal to [`Theme`]: the scheme
/// decides how light the surfaces are, the design decides what colour they
/// and the accent are, so four designs cover twelve appearances without
/// twelve hand-maintained themes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ColorDesign {
    /// Blueprint cyan on cold near-black. The product's default identity.
    Cyanotype,
    /// Neutral pencil greys with a lead-blue accent.
    Graphite,
    /// Drafting-lamp amber on warm near-black.
    Amber,
    /// Green CRT, a nod to the command-line heritage.
    Phosphor,
}

impl ColorDesign {
    pub fn as_ui_string(self) -> &'static str {
        match self {
            ColorDesign::Cyanotype => "cyanotype",
            ColorDesign::Graphite => "graphite",
            ColorDesign::Amber => "amber",
            ColorDesign::Phosphor => "phosphor",
        }
    }
}

/// The type scale. Stored as a named step rather than a raw number so the
/// setting stays meaningful and cannot be set to something unreadable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TextStyle {
    Compact,
    Normal,
    Comfortable,
}

impl TextStyle {
    /// Multiplier applied to every text size and to the toolbar row
    /// heights, so larger text does not clip.
    pub fn scale(self) -> f32 {
        match self {
            TextStyle::Compact => 0.92,
            TextStyle::Normal => 1.0,
            TextStyle::Comfortable => 1.15,
        }
    }
}

/// Everything the Settings page can change, plus the first-run marker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct AppSettings {
    // --- Style tab ---
    pub ui_style: UiStyle,
    /// Colour scheme: dark / light / high contrast.
    pub theme: Theme,
    /// Colour design: which hue family the chrome uses.
    pub color_design: ColorDesign,
    /// Text style: the type scale.
    pub text_style: TextStyle,
    /// Use monospace for UI labels too, not just numeric readouts.
    pub mono_labels: bool,
    /// Hex, `#rrggbb`, overriding the design's own accent. Empty means
    /// "use the design's accent". Validated on load; an unparseable value
    /// falls back to empty rather than rejecting the whole file.
    pub accent_color: String,
    pub icon_size: u32,

    // --- Basic tab ---
    pub default_canvas_width: u32,
    pub default_canvas_height: u32,
    pub autosave_enabled: bool,
    pub autosave_interval_secs: u32,
    pub confirm_before_exit: bool,
    pub recent_files_len: u32,

    /// macOS can add/remove chrome items but not reposition them (per spec).
    /// Empty means "show the default set".
    pub macos_hidden_items: Vec<String>,

    /// Set once the platform-detection dialog has been answered.
    pub first_run_completed: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            ui_style: UiStyle::Auto,
            theme: Theme::Dark,
            color_design: ColorDesign::Cyanotype,
            text_style: TextStyle::Normal,
            mono_labels: false,
            // Empty by default so the chosen colour design supplies the
            // accent; a non-empty value is an explicit user override.
            accent_color: String::new(),
            icon_size: 16,
            default_canvas_width: 64,
            default_canvas_height: 64,
            autosave_enabled: false,
            autosave_interval_secs: 300,
            confirm_before_exit: true,
            recent_files_len: 10,
            macos_hidden_items: Vec::new(),
            first_run_completed: false,
        }
    }
}

impl AppSettings {
    /// Clamps every numeric field into a sane range and repairs an invalid
    /// accent colour.
    ///
    /// A hand-edited or version-skewed config must never be able to put the
    /// shell into an unusable state (a 0-px icon, a 1x1 default canvas), so
    /// repair-on-load is preferred to rejecting the file.
    pub fn sanitize(&mut self) {
        self.icon_size = self.icon_size.clamp(14, 20);
        self.default_canvas_width = self.default_canvas_width.clamp(1, 8192);
        self.default_canvas_height = self.default_canvas_height.clamp(1, 8192);
        self.autosave_interval_secs = self.autosave_interval_secs.clamp(10, 86_400);
        self.recent_files_len = self.recent_files_len.clamp(0, 50);
        // An empty accent is legitimate: it means "follow the design".
        if !self.accent_color.is_empty() && !is_valid_hex_color(&self.accent_color) {
            self.accent_color = String::new();
        }
    }
}

fn is_valid_hex_color(s: &str) -> bool {
    let Some(body) = s.strip_prefix('#') else { return false };
    body.len() == 6 && body.chars().all(|c| c.is_ascii_hexdigit())
}

/// What happened when settings were read. The shell surfaces `Corrupt` in
/// the status line instead of silently discarding a user's configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadOutcome {
    Loaded,
    Missing,
    Corrupt(String),
}

/// Reads settings from an explicit path, never failing: a missing or
/// unparseable file yields defaults plus the reason.
pub fn load_from(path: &Path) -> (AppSettings, LoadOutcome) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return (AppSettings::default(), LoadOutcome::Missing);
    };
    match toml::from_str::<AppSettings>(&text) {
        Ok(mut settings) => {
            settings.sanitize();
            (settings, LoadOutcome::Loaded)
        }
        Err(e) => (AppSettings::default(), LoadOutcome::Corrupt(e.to_string())),
    }
}

/// Writes settings to an explicit path, creating parent directories.
pub fn save_to(path: &Path, settings: &AppSettings) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = toml::to_string_pretty(settings)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(path, text)
}

/// The per-OS configuration file location.
///
/// `directories` is used rather than a hard-coded `~/.config` because that
/// path is simply wrong on macOS, which is one of this phase's two target
/// platforms.
pub fn config_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("dev", "PixelCAD", "PixelCAD")
        .map(|dirs| dirs.config_dir().join("settings.toml"))
}

/// Convenience wrapper around [`load_from`] using [`config_path`].
pub fn load() -> (AppSettings, LoadOutcome) {
    match config_path() {
        Some(path) => load_from(&path),
        None => (AppSettings::default(), LoadOutcome::Missing),
    }
}

/// Convenience wrapper around [`save_to`] using [`config_path`].
pub fn save(settings: &AppSettings) -> std::io::Result<()> {
    let path = config_path().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no writable configuration directory for this user",
        )
    })?;
    save_to(&path, settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        (dir, path)
    }

    #[test]
    fn a_missing_file_yields_defaults_and_says_so() {
        let (_dir, path) = temp_path("absent.toml");
        let (settings, outcome) = load_from(&path);
        assert_eq!(outcome, LoadOutcome::Missing);
        assert_eq!(settings, AppSettings::default());
        assert!(
            !settings.first_run_completed,
            "a fresh install must still be able to trigger the first-run dialog"
        );
    }

    #[test]
    fn settings_round_trip_through_toml() {
        let (_dir, path) = temp_path("settings.toml");
        let mut written = AppSettings::default();
        written.ui_style = UiStyle::MacOs;
        written.theme = Theme::Light;
        written.accent_color = "#AABBCC".to_string();
        written.icon_size = 20;
        written.default_canvas_width = 320;
        written.first_run_completed = true;
        written.macos_hidden_items = vec!["media-bay".into()];

        save_to(&path, &written).unwrap();
        let (read_back, outcome) = load_from(&path);

        assert_eq!(outcome, LoadOutcome::Loaded);
        assert_eq!(read_back, written, "every field must survive the round trip");
    }

    #[test]
    fn save_creates_missing_parent_directories() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/deeper/settings.toml");
        save_to(&path, &AppSettings::default()).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn a_corrupt_file_falls_back_to_defaults_without_losing_the_reason() {
        let (_dir, path) = temp_path("broken.toml");
        std::fs::write(&path, "this is not valid toml {{{").unwrap();
        let (settings, outcome) = load_from(&path);
        assert_eq!(settings, AppSettings::default());
        match outcome {
            LoadOutcome::Corrupt(msg) => assert!(!msg.is_empty()),
            other => panic!("expected Corrupt, got {other:?}"),
        }
    }

    #[test]
    fn a_partial_file_keeps_defaults_for_absent_fields() {
        // `#[serde(default)]` is what makes a settings file written by an
        // older version still load after new fields are added.
        let (_dir, path) = temp_path("partial.toml");
        std::fs::write(&path, "theme = \"light\"\n").unwrap();
        let (settings, outcome) = load_from(&path);
        assert_eq!(outcome, LoadOutcome::Loaded);
        assert_eq!(settings.theme, Theme::Light);
        assert_eq!(settings.icon_size, AppSettings::default().icon_size);
        assert_eq!(settings.ui_style, AppSettings::default().ui_style);
    }

    #[test]
    fn out_of_range_values_are_repaired_rather_than_rejected() {
        let (_dir, path) = temp_path("silly.toml");
        std::fs::write(
            &path,
            "icon-size = 9000\ndefault-canvas-width = 0\nrecent-files-len = 999\naccent-color = \"not-a-colour\"\n",
        )
        .unwrap();
        let (settings, outcome) = load_from(&path);
        assert_eq!(outcome, LoadOutcome::Loaded);
        assert_eq!(settings.icon_size, 20);
        assert_eq!(settings.default_canvas_width, 1);
        assert_eq!(settings.recent_files_len, 50);
        assert_eq!(settings.accent_color, AppSettings::default().accent_color);
    }

    #[test]
    fn detect_platform_agrees_with_the_compile_target() {
        let detected = detect_platform();
        if cfg!(target_os = "macos") {
            assert_eq!(detected.platform, Platform::MacOs);
            assert!(!detected.windows_fallback);
        } else if cfg!(target_os = "windows") {
            // Windows has no chrome of its own by design; it borrows Linux's
            // and is flagged so the first-run dialog can recommend WSL.
            assert_eq!(detected.platform, Platform::Linux);
            assert!(detected.windows_fallback);
        } else {
            assert_eq!(detected.platform, Platform::Linux);
            assert!(!detected.windows_fallback);
        }
    }

    #[test]
    fn auto_resolves_to_the_host_while_explicit_choices_are_honoured() {
        assert_eq!(UiStyle::Auto.resolve(), detect_platform().platform);
        assert_eq!(UiStyle::Linux.resolve(), Platform::Linux);
        assert_eq!(UiStyle::MacOs.resolve(), Platform::MacOs);
        assert_eq!(UiStyle::Linux.as_ui_string(), "linux");
        assert_eq!(UiStyle::MacOs.as_ui_string(), "macos");
    }

    #[test]
    fn an_explicit_choice_survives_being_stored_as_auto_elsewhere() {
        // Regression guard for the temptation to store the *resolved*
        // platform instead of the user's actual choice: "Auto" must stay
        // "Auto" on disk so it re-resolves on a different machine.
        let (_dir, path) = temp_path("auto.toml");
        let settings = AppSettings { ui_style: UiStyle::Auto, ..Default::default() };
        save_to(&path, &settings).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("ui-style = \"auto\""), "got: {text}");
    }

    #[test]
    fn hex_validation_accepts_only_six_digit_hashes() {
        assert!(is_valid_hex_color("#3FA7D6"));
        assert!(is_valid_hex_color("#000000"));
        assert!(!is_valid_hex_color("3FA7D6"));
        assert!(!is_valid_hex_color("#3FA7D"));
        assert!(!is_valid_hex_color("#3FA7D6F"));
        assert!(!is_valid_hex_color("#GGGGGG"));
    }
}
