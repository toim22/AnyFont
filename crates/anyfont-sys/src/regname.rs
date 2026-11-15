//! Registry value names and data for registered fonts.
//!
//! Dossier `CdrFnt` 0x180203780 builds the value name:
//! - single face: full name + `" (TrueType)"`;
//! - collection (`.ttc`): the full names of all faces joined with `" & "` + `" (TrueType)"`;
//! - Type 1: the full name with `-` replaced by a space and no suffix;
//! - if the file is already registered under some name (0x180202BC0), that name is reused.
//!
//! The original appends `(TrueType)` to CFF-flavoured OpenType too; AnyFont follows the Windows
//! convention (`(OpenType)` for CFF outlines, as the Explorer font installer writes it). Both are
//! read back correctly. Value data (0x1802010F0): REG_SZ file name for `Fonts`, REG_MULTI_SZ
//! `T`, `<pfm>`, `<pfb>` for `Type 1 Installer\Type 1 Fonts`.

use std::path::Path;

use crate::fontfile::FontFormat;

/// Registry key paths (under HKLM / HKCU).
pub const FONTS_KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts";
pub const TYPE1_KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Type 1 Installer\Type 1 Fonts";

pub const TRUETYPE_SUFFIX: &str = " (TrueType)";
pub const OPENTYPE_SUFFIX: &str = " (OpenType)";

/// Value name for a font file.
pub fn value_name(full_names: &[String], format: FontFormat, has_cff: bool) -> String {
    let clean: Vec<String> = full_names.iter().map(|n| sanitize(n)).filter(|n| !n.is_empty()).collect();
    let base = if clean.is_empty() { "Unnamed font".to_string() } else { clean.join(" & ") };
    match format {
        FontFormat::Type1 => base.replace('-', " "),
        FontFormat::TrueType => base + TRUETYPE_SUFFIX,
        FontFormat::OpenTypeCff => base + OPENTYPE_SUFFIX,
        FontFormat::Collection => base + if has_cff { OPENTYPE_SUFFIX } else { TRUETYPE_SUFFIX },
        FontFormat::Bitmap => base,
    }
}

/// Registry value names cannot contain NUL; keep them on one line and of sane length.
fn sanitize(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    s.chars().take(200).collect()
}

/// Split a value name into the display name and the parenthesised technology suffix, e.g.
/// `"Arial Bold (TrueType)"` -> `("Arial Bold", Some("TrueType"))`,
/// `"Modern (All res)"` -> `("Modern", Some("All res"))`.
pub fn parse_value_name(value: &str) -> (&str, Option<&str>) {
    let v = value.trim_end();
    if v.ends_with(')') {
        if let Some(open) = v.rfind(" (") {
            return (&v[..open], Some(&v[open + 2..v.len() - 1]));
        }
    }
    (v, None)
}

/// Faces named by a value name (`"A & B (TrueType)"` -> `["A", "B"]`).
pub fn face_names(value: &str) -> Vec<&str> {
    parse_value_name(value).0.split(" & ").map(str::trim).filter(|s| !s.is_empty()).collect()
}

/// Value data to store for `file` registered in a scope whose fonts folder is `fonts_dir`:
/// the bare file name when the file is directly in the system fonts folder (Windows convention
/// for HKLM), the full path otherwise (always for HKCU).
pub fn data_for(file: &Path, system_fonts_dir: Option<&Path>) -> String {
    if let Some(dir) = system_fonts_dir {
        if crate::util::is_directly_in(dir, file) {
            if let Some(n) = file.file_name() {
                return n.to_string_lossy().into_owned();
            }
        }
    }
    file.to_string_lossy().into_owned()
}

/// REG_MULTI_SZ items of a Type 1 registration: `["T", pfm, pfb]`.
pub fn type1_data(pfm: &str, pfb: &str) -> Vec<String> {
    vec!["T".to_string(), pfm.to_string(), pfb.to_string()]
}

/// Parse Type 1 REG_MULTI_SZ items back into `(pfm, pfb)`.
pub fn parse_type1_data(items: &[String]) -> Option<(String, String)> {
    let rest: Vec<&String> = items.iter().filter(|s| !s.is_empty()).collect();
    let rest = match rest.first() {
        Some(t) if t.len() == 1 && t.chars().all(|c| c.is_ascii_alphabetic()) => &rest[1..],
        _ => &rest[..],
    };
    let pfm = rest.iter().find(|s| s.to_lowercase().ends_with(".pfm"))?;
    let pfb = rest.iter().find(|s| s.to_lowercase().ends_with(".pfb"))?;
    Some((pfm.to_string(), pfb.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn names_per_format() {
        assert_eq!(value_name(&s(&["Arial Bold"]), FontFormat::TrueType, false), "Arial Bold (TrueType)");
        assert_eq!(value_name(&s(&["Source Sans Pro"]), FontFormat::OpenTypeCff, true), "Source Sans Pro (OpenType)");
        assert_eq!(
            value_name(&s(&["Cambria", "Cambria Math"]), FontFormat::Collection, false),
            "Cambria & Cambria Math (TrueType)"
        );
        assert_eq!(value_name(&s(&["A", "B"]), FontFormat::Collection, true), "A & B (OpenType)");
        assert_eq!(value_name(&s(&["Helvetica-BoldOblique"]), FontFormat::Type1, false), "Helvetica BoldOblique");
    }

    #[test]
    fn names_are_sanitised() {
        assert_eq!(value_name(&s(&["  Odd\u{0}Name\n Bold "]), FontFormat::TrueType, false), "Odd Name Bold (TrueType)");
        assert_eq!(value_name(&[], FontFormat::TrueType, false), "Unnamed font (TrueType)");
    }

    #[test]
    fn parse_names() {
        assert_eq!(parse_value_name("Arial Bold (TrueType)"), ("Arial Bold", Some("TrueType")));
        assert_eq!(parse_value_name("Modern (All res)"), ("Modern", Some("All res")));
        assert_eq!(parse_value_name("Helvetica Bold"), ("Helvetica Bold", None));
        assert_eq!(face_names("Cambria & Cambria Math (TrueType)"), vec!["Cambria", "Cambria Math"]);
    }

    #[test]
    fn data_relative_only_in_system_folder() {
        let sys = PathBuf::from(r"C:\Windows\Fonts");
        assert_eq!(data_for(&sys.join("x.ttf"), Some(&sys)), "x.ttf");
        assert_eq!(data_for(Path::new(r"D:\f\x.ttf"), Some(&sys)), r"D:\f\x.ttf");
        assert_eq!(data_for(Path::new(r"C:\Users\u\AppData\Local\Microsoft\Windows\Fonts\x.ttf"), None), r"C:\Users\u\AppData\Local\Microsoft\Windows\Fonts\x.ttf");
    }

    #[test]
    fn type1_multi_sz() {
        let d = type1_data("HV.PFM", "HV.PFB");
        assert_eq!(d, s(&["T", "HV.PFM", "HV.PFB"]));
        assert_eq!(parse_type1_data(&d), Some(("HV.PFM".into(), "HV.PFB".into())));
        assert_eq!(parse_type1_data(&s(&["T", "a.pfm"])), None);
        assert_eq!(parse_type1_data(&s(&["a.pfm", "a.pfb", ""])), Some(("a.pfm".into(), "a.pfb".into())));
    }
}
