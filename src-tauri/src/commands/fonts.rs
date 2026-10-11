//! Bundled portable fonts and CSS-variable mapping for UI (sans) and mono fonts.
//!
//! Fonts are shipped in `public/fonts` and must not be loaded from the OS.

use serde::{Deserialize, Serialize};

/// CSS custom property applied to the document for body/UI (sans) text.
pub const CSS_VAR_SANS: &str = "--app-font-sans";
/// CSS custom property applied to the document for `font-mono` / code / logs.
pub const CSS_VAR_MONO: &str = "--app-font-mono";
/// CSS custom property used by Claude-theme body text (honors the UI font).
pub const CSS_VAR_BODY: &str = "--font-body";
/// CSS custom property used by Claude-theme headings (honors the UI font).
pub const CSS_VAR_HEADING: &str = "--font-heading";

/// Tailwind v4 default sans stack — matches an unset UI font.
pub const DEFAULT_SANS_STACK: &str = r#"ui-sans-serif, system-ui, sans-serif, "Apple Color Emoji", "Segoe UI Emoji", "Segoe UI Symbol", "Noto Color Emoji""#;
/// Tailwind v4 default mono stack — matches an unset mono font.
pub const DEFAULT_MONO_STACK: &str = r#"ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace"#;
/// Claude-theme body fallback when the UI font is unset.
pub const DEFAULT_BODY_STACK: &str =
    r#""Segoe UI", system-ui, -apple-system, Roboto, Helvetica, Arial, sans-serif"#;
/// Heading fallback when the UI font is unset (bundled Inter, not a network font).
pub const DEFAULT_HEADING_STACK: &str = r#""Inter", "Segoe UI", system-ui, sans-serif"#;

/// A font family shipped with the app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundledFont {
    /// CSS `font-family` name matching `@font-face` in `bundled-fonts.css`.
    pub family: String,
    /// True for the five code fonts.
    pub is_monospace: bool,
}

/// Five current UI fonts followed by five current code fonts.
pub const BUNDLED_FONTS: &[(&str, bool)] = &[
    ("Inter", false),
    ("Geist", false),
    ("IBM Plex Sans", false),
    ("Plus Jakarta Sans", false),
    ("Manrope", false),
    ("JetBrains Mono", true),
    ("Fira Code", true),
    ("Geist Mono", true),
    ("Source Code Pro", true),
    ("Roboto Mono", true),
];

/// CSS values bound to the document for sans vs mono surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FontCssBinding {
    /// Value for [`CSS_VAR_SANS`] (`font-sans` / body UI).
    pub sans: String,
    /// Value for [`CSS_VAR_MONO`] (`font-mono` / code / terminal / logs).
    pub mono: String,
    /// Value for [`CSS_VAR_BODY`].
    pub body: String,
    /// Value for [`CSS_VAR_HEADING`].
    pub heading: String,
    /// Document CSS variable that receives `sans`.
    pub sans_variable: String,
    /// Document CSS variable that receives `mono`.
    pub mono_variable: String,
}

/// Quote a font family name for a CSS `font-family` list.
pub fn css_quoted_family(name: &str) -> String {
    let escaped = name.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn with_fallback(name: &str, fallback: &str) -> String {
    format!("{}, {}", css_quoted_family(name), fallback)
}

/// Resolve a persisted name to a bundled family, or `None` for default/unknown.
pub fn resolve_bundled_family(name: &str, monospace: bool) -> Option<&'static str> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return None;
    }
    BUNDLED_FONTS
        .iter()
        .find(|(family, is_mono)| *is_mono == monospace && family.eq_ignore_ascii_case(trimmed))
        .map(|(family, _)| *family)
}

/// Map persisted UI/mono font names to the CSS values the document applies.
///
/// Empty, whitespace, or unknown names keep the default stacks.
pub fn font_css_binding(ui_font: &str, mono_font: &str) -> FontCssBinding {
    let (sans, body, heading) = match resolve_bundled_family(ui_font, false) {
        Some(family) => {
            let stack = with_fallback(family, DEFAULT_SANS_STACK);
            (stack.clone(), stack.clone(), stack)
        }
        None => (
            DEFAULT_SANS_STACK.to_string(),
            DEFAULT_BODY_STACK.to_string(),
            DEFAULT_HEADING_STACK.to_string(),
        ),
    };

    let mono_stack = match resolve_bundled_family(mono_font, true) {
        Some(family) => with_fallback(family, DEFAULT_MONO_STACK),
        None => DEFAULT_MONO_STACK.to_string(),
    };

    FontCssBinding {
        sans,
        mono: mono_stack,
        body,
        heading,
        sans_variable: CSS_VAR_SANS.to_string(),
        mono_variable: CSS_VAR_MONO.to_string(),
    }
}

/// Canonicalize a stored font to a bundled family, or empty for default.
pub fn canonicalize_stored_font(name: &str, monospace: bool) -> String {
    resolve_bundled_family(name, monospace)
        .unwrap_or("")
        .to_string()
}

/// Bundled font catalog (portable; does not enumerate the OS).
pub fn list_bundled_fonts() -> Vec<BundledFont> {
    BUNDLED_FONTS
        .iter()
        .map(|(family, is_monospace)| BundledFont {
            family: (*family).to_string(),
            is_monospace: *is_monospace,
        })
        .collect()
}

/// Tauri command: bundled font catalog.
#[tauri::command]
pub fn get_bundled_fonts() -> Vec<BundledFont> {
    list_bundled_fonts()
}

/// Back-compat alias used by older frontends.
#[tauri::command]
pub fn get_system_fonts() -> Vec<BundledFont> {
    list_bundled_fonts()
}

/// Tauri command: CSS binding for a pair of persisted font names.
#[tauri::command]
pub fn get_font_css_binding(ui_font: String, mono_font: String) -> FontCssBinding {
    font_css_binding(&ui_font, &mono_font)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_catalog_should_contain_five_ui_and_five_mono_fonts() {
        let fonts = list_bundled_fonts();
        assert_eq!(fonts.len(), 10);
        assert_eq!(fonts.iter().filter(|f| !f.is_monospace).count(), 5);
        assert_eq!(fonts.iter().filter(|f| f.is_monospace).count(), 5);
        let names: Vec<&str> = fonts.iter().map(|f| f.family.as_str()).collect();
        assert!(names.contains(&"Inter"));
        assert!(names.contains(&"Geist"));
        assert!(names.contains(&"IBM Plex Sans"));
        assert!(names.contains(&"Plus Jakarta Sans"));
        assert!(names.contains(&"Manrope"));
        assert!(names.contains(&"JetBrains Mono"));
        assert!(names.contains(&"Fira Code"));
        assert!(names.contains(&"Geist Mono"));
        assert!(names.contains(&"Source Code Pro"));
        assert!(names.contains(&"Roboto Mono"));
    }

    #[test]
    fn font_css_binding_should_use_default_stacks_when_unset() {
        let binding = font_css_binding("", "");
        assert_eq!(binding.sans, DEFAULT_SANS_STACK);
        assert_eq!(binding.mono, DEFAULT_MONO_STACK);
        assert_eq!(binding.body, DEFAULT_BODY_STACK);
        assert_eq!(binding.heading, DEFAULT_HEADING_STACK);
        assert_eq!(binding.sans_variable, CSS_VAR_SANS);
        assert_eq!(binding.mono_variable, CSS_VAR_MONO);
        assert_ne!(binding.sans, binding.mono);
    }

    #[test]
    fn font_css_binding_should_map_bundled_fonts_to_distinct_sans_and_mono_stacks() {
        let binding = font_css_binding("Inter", "JetBrains Mono");
        assert!(
            binding.sans.starts_with("\"Inter\""),
            "sans binding: {}",
            binding.sans
        );
        assert!(
            binding.mono.starts_with("\"JetBrains Mono\""),
            "mono binding: {}",
            binding.mono
        );
        assert!(binding.sans.contains(DEFAULT_SANS_STACK));
        assert!(binding.mono.contains(DEFAULT_MONO_STACK));
        assert_ne!(binding.sans, binding.mono);
        assert_eq!(binding.body, binding.sans);
        assert_eq!(binding.heading, binding.sans);
    }

    #[test]
    fn font_css_binding_should_ignore_unknown_and_os_font_names() {
        let binding = font_css_binding("Comic Sans MS", "Consolas");
        assert_eq!(binding.sans, DEFAULT_SANS_STACK);
        assert_eq!(binding.mono, DEFAULT_MONO_STACK);
    }

    #[test]
    fn font_css_binding_should_treat_whitespace_as_unset() {
        let binding = font_css_binding("   ", "\t");
        assert_eq!(binding.sans, DEFAULT_SANS_STACK);
        assert_eq!(binding.mono, DEFAULT_MONO_STACK);
    }

    #[test]
    fn resolve_bundled_family_should_be_case_insensitive() {
        assert_eq!(resolve_bundled_family("inter", false), Some("Inter"));
        assert_eq!(
            resolve_bundled_family("JETBRAINS MONO", true),
            Some("JetBrains Mono")
        );
        assert_eq!(resolve_bundled_family("Inter", true), None);
        assert_eq!(resolve_bundled_family("JetBrains Mono", false), None);
    }

    #[test]
    fn bundled_font_files_should_exist_in_public_fonts() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../public/fonts");
        let files = [
            "inter-latin-wght-normal.woff2",
            "geist-latin-wght-normal.woff2",
            "ibm-plex-sans-latin-wght-normal.woff2",
            "plus-jakarta-sans-latin-wght-normal.woff2",
            "manrope-latin-wght-normal.woff2",
            "jetbrains-mono-latin-wght-normal.woff2",
            "fira-code-latin-wght-normal.woff2",
            "geist-mono-latin-wght-normal.woff2",
            "source-code-pro-latin-wght-normal.woff2",
            "roboto-mono-latin-wght-normal.woff2",
        ];
        for file in files {
            let path = dir.join(file);
            assert!(path.is_file(), "missing bundled font file {}", path.display());
        }
    }

    #[test]
    fn get_font_css_binding_command_should_delegate_to_mapping() {
        let via_command =
            get_font_css_binding("Manrope".to_string(), "Fira Code".to_string());
        let via_fn = font_css_binding("Manrope", "Fira Code");
        assert_eq!(via_command, via_fn);
    }
}
