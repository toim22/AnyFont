//! Minimal font file inspection needed for registration: format and the full names that make up
//! the registry value name. (Detailed metadata is `anyfont-core`'s job.)

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::FontOpError;

/// Font file kinds as Windows registers them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FontFormat {
    /// `glyf` outlines (`.ttf`, or `.otf` with TrueType outlines).
    TrueType,
    /// CFF / CFF2 outlines (`.otf`).
    OpenTypeCff,
    /// TrueType/OpenType collection (`.ttc`, `.otc`).
    Collection,
    /// PostScript Type 1 (`.pfm` metrics + `.pfb` outlines).
    Type1,
    /// Windows bitmap / vector fonts (`.fon`, `.fnt`): listed, never installed by AnyFont.
    Bitmap,
}

/// Result of [`inspect`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontFileInfo {
    pub format: FontFormat,
    /// Full name (name ID 4, English preferred) of every face; one entry unless a collection.
    pub full_names: Vec<String>,
    /// Collection contains at least one CFF face.
    pub has_cff: bool,
    /// Type 1: the `.pfm` and `.pfb` files (metrics first).
    pub type1_files: Option<(PathBuf, PathBuf)>,
}

impl FontFileInfo {
    /// The registry value name Windows (and AnyFont) uses for this file.
    pub fn registry_value_name(&self) -> String {
        crate::regname::value_name(&self.full_names, self.format, self.has_cff)
    }
}

/// Lower-case extension without the dot.
pub fn extension(path: &Path) -> String {
    path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

/// Extensions AnyFont installs.
pub const INSTALLABLE_EXTENSIONS: &[&str] = &["ttf", "otf", "ttc", "otc", "pfm", "pfb"];

/// Inspect a font file. For Type 1 either the `.pfm` or the `.pfb` may be given; the partner with
/// the same stem in the same folder is required.
pub fn inspect(path: &Path) -> Result<FontFileInfo, FontOpError> {
    if !path.is_file() {
        return Err(FontOpError::NotFound);
    }
    let ext = extension(path);
    match ext.as_str() {
        "pfm" | "pfb" => inspect_type1(path),
        "fon" | "fnt" => Err(FontOpError::UnsupportedFormat(ext)),
        "ttf" | "otf" | "ttc" | "otc" | "tte" => inspect_sfnt(path),
        other => {
            // Unknown extension: accept it only if the content is an sfnt.
            match inspect_sfnt(path) {
                Ok(i) => Ok(i),
                Err(_) => Err(FontOpError::UnsupportedFormat(if other.is_empty() { "no extension".into() } else { other.to_string() })),
            }
        }
    }
}

fn inspect_sfnt(path: &Path) -> Result<FontFileInfo, FontOpError> {
    let data = std::fs::read(path)?;
    let fallback = || path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    if let Some(count) = ttf_parser::fonts_in_collection(&data) {
        if count == 0 {
            return Err(FontOpError::InvalidFont("empty collection".into()));
        }
        let mut names = Vec::new();
        let mut has_cff = false;
        for i in 0..count {
            let face = ttf_parser::Face::parse(&data, i).map_err(|e| FontOpError::InvalidFont(e.to_string()))?;
            has_cff |= is_cff(&face);
            names.push(full_name(&face).unwrap_or_else(|| format!("{} {}", fallback(), i + 1)));
        }
        return Ok(FontFileInfo { format: FontFormat::Collection, full_names: names, has_cff, type1_files: None });
    }
    let face = ttf_parser::Face::parse(&data, 0).map_err(|e| FontOpError::InvalidFont(e.to_string()))?;
    let cff = is_cff(&face);
    Ok(FontFileInfo {
        format: if cff { FontFormat::OpenTypeCff } else { FontFormat::TrueType },
        full_names: vec![full_name(&face).unwrap_or_else(fallback)],
        has_cff: cff,
        type1_files: None,
    })
}

fn is_cff(face: &ttf_parser::Face<'_>) -> bool {
    let raw = face.raw_face();
    raw.table(ttf_parser::Tag::from_bytes(b"CFF ")).is_some() || raw.table(ttf_parser::Tag::from_bytes(b"CFF2")).is_some()
}

/// Full name (ID 4): Windows platform US English first, then any Windows Unicode, then any
/// decodable record; falls back to "Family Subfamily".
pub fn full_name(face: &ttf_parser::Face<'_>) -> Option<String> {
    pick_name(face, ttf_parser::name_id::FULL_NAME).or_else(|| {
        let fam = pick_name(face, ttf_parser::name_id::FAMILY)?;
        match pick_name(face, ttf_parser::name_id::SUBFAMILY) {
            Some(sub) if !sub.eq_ignore_ascii_case("regular") => Some(format!("{fam} {sub}")),
            _ => Some(fam),
        }
    })
}

fn pick_name(face: &ttf_parser::Face<'_>, id: u16) -> Option<String> {
    let mut best: Option<(u8, String)> = None;
    for name in face.names() {
        if name.name_id != id {
            continue;
        }
        let Some(text) = name.to_string() else { continue };
        let text = text.trim().to_string();
        if text.is_empty() {
            continue;
        }
        let rank = match (name.platform_id, name.language_id) {
            (ttf_parser::PlatformId::Windows, 0x0409) => 0,
            (ttf_parser::PlatformId::Windows, _) => 1,
            (ttf_parser::PlatformId::Unicode, _) => 2,
            _ => 3,
        };
        if best.as_ref().is_none_or(|(r, _)| rank < *r) {
            best = Some((rank, text));
        }
    }
    best.map(|(_, t)| t)
}

fn inspect_type1(path: &Path) -> Result<FontFileInfo, FontOpError> {
    let (pfm, pfb) = type1_pair(path)?;
    let data = std::fs::read(&pfm)?;
    let name = pfm_postscript_name(&data)
        .or_else(|| pfm_face_name(&data))
        .or_else(|| pfm.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .ok_or_else(|| FontOpError::InvalidFont("unreadable .pfm".into()))?;
    Ok(FontFileInfo { format: FontFormat::Type1, full_names: vec![name], has_cff: false, type1_files: Some((pfm, pfb)) })
}

/// Find the `.pfm` + `.pfb` pair for either file.
pub fn type1_pair(path: &Path) -> Result<(PathBuf, PathBuf), FontOpError> {
    let ext = extension(path);
    let want = if ext == "pfm" { "pfb" } else { "pfm" };
    let partner = find_with_extension(path, want).ok_or_else(|| FontOpError::MissingCompanion(path.with_extension(want)))?;
    if ext == "pfm" {
        Ok((path.to_path_buf(), partner))
    } else {
        Ok((partner, path.to_path_buf()))
    }
}

fn find_with_extension(path: &Path, ext: &str) -> Option<PathBuf> {
    let stem = path.file_stem()?.to_string_lossy().to_lowercase();
    let dir = path.parent()?;
    // Scan first so the name keeps its on-disk spelling (it ends up in the registry data).
    let scanned = std::fs::read_dir(dir).ok().and_then(|rd| {
        rd.flatten().map(|e| e.path()).find(|p| {
            p.is_file()
                && extension(p) == ext
                && p.file_stem().map(|s| s.to_string_lossy().to_lowercase() == stem).unwrap_or(false)
        })
    });
    scanned.or_else(|| {
        let direct = path.with_extension(ext);
        direct.is_file().then_some(direct)
    })
}

fn read_u16(d: &[u8], o: usize) -> Option<u16> {
    d.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]))
}
fn read_u32(d: &[u8], o: usize) -> Option<u32> {
    d.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}
fn c_string(d: &[u8], o: usize) -> Option<String> {
    let bytes = d.get(o..)?;
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len()).min(128);
    let s: String = bytes[..end].iter().map(|b| *b as char).collect();
    let s = s.trim().to_string();
    (!s.is_empty() && s.chars().all(|c| !c.is_control())).then_some(s)
}

/// PostScript font name from the PFM extension's `dfDriverInfo` offset (byte 139).
pub fn pfm_postscript_name(d: &[u8]) -> Option<String> {
    if read_u16(d, 0)? != 0x0100 && read_u16(d, 0)? != 0x0200 {
        return None;
    }
    let off = read_u32(d, 139)? as usize;
    if off == 0 || off >= d.len() {
        return None;
    }
    c_string(d, off)
}

/// Windows face name from `dfFace` (byte 105).
pub fn pfm_face_name(d: &[u8]) -> Option<String> {
    let off = read_u32(d, 105)? as usize;
    if off == 0 || off >= d.len() {
        return None;
    }
    c_string(d, off)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Build a minimal PFM header with a face name and a PostScript name.
    pub(crate) fn fake_pfm(face: &str, ps: &str) -> Vec<u8> {
        let mut d = vec![0u8; 147];
        d[0..2].copy_from_slice(&0x0100u16.to_le_bytes());
        let face_off = d.len() as u32;
        d.extend_from_slice(face.as_bytes());
        d.push(0);
        let ps_off = d.len() as u32;
        d.extend_from_slice(ps.as_bytes());
        d.push(0);
        let size = d.len() as u32;
        d[2..6].copy_from_slice(&size.to_le_bytes());
        d[105..109].copy_from_slice(&face_off.to_le_bytes());
        d[139..143].copy_from_slice(&ps_off.to_le_bytes());
        d
    }

    #[test]
    fn pfm_names() {
        let d = fake_pfm("Helvetica", "Helvetica-BoldOblique");
        assert_eq!(pfm_postscript_name(&d).as_deref(), Some("Helvetica-BoldOblique"));
        assert_eq!(pfm_face_name(&d).as_deref(), Some("Helvetica"));
    }

    #[test]
    fn type1_pair_and_name() {
        let dir = tempfile::tempdir().unwrap();
        let pfm = dir.path().join("HVB.PFM");
        std::fs::write(&pfm, fake_pfm("Helvetica", "Helvetica-Bold")).unwrap();
        assert!(matches!(inspect(&pfm), Err(FontOpError::MissingCompanion(_))));
        std::fs::write(dir.path().join("hvb.pfb"), b"\x80\x01fake").unwrap();
        let info = inspect(&dir.path().join("hvb.pfb")).unwrap();
        assert_eq!(info.format, FontFormat::Type1);
        assert_eq!(info.registry_value_name(), "Helvetica Bold");
        let (m, b) = info.type1_files.unwrap();
        assert_eq!(extension(&m), "pfm");
        assert_eq!(extension(&b), "pfb");
    }

    #[test]
    fn rejects_non_fonts() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.txt");
        std::fs::write(&p, b"hello").unwrap();
        assert!(matches!(inspect(&p), Err(FontOpError::UnsupportedFormat(_))));
        let p = dir.path().join("x.ttf");
        std::fs::write(&p, b"hello").unwrap();
        assert!(matches!(inspect(&p), Err(FontOpError::InvalidFont(_))));
        assert_eq!(inspect(&dir.path().join("missing.ttf")), Err(FontOpError::NotFound));
    }

    #[test]
    fn system_font_names() {
        // Read-only use of fonts that ship with Windows.
        let windir = std::env::var_os("WINDIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        let arial = windir.join("Fonts").join("arial.ttf");
        if arial.is_file() {
            let info = inspect(&arial).unwrap();
            assert_eq!(info.format, FontFormat::TrueType);
            assert_eq!(info.registry_value_name(), "Arial (TrueType)");
        }
        let cambria = windir.join("Fonts").join("cambria.ttc");
        if cambria.is_file() {
            let info = inspect(&cambria).unwrap();
            assert_eq!(info.format, FontFormat::Collection);
            assert_eq!(info.registry_value_name(), "Cambria & Cambria Math (TrueType)");
        }
    }
}
