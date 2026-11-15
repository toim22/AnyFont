//! Display formatting: the exact captions the font list and the Font info tab show.
//!
//! - face labels of the list (`Agency FB (Regular, Normal)`, `Arial (Normal-Italic)`, `Arial Black`),
//! - file size (`74.8 KB`, `353 KB`), weight / width names (`Normal`, `Extra-condensed`),
//! - code pages (`1252  (ANSI - Latin I)`), Italic (`Yes`/`No`),
//! - the status bar count text (`My fonts: 681 fonts`, `My fonts (filtered): 10 fonts`).
//!
//! English only; the UI translates these strings through its i18n layer.

use crate::model::FontRecord;

/// Legacy style name of a face: `Normal`, `Normal-Italic`, `Bold`, `Bold-Italic`, or `None` for
/// faces outside the four basic styles (they are labelled with their full name).
pub fn basic_style_name(weight_class: u16, italic: bool) -> Option<&'static str> {
    let w = (((weight_class.clamp(1, 1000) + 50) / 100) * 100).clamp(100, 900);
    match (w, italic) {
        (400 | 500, false) => Some("Normal"),
        (400 | 500, true) => Some("Normal-Italic"),
        (700, false) => Some("Bold"),
        (700, true) => Some("Bold-Italic"),
        _ => None,
    }
}

fn squash(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect()
}

/// Face label shown above each preview in the font list.
///
/// Rules reproduced from the screenshots (`06-panels/font-list/*.png`):
/// - basic styles get the family plus the style in parentheses; a `Regular` sub-family is listed too:
///   `Arial (Regular, Normal)`, `Arial (Normal-Italic)`, `Arial (Bold)`, `Arial (Bold-Italic)`;
/// - other weights show the full name without parentheses: `Arial Black`,
///   `Bahnschrift Light Condensed`.
pub fn face_label(font: &FontRecord, language: u16, show_latin: bool) -> String {
    let family = font.display_legacy_family(language, show_latin);
    if font.corrupt.is_some() && font.family.is_empty() {
        return font
            .path()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
    }
    let sub = font.subfamily.trim();
    match basic_style_name(font.weight_class, font.italic) {
        Some(style) => {
            let sq_sub = squash(sub);
            let equivalent = match style {
                "Normal" => sq_sub == "normal",
                "Normal-Italic" => sq_sub == "italic" || sq_sub == "normalitalic" || sq_sub == "oblique",
                "Bold" => sq_sub == "bold",
                "Bold-Italic" => sq_sub == "bolditalic" || sq_sub == "boldoblique",
                _ => false,
            };
            if equivalent || sub.is_empty() {
                format!("{family} ({style})")
            } else {
                // "Regular" (or any other sub-family name) is listed before the basic style
                format!("{family} ({sub}, {style})")
            }
        }
        None => {
            let sq = squash(sub);
            if sub.is_empty() || sq == "regular" || sq == "normal" || squash(&family).ends_with(&sq) {
                family
            } else {
                format!("{family} {sub}")
            }
        }
    }
}

/// File size as the Font info tab shows it: three significant digits, binary units
/// (`74.8 KB`, `353 KB`, `1.25 MB`, `512 bytes`).
pub fn file_size(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} bytes");
    }
    let units = ["KB", "MB", "GB", "TB"];
    let mut v = bytes as f64 / 1024.0;
    let mut i = 0;
    while v >= 1000.0 && i + 1 < units.len() {
        v /= 1024.0;
        i += 1;
    }
    let s = if v >= 100.0 {
        format!("{:.0}", v)
    } else if v >= 10.0 {
        format!("{:.1}", v)
    } else {
        format!("{:.2}", v)
    };
    format!("{s} {}", units[i])
}

/// Weight name of a weight class (`Thin` ... `Black`), as in the Font info tab.
pub fn weight_name(weight_class: u16) -> &'static str {
    match (((weight_class.clamp(1, 1000) + 50) / 100) * 100).clamp(100, 900) {
        100 => "Thin",
        200 => "Extra-light",
        300 => "Light",
        400 => "Normal",
        500 => "Medium",
        600 => "Semi-bold",
        700 => "Bold",
        800 => "Extra-bold",
        _ => "Black",
    }
}

/// Width name of a width class 1..9 (`Ultra-condensed` ... `Ultra-expanded`).
pub fn width_name(width_class: u16) -> &'static str {
    match width_class {
        1 => "Ultra-condensed",
        2 => "Extra-condensed",
        3 => "Condensed",
        4 => "Semi-condensed",
        6 => "Semi-expanded",
        7 => "Expanded",
        8 => "Extra-expanded",
        9 => "Ultra-expanded",
        _ => "Normal",
    }
}

/// `Yes` / `No`.
pub fn yes_no(v: bool) -> &'static str {
    if v {
        "Yes"
    } else {
        "No"
    }
}

/// `OS/2.ulCodePageRange` bits: (bit, code page number or 0, description), in Windows order.
pub const CODE_PAGES: &[(u8, u16, &str)] = &[
    (0, 1252, "ANSI - Latin I"),
    (1, 1250, "ANSI - Central Europe"),
    (2, 1251, "ANSI - Cyrillic"),
    (3, 1253, "ANSI - Greek"),
    (4, 1254, "ANSI - Turkish"),
    (5, 1255, "ANSI - Hebrew"),
    (6, 1256, "ANSI - Arabic"),
    (7, 1257, "ANSI - Baltic"),
    (8, 1258, "ANSI/OEM - Viet Nam"),
    (16, 874, "ANSI/OEM - Thai"),
    (17, 932, "ANSI/OEM - Japanese, Shift-JIS"),
    (18, 936, "ANSI/OEM - Simplified Chinese GBK"),
    (19, 949, "ANSI/OEM - Korean (Unified Hangul Code)"),
    (20, 950, "ANSI/OEM - Traditional Chinese Big5"),
    (21, 1361, "Korean (Johab)"),
    (29, 0, "Macintosh Character Set (US Roman)"),
    (30, 0, "OEM Character Set"),
    (31, 0, "Symbol Character Set"),
    (48, 869, "IBM Greek"),
    (49, 866, "MS-DOS Russian"),
    (50, 865, "MS-DOS Nordic"),
    (51, 864, "Arabic"),
    (52, 863, "MS-DOS Canadian French"),
    (53, 862, "Hebrew"),
    (54, 861, "MS-DOS Icelandic"),
    (55, 860, "MS-DOS Portuguese"),
    (56, 857, "IBM Turkish"),
    (57, 855, "IBM Cyrillic; primarily Russian"),
    (58, 852, "Latin 2"),
    (59, 775, "MS-DOS Baltic"),
    (60, 737, "Greek; former 437 G"),
    (61, 708, "Arabic; ASMO 708"),
    (62, 850, "WE/Latin 1"),
    (63, 437, "US"),
];

/// A supported code page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodePage {
    /// Code page number (0 for the character-set bits without a number).
    pub number: u16,
    /// Description.
    pub description: &'static str,
}

impl CodePage {
    /// Line as shown in the Font info tab: `1252  (ANSI - Latin I)`.
    pub fn caption(&self) -> String {
        if self.number == 0 {
            self.description.to_string()
        } else {
            format!("{}  ({})", self.number, self.description)
        }
    }
}

/// Code pages a face supports, sorted by code page number like the Font info tab
/// (1250, 1251, 1252 ...). Numberless character sets come last.
pub fn code_pages(range: [u32; 2]) -> Vec<CodePage> {
    let bits = (range[0] as u64) | ((range[1] as u64) << 32);
    let mut v: Vec<CodePage> = CODE_PAGES
        .iter()
        .filter(|(bit, _, _)| bits & (1u64 << bit) != 0)
        .map(|&(_, number, description)| CodePage { number, description })
        .collect();
    v.sort_by_key(|c| if c.number == 0 { u32::MAX } else { c.number as u32 });
    v
}

/// `"1 glyph"`, `"262 glyphs"`.
pub fn count_text(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

/// Status bar text: `My fonts: 681 fonts`, `My fonts (filtered): 10 fonts`, `fm-map-temp-folder: 2 fonts`.
pub fn status_text(library_name: &str, count: usize, filtered: bool) -> String {
    let n = count_text(count, "font");
    if filtered {
        format!("{library_name} (filtered): {n}")
    } else {
        format!("{library_name}: {n}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(family: &str, sub: &str, w: u16, italic: bool) -> FontRecord {
        FontRecord {
            family: family.into(),
            subfamily: sub.into(),
            weight_class: w,
            italic,
            ..Default::default()
        }
    }

    #[test]
    fn labels_match_screenshots() {
        assert_eq!(face_label(&rec("Arial", "Regular", 400, false), 0x409, false), "Arial (Regular, Normal)");
        assert_eq!(face_label(&rec("Arial", "Italic", 400, true), 0x409, false), "Arial (Normal-Italic)");
        assert_eq!(face_label(&rec("Arial", "Bold", 700, false), 0x409, false), "Arial (Bold)");
        assert_eq!(face_label(&rec("Arial", "Bold Italic", 700, true), 0x409, false), "Arial (Bold-Italic)");
        assert_eq!(face_label(&rec("Arial Black", "Regular", 900, false), 0x409, false), "Arial Black");
        assert_eq!(face_label(&rec("Agency FB", "Bold", 700, false), 0x409, false), "Agency FB (Bold)");
        assert_eq!(
            face_label(&rec("Bahnschrift", "Light Condensed", 300, false), 0x409, false),
            "Bahnschrift Light Condensed"
        );
    }

    #[test]
    fn sizes_and_names() {
        assert_eq!(file_size(76_595), "74.8 KB");
        assert_eq!(file_size(361_472), "353 KB");
        assert_eq!(file_size(500), "500 bytes");
        assert_eq!(weight_name(400), "Normal");
        assert_eq!(weight_name(300), "Light");
        assert_eq!(width_name(2), "Extra-condensed");
        let cps = code_pages([1 | 2, 0]);
        assert_eq!(cps[0].caption(), "1250  (ANSI - Central Europe)");
        assert_eq!(cps[1].caption(), "1252  (ANSI - Latin I)");
        assert_eq!(status_text("My fonts", 681, false), "My fonts: 681 fonts");
        assert_eq!(status_text("My fonts", 10, true), "My fonts (filtered): 10 fonts");
    }
}
