//! Colour schemes (Options ▸ Color ▸ Theme), fonts and the global egui style.
//!
//! The four schemes are the original's window-painter schemes (UI-SPEC §10, lead decision §13.5):
//! **Light**, **Medium**, **Dark** (default, every value measured in `docs/design-tokens.md`) and **Black**.
//! Code never uses colour literals: it reads the tokens of the active scheme with [`t()`].

use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, Visuals};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU8, Ordering};

/// Options ▸ Color ▸ Theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Scheme {
    Light,
    Medium,
    #[default]
    Dark,
    Black,
}

impl Scheme {
    pub const ALL: [Scheme; 4] = [Scheme::Light, Scheme::Medium, Scheme::Dark, Scheme::Black];
    /// Caption in the Theme combo (a `docs/strings-en.tsv` key).
    pub fn caption(self) -> &'static str {
        match self {
            Scheme::Light => "Light",
            Scheme::Medium => "Medium",
            Scheme::Dark => "Dark",
            Scheme::Black => "Black",
        }
    }
    pub fn tokens(self) -> &'static Tokens {
        match self {
            Scheme::Light => &LIGHT,
            Scheme::Medium => &MEDIUM,
            Scheme::Dark => &DARK,
            Scheme::Black => &BLACK,
        }
    }
    /// Dark when Windows apps use the dark mode (`AppsUseLightTheme` = 0), Light otherwise (not the default:
    /// new settings start Dark, lead decision 5).
    pub fn system_default() -> Scheme {
        #[cfg(windows)]
        {
            if crate::platform::apps_use_light_theme() == Some(true) {
                return Scheme::Light;
            }
        }
        Scheme::Dark
    }
}

/// The named design tokens (`docs/design-tokens.md`, UI-SPEC §10).
#[derive(Debug)]
pub struct Tokens {
    pub light: bool,
    pub title_bar_bg: Color32,
    pub title_text: Color32,
    pub title_text_inactive: Color32,
    pub title_hot: Color32,
    pub close_hot: Color32,
    pub menu_bar_bg: Color32,
    pub menu_bar_hot: Color32,
    pub toolbar_bg: Color32,
    pub line_strong: Color32,
    pub line_divider: Color32,
    pub panel_bg: Color32,
    pub list_bg: Color32,
    pub library_selected: Color32,
    pub list_selected: Color32,
    pub row_checked: Color32,
    pub row_hot: Color32,
    pub installed_bar: Color32,
    pub scroll_track: Color32,
    pub scroll_thumb: Color32,
    pub text: Color32,
    pub text_strong: Color32,
    pub text_disabled: Color32,
    pub text_placeholder: Color32,
    pub text_secondary: Color32,
    pub accent: Color32,
    pub accent_checked: Color32,
    pub chevron: Color32,
    pub menu_bg: Color32,
    pub menu_border: Color32,
    pub menu_separator: Color32,
    pub menu_hot: Color32,
    pub submenu_arrow: Color32,
    pub edit_bg: Color32,
    pub edit_border: Color32,
    pub combo_bg: Color32,
    pub button_bg: Color32,
    pub button_border: Color32,
    pub button_hot: Color32,
    pub button_pressed: Color32,
    pub checkbox_bg: Color32,
    pub checkbox_border: Color32,
    pub slider_track: Color32,
    pub icon: Color32,
    pub icon_disabled: Color32,
    pub icon_badge_disabled: Color32,
    pub icon_title_bar: Color32,
    pub icon_window_buttons: Color32,
    pub badge_remove: Color32,
    pub dialog_bg: Color32,
    pub dialog_separator: Color32,
    pub window_border: Color32,
    pub hyperlink: Color32,
    pub glyph_metric: Color32,
    pub glyph_baseline: Color32,
    pub tooltip_bg: Color32,
}

const fn rgb(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// Dark: every value measured in the screenshots (`docs/design-tokens.md`).
pub static DARK: Tokens = Tokens {
    light: false,
    title_bar_bg: rgb(0x262626),
    title_text: rgb(0xFFFFFF),
    title_text_inactive: rgb(0x969696),
    title_hot: rgb(0x3C3C3C),
    close_hot: rgb(0xC42B1C),
    menu_bar_bg: rgb(0x404040),
    menu_bar_hot: rgb(0x525252),
    toolbar_bg: rgb(0x262626),
    line_strong: rgb(0x525252),
    line_divider: rgb(0x3C3C3C),
    panel_bg: rgb(0x262626),
    list_bg: rgb(0x262626),
    library_selected: rgb(0x3C3C3C),
    list_selected: rgb(0x525252),
    row_checked: rgb(0x525252),
    row_hot: rgb(0x333333),
    installed_bar: rgb(0x66BF83),
    scroll_track: rgb(0x171717),
    scroll_thumb: rgb(0x959595),
    text: rgb(0xCACED4),
    text_strong: rgb(0xFFFFFF),
    text_disabled: rgb(0x808080),
    text_placeholder: rgb(0x575757),
    text_secondary: rgb(0xCACED4),
    accent: rgb(0x017DBC),
    accent_checked: rgb(0x1D4470),
    chevron: rgb(0x1D4470),
    menu_bg: rgb(0x262626),
    menu_border: rgb(0x525252),
    menu_separator: rgb(0x525252),
    menu_hot: rgb(0x525252),
    submenu_arrow: rgb(0x969696),
    edit_bg: rgb(0x262626),
    edit_border: rgb(0x525252),
    combo_bg: rgb(0x3C3C3C),
    button_bg: rgb(0x3C3C3C),
    button_border: rgb(0x525252),
    button_hot: rgb(0x525252),
    button_pressed: rgb(0x1D4470),
    checkbox_bg: rgb(0x404040),
    checkbox_border: rgb(0x525252),
    slider_track: rgb(0x525252),
    icon: rgb(0xCACED4),
    icon_disabled: rgb(0x5D5D5D),
    icon_badge_disabled: rgb(0x434343),
    icon_title_bar: rgb(0x5D5D5D),
    icon_window_buttons: rgb(0x969696),
    badge_remove: rgb(0xFF6250),
    dialog_bg: rgb(0x262626),
    dialog_separator: rgb(0x525252),
    window_border: rgb(0x000000),
    hyperlink: rgb(0x3FA9E8),
    glyph_metric: rgb(0x95CAFF),
    glyph_baseline: rgb(0x00DC32),
    tooltip_bg: rgb(0x262626),
};

/// Black: darker panels (#1A1A1A) and list (#111111), lead decision §13.5.
pub static BLACK: Tokens = Tokens {
    light: false,
    title_bar_bg: rgb(0x111111),
    title_text: rgb(0xFFFFFF),
    title_text_inactive: rgb(0x8C8C8C),
    title_hot: rgb(0x2E2E2E),
    close_hot: rgb(0xC42B1C),
    menu_bar_bg: rgb(0x2A2A2A),
    menu_bar_hot: rgb(0x3E3E3E),
    toolbar_bg: rgb(0x1A1A1A),
    line_strong: rgb(0x3E3E3E),
    line_divider: rgb(0x2A2A2A),
    panel_bg: rgb(0x1A1A1A),
    list_bg: rgb(0x111111),
    library_selected: rgb(0x2E2E2E),
    list_selected: rgb(0x3E3E3E),
    row_checked: rgb(0x3E3E3E),
    row_hot: rgb(0x222222),
    installed_bar: rgb(0x66BF83),
    scroll_track: rgb(0x0A0A0A),
    scroll_thumb: rgb(0x8A8A8A),
    text: rgb(0xCACED4),
    text_strong: rgb(0xFFFFFF),
    text_disabled: rgb(0x787878),
    text_placeholder: rgb(0x4E4E4E),
    text_secondary: rgb(0xCACED4),
    accent: rgb(0x017DBC),
    accent_checked: rgb(0x1D4470),
    chevron: rgb(0x2A5A90),
    menu_bg: rgb(0x1A1A1A),
    menu_border: rgb(0x3E3E3E),
    menu_separator: rgb(0x3E3E3E),
    menu_hot: rgb(0x3E3E3E),
    submenu_arrow: rgb(0x8C8C8C),
    edit_bg: rgb(0x111111),
    edit_border: rgb(0x3E3E3E),
    combo_bg: rgb(0x2A2A2A),
    button_bg: rgb(0x2A2A2A),
    button_border: rgb(0x3E3E3E),
    button_hot: rgb(0x3E3E3E),
    button_pressed: rgb(0x1D4470),
    checkbox_bg: rgb(0x2E2E2E),
    checkbox_border: rgb(0x4A4A4A),
    slider_track: rgb(0x3E3E3E),
    icon: rgb(0xCACED4),
    icon_disabled: rgb(0x4E4E4E),
    icon_badge_disabled: rgb(0x363636),
    icon_title_bar: rgb(0x4E4E4E),
    icon_window_buttons: rgb(0x8C8C8C),
    badge_remove: rgb(0xFF6250),
    dialog_bg: rgb(0x1A1A1A),
    dialog_separator: rgb(0x3E3E3E),
    window_border: rgb(0x000000),
    hyperlink: rgb(0x3FA9E8),
    glyph_metric: rgb(0x95CAFF),
    glyph_baseline: rgb(0x00DC32),
    tooltip_bg: rgb(0x1A1A1A),
};

/// Medium: the #535353 family (lead decision §13.5).
pub static MEDIUM: Tokens = Tokens {
    light: false,
    title_bar_bg: rgb(0x4A4A4A),
    title_text: rgb(0xFFFFFF),
    title_text_inactive: rgb(0xB4B4B4),
    title_hot: rgb(0x5E5E5E),
    close_hot: rgb(0xC42B1C),
    menu_bar_bg: rgb(0x636363),
    menu_bar_hot: rgb(0x747474),
    toolbar_bg: rgb(0x535353),
    line_strong: rgb(0x747474),
    line_divider: rgb(0x5E5E5E),
    panel_bg: rgb(0x535353),
    list_bg: rgb(0x535353),
    library_selected: rgb(0x636363),
    list_selected: rgb(0x747474),
    row_checked: rgb(0x747474),
    row_hot: rgb(0x5B5B5B),
    installed_bar: rgb(0x66BF83),
    scroll_track: rgb(0x3E3E3E),
    scroll_thumb: rgb(0xB5B5B5),
    text: rgb(0xEDEDED),
    text_strong: rgb(0xFFFFFF),
    text_disabled: rgb(0xA0A0A0),
    text_placeholder: rgb(0x8C8C8C),
    text_secondary: rgb(0xEDEDED),
    accent: rgb(0x2B9BE0),
    accent_checked: rgb(0x2A5A90),
    chevron: rgb(0x2B9BE0),
    menu_bg: rgb(0x535353),
    menu_border: rgb(0x747474),
    menu_separator: rgb(0x747474),
    menu_hot: rgb(0x747474),
    submenu_arrow: rgb(0xBEBEBE),
    edit_bg: rgb(0x535353),
    edit_border: rgb(0x7A7A7A),
    combo_bg: rgb(0x636363),
    button_bg: rgb(0x636363),
    button_border: rgb(0x7A7A7A),
    button_hot: rgb(0x747474),
    button_pressed: rgb(0x2A5A90),
    checkbox_bg: rgb(0x636363),
    checkbox_border: rgb(0x7E7E7E),
    slider_track: rgb(0x7A7A7A),
    icon: rgb(0xEDEDED),
    icon_disabled: rgb(0x828282),
    icon_badge_disabled: rgb(0x6A6A6A),
    icon_title_bar: rgb(0x9A9A9A),
    icon_window_buttons: rgb(0xC8C8C8),
    badge_remove: rgb(0xFF6250),
    dialog_bg: rgb(0x535353),
    dialog_separator: rgb(0x747474),
    window_border: rgb(0x2A2A2A),
    hyperlink: rgb(0x8CCBFF),
    glyph_metric: rgb(0x95CAFF),
    glyph_baseline: rgb(0x00DC32),
    tooltip_bg: rgb(0x535353),
};

/// Light: the #F0F0F0 family with dark text (lead decision §13.5); icons tinted dark (icons README).
pub static LIGHT: Tokens = Tokens {
    light: true,
    title_bar_bg: rgb(0xE6E6E6),
    title_text: rgb(0x1E1E1E),
    title_text_inactive: rgb(0x8A8A8A),
    title_hot: rgb(0xD2D2D2),
    close_hot: rgb(0xC42B1C),
    menu_bar_bg: rgb(0xDADADA),
    menu_bar_hot: rgb(0xC4C4C4),
    toolbar_bg: rgb(0xF0F0F0),
    line_strong: rgb(0xC4C4C4),
    line_divider: rgb(0xD4D4D4),
    panel_bg: rgb(0xF0F0F0),
    list_bg: rgb(0xFAFAFA),
    library_selected: rgb(0xD9D9D9),
    list_selected: rgb(0xD0D0D0),
    row_checked: rgb(0xD0D0D0),
    row_hot: rgb(0xE8E8E8),
    installed_bar: rgb(0x66BF83),
    scroll_track: rgb(0xE2E2E2),
    scroll_thumb: rgb(0x8A8A8A),
    text: rgb(0x1E1E1E),
    text_strong: rgb(0x000000),
    text_disabled: rgb(0xA0A0A0),
    text_placeholder: rgb(0x9A9A9A),
    text_secondary: rgb(0x1E1E1E),
    accent: rgb(0x017DBC),
    accent_checked: rgb(0xC7DDF2),
    chevron: rgb(0x017DBC),
    menu_bg: rgb(0xF9F9F9),
    menu_border: rgb(0xC4C4C4),
    menu_separator: rgb(0xD7D7D7),
    menu_hot: rgb(0xDADADA),
    submenu_arrow: rgb(0x6E6E6E),
    edit_bg: rgb(0xFFFFFF),
    edit_border: rgb(0xADADAD),
    combo_bg: rgb(0xFFFFFF),
    button_bg: rgb(0xE1E1E1),
    button_border: rgb(0xADADAD),
    button_hot: rgb(0xD0D0D0),
    button_pressed: rgb(0xC7DDF2),
    checkbox_bg: rgb(0xFFFFFF),
    checkbox_border: rgb(0x8A8A8A),
    slider_track: rgb(0xC4C4C4),
    icon: rgb(0x3A3A3A),
    icon_disabled: rgb(0xB4B4B4),
    icon_badge_disabled: rgb(0xC8C8C8),
    icon_title_bar: rgb(0x8A8A8A),
    icon_window_buttons: rgb(0x5A5A5A),
    badge_remove: rgb(0xE5483A),
    dialog_bg: rgb(0xF0F0F0),
    dialog_separator: rgb(0xC4C4C4),
    window_border: rgb(0xAAAAAA),
    hyperlink: rgb(0x0066B4),
    glyph_metric: rgb(0x4A8FE0),
    glyph_baseline: rgb(0x00A028),
    tooltip_bg: rgb(0xFFFFFF),
};

static SCHEME: AtomicU8 = AtomicU8::new(2);

/// The active scheme.
pub fn scheme() -> Scheme {
    Scheme::ALL[SCHEME.load(Ordering::Relaxed) as usize % 4]
}

/// The tokens of the active scheme.
pub fn t() -> &'static Tokens {
    scheme().tokens()
}

/// The UI font size: Segoe UI 9 pt = 12 px (UI-SPEC conventions).
pub const UI_SIZE: f32 = 12.0;

/// Activates a scheme and rebuilds the egui visuals.
pub fn set_scheme(ctx: &egui::Context, s: Scheme) {
    SCHEME.store(Scheme::ALL.iter().position(|x| *x == s).unwrap_or(2) as u8, Ordering::Relaxed);
    apply_visuals(ctx);
}

/// Installs fonts, spacing and the visuals of the active scheme.
pub fn apply(ctx: &egui::Context) {
    install_fonts(ctx);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(6.0, 4.0);
        s.spacing.button_padding = egui::vec2(8.0, 3.0);
        s.spacing.interact_size = egui::vec2(24.0, 22.0);
        s.spacing.menu_margin = egui::Margin::same(2);
        s.spacing.window_margin = egui::Margin::same(0);
        s.spacing.icon_width = 12.0;
        s.spacing.icon_spacing = 5.0;
        s.spacing.combo_height = 400.0;
        s.spacing.scroll = egui::style::ScrollStyle::thin();
        s.spacing.scroll.fade.strength = 0.0;
        s.interaction.tooltip_delay = 0.5;
        s.interaction.selectable_labels = false;
        s.text_styles = [
            (egui::TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
            (egui::TextStyle::Body, FontId::new(UI_SIZE, FontFamily::Proportional)),
            (egui::TextStyle::Button, FontId::new(UI_SIZE, FontFamily::Proportional)),
            (egui::TextStyle::Heading, FontId::new(15.0, FontFamily::Name("bold".into()))),
            (egui::TextStyle::Monospace, FontId::new(UI_SIZE, FontFamily::Monospace)),
        ]
        .into();
    });
    apply_visuals(ctx);
}

fn apply_visuals(ctx: &egui::Context) {
    let p = t();
    let mut v = if p.light { Visuals::light() } else { Visuals::dark() };
    v.panel_fill = p.panel_bg;
    v.window_fill = p.dialog_bg;
    v.extreme_bg_color = p.edit_bg;
    v.text_edit_bg_color = Some(p.edit_bg);
    v.faint_bg_color = p.row_hot;
    v.code_bg_color = p.combo_bg;
    v.override_text_color = Some(p.text);
    v.window_stroke = Stroke::new(1.0, p.menu_border);
    v.window_corner_radius = CornerRadius::same(0);
    v.menu_corner_radius = CornerRadius::same(0);
    v.window_shadow = egui::epaint::Shadow { offset: [2, 3], blur: 10, spread: 0, color: Color32::from_black_alpha(110) };
    v.popup_shadow = egui::epaint::Shadow { offset: [1, 2], blur: 6, spread: 0, color: Color32::from_black_alpha(90) };
    v.selection.bg_fill = rgb(0x0078D4);
    v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    v.hyperlink_color = p.hyperlink;
    v.text_cursor.stroke = Stroke::new(1.0, p.text);
    let r = CornerRadius::same(2);
    v.widgets.noninteractive.bg_fill = p.panel_bg;
    v.widgets.noninteractive.weak_bg_fill = p.panel_bg;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.line_strong);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.noninteractive.corner_radius = r;
    v.widgets.inactive.bg_fill = p.combo_bg;
    v.widgets.inactive.weak_bg_fill = p.button_bg;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, p.edit_border);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.inactive.corner_radius = r;
    v.widgets.inactive.expansion = 0.0;
    v.widgets.hovered.bg_fill = p.button_hot;
    v.widgets.hovered.weak_bg_fill = p.button_hot;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.accent);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, p.text_strong);
    v.widgets.hovered.corner_radius = r;
    v.widgets.hovered.expansion = 0.0;
    v.widgets.active.bg_fill = p.button_pressed;
    v.widgets.active.weak_bg_fill = p.button_pressed;
    v.widgets.active.bg_stroke = Stroke::new(1.0, p.accent);
    v.widgets.active.fg_stroke = Stroke::new(1.0, p.text_strong);
    v.widgets.active.corner_radius = r;
    v.widgets.active.expansion = 0.0;
    v.widgets.open.bg_fill = p.combo_bg;
    v.widgets.open.weak_bg_fill = p.combo_bg;
    v.widgets.open.bg_stroke = Stroke::new(1.0, p.accent);
    v.widgets.open.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.open.corner_radius = r;
    v.striped = false;
    v.slider_trailing_fill = true;
    v.handle_shape = egui::style::HandleShape::Circle;
    ctx.set_visuals_of(egui::Theme::Dark, v.clone());
    ctx.set_visuals_of(egui::Theme::Light, v);
    ctx.set_theme(if p.light { egui::ThemePreference::Light } else { egui::ThemePreference::Dark });
}

/// Segoe UI (regular, bold, light) from the Windows fonts folder, with egui's default font as the fallback,
/// plus the Chinese/Japanese UI font of the interface language.
pub fn install_fonts(ctx: &egui::Context) {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    let fonts = std::path::Path::new(&windir).join("Fonts");
    let mut defs = egui::FontDefinitions::default();
    let mut load = |name: &str, files: &[&str]| -> bool {
        for f in files {
            if let Ok(bytes) = std::fs::read(fonts.join(f)) {
                defs.font_data.insert(name.to_string(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
                return true;
            }
        }
        false
    };
    let has_regular = load("segoeui", &["segoeui.ttf"]);
    let has_bold = load("segoeuib", &["segoeuib.ttf"]);
    let has_light = load("segoeuil", &["segoeuil.ttf", "segoeuisl.ttf"]);
    let has_italic = load("segoeuii", &["segoeuii.ttf"]);
    let has_symbol = load("segoeuisym", &["seguisym.ttf"]);
    let has_mono = load("consolas", &["consola.ttf"]);
    let prop = defs.families.entry(FontFamily::Proportional).or_default();
    if has_regular {
        prop.insert(0, "segoeui".into());
    }
    if has_symbol {
        prop.push("segoeuisym".into());
    }
    let base = defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    if has_mono {
        defs.families.entry(FontFamily::Monospace).or_default().insert(0, "consolas".into());
    }
    let with = |first: bool, name: &str| {
        let mut v = base.clone();
        if first {
            v.insert(0, name.to_string());
        }
        v
    };
    defs.families.insert(FontFamily::Name("bold".into()), with(has_bold, "segoeuib"));
    defs.families.insert(FontFamily::Name("light".into()), with(has_light, "segoeuil"));
    defs.families.insert(FontFamily::Name("italic".into()), with(has_italic, "segoeuii"));
    crate::i18n::add_fonts(&mut defs, &fonts);
    ctx.set_fonts(defs);
}

pub fn body(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}
pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}
pub fn light(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("light".into()))
}
pub fn italic(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("italic".into()))
}

/// `#RRGGBB` of a colour (icon recolouring).
pub fn hex(c: Color32) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b())
}
