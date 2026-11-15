//! Low-level sfnt (TrueType/OpenType) reading and the validation rules of the original font engine.
//!
//! The checks reproduce `CFontInformerOTF_STREAM::Initialize` / `ReadOffsetTable` (the font engine DLL
//! 0x18017F470 / 0x18017FC90), documented in `05-data-models/file-formats/font-files.md` §6:
//!
//! | Check | Rule |
//! |---|---|
//! | TTC face selection | first dword `ttcf` and `face_index < numFonts`; the face's offset table is `header[faceIndex + 3]` |
//! | table directory | `numTables != 0`; header + directory must fit in the file |
//! | `head` | present, magic `0x5F0F3CF5`, `unitsPerEm` 16..16384, `glyphDataFormat == 0` |
//! | `maxp` | present, version `0x00005000` (CFF) or `0x00010000` (TrueType) - the flavour discriminator |
//! | `cmap` | present, version < 2 |
//! | quirk list | families `Libre Franklin`, `Libre Franklin Italics`, `Nunito VF Beta` are rejected |
//! | outlines | `glyf`+`loca` for maxp 1.0, `CFF ` or `CFF2` for maxp 0.5 (accessors of the original) |
//! | metrics | `hhea`/`hmtx` missing -> warning only (used for scaling, not enforced at open) |
//!
//! Besides validation the module offers raw readers for what `ttf-parser` does not expose:
//! `OS/2` fields (fsType, PANOSE, code pages, unicode ranges, achVendID), the `name` table with
//! platform/language, `fvar` named instances and `STAT` axis names.

use std::fmt;

/// Reads a big-endian u16.
pub(crate) fn be_u16(d: &[u8], off: usize) -> Option<u16> {
    d.get(off..off + 2).map(|b| u16::from_be_bytes([b[0], b[1]]))
}
/// Reads a big-endian i16.
pub(crate) fn be_i16(d: &[u8], off: usize) -> Option<i16> {
    be_u16(d, off).map(|v| v as i16)
}
/// Reads a big-endian u32.
pub(crate) fn be_u32(d: &[u8], off: usize) -> Option<u32> {
    d.get(off..off + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}
/// Reads a 16.16 fixed number.
pub(crate) fn be_fixed(d: &[u8], off: usize) -> Option<f32> {
    be_u32(d, off).map(|v| (v as i32) as f32 / 65536.0)
}

/// A 4-byte table tag.
pub type Tag = [u8; 4];

/// Tag as text (`"OS/2"`).
pub fn tag_str(t: &Tag) -> String {
    t.iter().map(|&b| if (0x20..0x7F).contains(&b) { b as char } else { '?' }).collect()
}

/// Why a file is rejected by the validator.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ValidationError {
    /// File shorter than an sfnt header.
    #[error("file is too short to be a font")]
    TooShort,
    /// First dword is not a known sfnt/collection signature.
    #[error("unknown font signature 0x{0:08X}")]
    UnknownSignature(u32),
    /// `faceIndex >= numFonts` in a collection.
    #[error("face {index} does not exist (the collection has {count} faces)")]
    FaceIndexOutOfRange {
        /// Requested face.
        index: u32,
        /// Faces in the collection.
        count: u32,
    },
    /// `numTables == 0`.
    #[error("the font has no tables")]
    NoTables,
    /// Offset table or directory past the end of the file.
    #[error("the table directory is truncated")]
    DirectoryOutOfBounds,
    /// A table record points outside the file.
    #[error("table '{0}' lies outside the file")]
    TableOutOfBounds(String),
    /// A mandatory table is missing.
    #[error("required table '{0}' is missing")]
    MissingTable(&'static str),
    /// `head.magicNumber != 0x5F0F3CF5`.
    #[error("invalid 'head' table (bad magic number)")]
    BadHeadMagic,
    /// `head.unitsPerEm` outside 16..16384.
    #[error("invalid units per em {0} (must be 16..16384)")]
    BadUnitsPerEm(u16),
    /// `head.glyphDataFormat != 0`.
    #[error("unsupported glyph data format {0}")]
    BadGlyphDataFormat(i16),
    /// `maxp.version` is neither 0.5 nor 1.0.
    #[error("unsupported 'maxp' version 0x{0:08X}")]
    BadMaxpVersion(u32),
    /// `cmap.version >= 2`.
    #[error("unsupported 'cmap' version {0}")]
    BadCmapVersion(u16),
    /// Outline tables for the flavour are missing.
    #[error("outline table '{0}' is missing")]
    MissingOutlines(&'static str),
    /// Family on the known-problem list of the original engine.
    #[error("known problem font '{0}'")]
    KnownProblemFont(String),
}

/// Families rejected by the original validator (`sub_180217FE0`, known-bad variable fonts).
pub const QUIRK_FAMILIES: [&str; 3] = ["Libre Franklin", "Libre Franklin Italics", "Nunito VF Beta"];

/// Outline flavour decided by `maxp.version`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flavour {
    /// `maxp` 1.0, `glyf` outlines.
    TrueType,
    /// `maxp` 0.5, CFF/CFF2 outlines.
    Cff,
}

/// One table directory entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableRecord {
    /// Tag.
    pub tag: Tag,
    /// Checksum.
    pub checksum: u32,
    /// Offset from the start of the file.
    pub offset: u32,
    /// Length.
    pub length: u32,
}

/// One face of an sfnt file with its table directory.
#[derive(Clone)]
pub struct SfntFace<'a> {
    /// The whole file.
    pub data: &'a [u8],
    /// sfnt version of the face (`0x00010000`, `OTTO`, `true`).
    pub sfnt_version: u32,
    /// Table directory.
    pub tables: Vec<TableRecord>,
}

impl fmt::Debug for SfntFace<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SfntFace")
            .field("sfnt_version", &format_args!("0x{:08X}", self.sfnt_version))
            .field("tables", &self.tables.iter().map(|t| tag_str(&t.tag)).collect::<Vec<_>>())
            .finish()
    }
}

/// `ttcf`.
pub const TAG_TTCF: u32 = 0x7474_6366;
/// `OTTO`.
pub const TAG_OTTO: u32 = 0x4F54_544F;
/// `true` (old Apple TrueType).
pub const TAG_TRUE: u32 = 0x7472_7565;
/// `wOFF`.
pub const TAG_WOFF: u32 = 0x774F_4646;
/// `wOF2`.
pub const TAG_WOFF2: u32 = 0x774F_4632;

/// Number of faces of a file: `numFonts` for a `ttcf` collection, 1 otherwise.
pub fn face_count(data: &[u8]) -> u32 {
    match be_u32(data, 0) {
        Some(TAG_TTCF) => be_u32(data, 8).unwrap_or(0),
        Some(_) => 1,
        None => 0,
    }
}

/// True when the data starts with an sfnt or collection signature.
pub fn is_sfnt(data: &[u8]) -> bool {
    matches!(be_u32(data, 0), Some(0x0001_0000 | TAG_OTTO | TAG_TRUE | TAG_TTCF))
}

impl<'a> SfntFace<'a> {
    /// Opens face `face_index`: TTC face selection and offset-table checks (§6.2, §6.3).
    pub fn open(data: &'a [u8], face_index: u32) -> Result<Self, ValidationError> {
        let first = be_u32(data, 0).ok_or(ValidationError::TooShort)?;
        let offset = if first == TAG_TTCF {
            let count = be_u32(data, 8).ok_or(ValidationError::TooShort)?;
            if face_index >= count {
                return Err(ValidationError::FaceIndexOutOfRange { index: face_index, count });
            }
            be_u32(data, 12 + 4 * face_index as usize).ok_or(ValidationError::DirectoryOutOfBounds)? as usize
        } else {
            if face_index != 0 {
                return Err(ValidationError::FaceIndexOutOfRange { index: face_index, count: 1 });
            }
            0
        };
        let version = be_u32(data, offset).ok_or(ValidationError::TooShort)?;
        if !matches!(version, 0x0001_0000 | TAG_OTTO | TAG_TRUE) {
            return Err(ValidationError::UnknownSignature(version));
        }
        let num_tables = be_u16(data, offset + 4).ok_or(ValidationError::TooShort)? as usize;
        if num_tables == 0 {
            return Err(ValidationError::NoTables);
        }
        let dir_end = offset + 12 + 16 * num_tables;
        if dir_end > data.len() {
            return Err(ValidationError::DirectoryOutOfBounds);
        }
        let mut tables = Vec::with_capacity(num_tables);
        for i in 0..num_tables {
            let r = offset + 12 + 16 * i;
            let tag = [data[r], data[r + 1], data[r + 2], data[r + 3]];
            let rec = TableRecord {
                tag,
                checksum: be_u32(data, r + 4).unwrap_or(0),
                offset: be_u32(data, r + 8).unwrap_or(0),
                length: be_u32(data, r + 12).unwrap_or(0),
            };
            if (rec.offset as u64 + rec.length as u64) > data.len() as u64 {
                return Err(ValidationError::TableOutOfBounds(tag_str(&tag)));
            }
            tables.push(rec);
        }
        Ok(SfntFace { data, sfnt_version: version, tables })
    }

    /// Table data by tag.
    pub fn table(&self, tag: &Tag) -> Option<&'a [u8]> {
        self.tables
            .iter()
            .find(|t| &t.tag == tag)
            .map(|t| &self.data[t.offset as usize..(t.offset + t.length) as usize])
    }

    /// True if the table exists.
    pub fn has(&self, tag: &Tag) -> bool {
        self.tables.iter().any(|t| &t.tag == tag)
    }

    /// Enforced field checks of `Initialize` (§6.4) and the per-flavour table matrix (§6.5).
    /// Returns the flavour and the non-fatal warnings.
    pub fn validate(&self) -> Result<(Flavour, Vec<String>), ValidationError> {
        let mut warnings = Vec::new();
        let head = self.table(b"head").ok_or(ValidationError::MissingTable("head"))?;
        if be_u32(head, 12) != Some(0x5F0F_3CF5) {
            return Err(ValidationError::BadHeadMagic);
        }
        let upem = be_u16(head, 18).ok_or(ValidationError::BadHeadMagic)?;
        if !(16..=16384).contains(&upem) {
            return Err(ValidationError::BadUnitsPerEm(upem));
        }
        let gdf = be_i16(head, 52).ok_or(ValidationError::BadHeadMagic)?;
        if gdf != 0 {
            return Err(ValidationError::BadGlyphDataFormat(gdf));
        }
        let maxp = self.table(b"maxp").ok_or(ValidationError::MissingTable("maxp"))?;
        let flavour = match be_u32(maxp, 0) {
            Some(0x0000_5000) => Flavour::Cff,
            Some(0x0001_0000) => Flavour::TrueType,
            Some(v) => return Err(ValidationError::BadMaxpVersion(v)),
            None => return Err(ValidationError::MissingTable("maxp")),
        };
        let cmap = self.table(b"cmap").ok_or(ValidationError::MissingTable("cmap"))?;
        let cmap_version = be_u16(cmap, 0).ok_or(ValidationError::MissingTable("cmap"))?;
        if cmap_version >= 2 {
            return Err(ValidationError::BadCmapVersion(cmap_version));
        }
        match flavour {
            Flavour::TrueType => {
                let bitmap_only = self.has(b"EBDT") || self.has(b"CBDT") || self.has(b"sbix");
                if !self.has(b"glyf") && !bitmap_only {
                    return Err(ValidationError::MissingOutlines("glyf"));
                }
                if self.has(b"glyf") && !self.has(b"loca") {
                    return Err(ValidationError::MissingOutlines("loca"));
                }
            }
            Flavour::Cff => {
                if !self.has(b"CFF ") && !self.has(b"CFF2") {
                    return Err(ValidationError::MissingOutlines("CFF "));
                }
            }
        }
        for t in [b"hhea", b"hmtx"] {
            if !self.has(t) {
                warnings.push(format!("table '{}' is missing", tag_str(t)));
            }
        }
        if let Some(name) = self.table(b"name") {
            let names = NameTable::parse(name);
            for id in [1u16, 16] {
                if let Some(f) = names.english(id) {
                    if QUIRK_FAMILIES.iter().any(|q| q.eq_ignore_ascii_case(f.trim())) {
                        return Err(ValidationError::KnownProblemFont(f.trim().to_string()));
                    }
                }
            }
        } else {
            warnings.push("table 'name' is missing".into());
        }
        Ok((flavour, warnings))
    }
}

// ------------------------------------------------------------------------------------------------
// name table
// ------------------------------------------------------------------------------------------------

/// One decoded `name` record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NameRecord {
    /// Platform (0 Unicode, 1 Macintosh, 3 Windows).
    pub platform: u16,
    /// Encoding.
    pub encoding: u16,
    /// Language (Windows LCID or Mac language code).
    pub language: u16,
    /// Name ID.
    pub name_id: u16,
    /// Decoded text.
    pub value: String,
}

/// Decoded `name` table.
#[derive(Clone, Debug, Default)]
pub struct NameTable {
    /// Records that could be decoded.
    pub records: Vec<NameRecord>,
}

const MAC_ROMAN_HIGH: [char; 128] = [
    'Ä', 'Å', 'Ç', 'É', 'Ñ', 'Ö', 'Ü', 'á', 'à', 'â', 'ä', 'ã', 'å', 'ç', 'é', 'è', 'ê', 'ë', 'í', 'ì', 'î', 'ï',
    'ñ', 'ó', 'ò', 'ô', 'ö', 'õ', 'ú', 'ù', 'û', 'ü', '†', '°', '¢', '£', '§', '•', '¶', 'ß', '®', '©', '™', '´',
    '¨', '≠', 'Æ', 'Ø', '∞', '±', '≤', '≥', '¥', 'µ', '∂', '∑', '∏', 'π', '∫', 'ª', 'º', 'Ω', 'æ', 'ø', '¿', '¡',
    '¬', '√', 'ƒ', '≈', '∆', '«', '»', '…', '\u{A0}', 'À', 'Ã', 'Õ', 'Œ', 'œ', '–', '—', '“', '”', '‘', '’',
    '÷', '◊', 'ÿ', 'Ÿ', '⁄', '€', '‹', '›', 'ﬁ', 'ﬂ', '‡', '·', '‚', '„', '‰', 'Â', 'Ê', 'Á', 'Ë', 'È', 'Í',
    'Î', 'Ï', 'Ì', 'Ó', 'Ô', '\u{F8FF}', 'Ò', 'Ú', 'Û', 'Ù', 'ı', 'ˆ', '˜', '¯', '˘', '˙', '˚', '¸', '˝', '˛',
    'ˇ',
];

/// Decodes Mac Roman bytes.
pub fn decode_mac_roman(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| if b < 0x80 { b as char } else { MAC_ROMAN_HIGH[(b - 0x80) as usize] })
        .collect()
}

/// Decodes UTF-16BE bytes (lossy).
pub fn decode_utf16_be(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
    String::from_utf16_lossy(&units)
}

impl NameTable {
    /// Parses a `name` table; undecodable records (legacy CJK Windows encodings, non-Roman Mac
    /// encodings) are skipped. Never fails.
    pub fn parse(data: &[u8]) -> NameTable {
        let mut records = Vec::new();
        let count = be_u16(data, 2).unwrap_or(0) as usize;
        let storage = be_u16(data, 4).unwrap_or(0) as usize;
        for i in 0..count {
            let r = 6 + 12 * i;
            let (Some(platform), Some(encoding), Some(language), Some(name_id), Some(len), Some(off)) = (
                be_u16(data, r),
                be_u16(data, r + 2),
                be_u16(data, r + 4),
                be_u16(data, r + 6),
                be_u16(data, r + 8),
                be_u16(data, r + 10),
            ) else {
                break;
            };
            let start = storage + off as usize;
            let Some(bytes) = data.get(start..start + len as usize) else { continue };
            let value = match (platform, encoding) {
                (0, _) | (3, 0) | (3, 1) | (3, 10) => decode_utf16_be(bytes),
                (1, 0) => decode_mac_roman(bytes),
                _ => continue,
            };
            let value = value.trim_end_matches('\0').to_string();
            records.push(NameRecord { platform, encoding, language, name_id, value });
        }
        NameTable { records }
    }

    /// English (Latin) value of a name ID: Windows en-US, any Windows English, Unicode platform,
    /// Mac English, then any Windows record.
    pub fn english(&self, id: u16) -> Option<&str> {
        let by = |pred: &dyn Fn(&NameRecord) -> bool| {
            self.records.iter().find(|r| r.name_id == id && !r.value.is_empty() && pred(r)).map(|r| r.value.as_str())
        };
        by(&|r| r.platform == 3 && r.language == 0x0409)
            .or_else(|| by(&|r| r.platform == 3 && r.language & 0x3FF == 0x09))
            .or_else(|| by(&|r| r.platform == 0))
            .or_else(|| by(&|r| r.platform == 1 && r.language == 0))
            .or_else(|| by(&|r| r.platform == 3))
    }

    /// Windows value of a name ID in one language.
    pub fn windows(&self, id: u16, language: u16) -> Option<&str> {
        self.records
            .iter()
            .find(|r| r.platform == 3 && r.name_id == id && r.language == language && !r.value.is_empty())
            .map(|r| r.value.as_str())
    }

    /// Distinct Windows languages with at least one record (the original `DBLocales`).
    pub fn windows_languages(&self) -> Vec<u16> {
        let mut v: Vec<u16> = self.records.iter().filter(|r| r.platform == 3).map(|r| r.language).collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// Any value (English first) of a name ID.
    pub fn get(&self, id: u16) -> Option<&str> {
        self.english(id)
    }
}

// ------------------------------------------------------------------------------------------------
// OS/2
// ------------------------------------------------------------------------------------------------

/// Raw `OS/2` fields.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Os2 {
    /// Table version.
    pub version: u16,
    /// usWeightClass.
    pub weight_class: u16,
    /// usWidthClass.
    pub width_class: u16,
    /// fsType.
    pub fs_type: u16,
    /// panose.
    pub panose: [u8; 10],
    /// ulUnicodeRange1..4.
    pub unicode_range: [u32; 4],
    /// achVendID.
    pub vendor_id: [u8; 4],
    /// fsSelection.
    pub fs_selection: u16,
    /// ulCodePageRange1..2 (version >= 1).
    pub code_page_range: [u32; 2],
}

impl Os2 {
    /// Parses the table; returns `None` when it is shorter than the version-0 layout (68 bytes for the
    /// fields used here).
    pub fn parse(d: &[u8]) -> Option<Os2> {
        let mut o = Os2 {
            version: be_u16(d, 0)?,
            weight_class: be_u16(d, 4)?,
            width_class: be_u16(d, 6)?,
            fs_type: be_u16(d, 8)?,
            ..Default::default()
        };
        o.panose.copy_from_slice(d.get(32..42)?);
        for i in 0..4 {
            o.unicode_range[i] = be_u32(d, 42 + 4 * i)?;
        }
        o.vendor_id.copy_from_slice(d.get(58..62)?);
        o.fs_selection = be_u16(d, 62)?;
        if o.version >= 1 {
            o.code_page_range = [be_u32(d, 78).unwrap_or(0), be_u32(d, 82).unwrap_or(0)];
        }
        Some(o)
    }
    /// achVendID as trimmed text.
    pub fn vendor(&self) -> String {
        self.vendor_id
            .iter()
            .map(|&b| if (0x20..0x7F).contains(&b) { b as char } else { ' ' })
            .collect::<String>()
            .trim()
            .to_string()
    }
}

// ------------------------------------------------------------------------------------------------
// fvar / STAT
// ------------------------------------------------------------------------------------------------

/// Raw `fvar` axis.
#[derive(Clone, Debug, PartialEq)]
pub struct FvarAxis {
    /// Tag.
    pub tag: Tag,
    /// Min.
    pub min: f32,
    /// Default.
    pub default: f32,
    /// Max.
    pub max: f32,
    /// Flags (bit 0 = hidden).
    pub flags: u16,
    /// Name ID of the axis name.
    pub name_id: u16,
}

/// Raw `fvar` named instance.
#[derive(Clone, Debug, PartialEq)]
pub struct FvarInstance {
    /// Name ID of the sub-family name.
    pub subfamily_name_id: u16,
    /// Optional PostScript name ID.
    pub postscript_name_id: Option<u16>,
    /// Coordinates.
    pub coordinates: Vec<f32>,
}

/// Parsed `fvar`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fvar {
    /// Axes.
    pub axes: Vec<FvarAxis>,
    /// Named instances.
    pub instances: Vec<FvarInstance>,
}

impl Fvar {
    /// Parses `fvar`; malformed data yields what could be read.
    pub fn parse(d: &[u8]) -> Fvar {
        let mut f = Fvar::default();
        let (Some(axes_off), Some(axis_count), Some(axis_size), Some(inst_count), Some(inst_size)) =
            (be_u16(d, 4), be_u16(d, 8), be_u16(d, 10), be_u16(d, 12), be_u16(d, 14))
        else {
            return f;
        };
        let (axes_off, axis_count, axis_size) = (axes_off as usize, axis_count as usize, axis_size as usize);
        for i in 0..axis_count {
            let r = axes_off + i * axis_size;
            let Some(tag) = d.get(r..r + 4) else { break };
            let (Some(min), Some(def), Some(max), Some(flags), Some(name_id)) =
                (be_fixed(d, r + 4), be_fixed(d, r + 8), be_fixed(d, r + 12), be_u16(d, r + 16), be_u16(d, r + 18))
            else {
                break;
            };
            f.axes.push(FvarAxis { tag: [tag[0], tag[1], tag[2], tag[3]], min, default: def, max, flags, name_id });
        }
        let inst_start = axes_off + axis_count * axis_size;
        let inst_size = inst_size as usize;
        let has_ps = inst_size >= axis_count * 4 + 6;
        for i in 0..inst_count as usize {
            let r = inst_start + i * inst_size;
            let Some(sub) = be_u16(d, r) else { break };
            let mut coords = Vec::with_capacity(axis_count);
            for a in 0..axis_count {
                coords.push(be_fixed(d, r + 4 + 4 * a).unwrap_or(0.0));
            }
            let ps = if has_ps { be_u16(d, r + 4 + 4 * axis_count).filter(|&v| v != 0xFFFF) } else { None };
            f.instances.push(FvarInstance { subfamily_name_id: sub, postscript_name_id: ps, coordinates: coords });
        }
        f
    }
}

/// `STAT` design axes: (tag, name ID).
pub fn stat_axes(d: &[u8]) -> Vec<(Tag, u16)> {
    let mut v = Vec::new();
    let (Some(size), Some(count), Some(off)) = (be_u16(d, 4), be_u16(d, 6), be_u32(d, 8)) else { return v };
    for i in 0..count as usize {
        let r = off as usize + i * size as usize;
        let (Some(tag), Some(name_id)) = (d.get(r..r + 4), be_u16(d, r + 4)) else { break };
        v.push(([tag[0], tag[1], tag[2], tag[3]], name_id));
    }
    v
}

#[cfg(test)]
pub(crate) mod testutil {
    //! Builds synthetic sfnt files for the validation tests.
    use super::*;

    /// Builds an sfnt from (tag, data) pairs.
    pub fn build_sfnt(version: u32, tables: &[(Tag, Vec<u8>)]) -> Vec<u8> {
        let n = tables.len();
        let mut out = Vec::new();
        out.extend_from_slice(&version.to_be_bytes());
        out.extend_from_slice(&(n as u16).to_be_bytes());
        out.extend_from_slice(&[0u8; 6]);
        let mut offset = 12 + 16 * n;
        let mut body = Vec::new();
        for (tag, data) in tables {
            out.extend_from_slice(tag);
            out.extend_from_slice(&0u32.to_be_bytes());
            out.extend_from_slice(&(offset as u32).to_be_bytes());
            out.extend_from_slice(&(data.len() as u32).to_be_bytes());
            body.extend_from_slice(data);
            while body.len() % 4 != 0 {
                body.push(0);
            }
            offset = 12 + 16 * n + body.len();
        }
        out.extend_from_slice(&body);
        out
    }

    /// A minimal valid `head`.
    pub fn head(upem: u16, glyph_format: i16, magic: u32) -> Vec<u8> {
        let mut h = vec![0u8; 54];
        h[0..4].copy_from_slice(&0x0001_0000u32.to_be_bytes());
        h[12..16].copy_from_slice(&magic.to_be_bytes());
        h[18..20].copy_from_slice(&upem.to_be_bytes());
        h[52..54].copy_from_slice(&glyph_format.to_be_bytes());
        h
    }

    /// `maxp` with a version.
    pub fn maxp(version: u32) -> Vec<u8> {
        let mut m = vec![0u8; if version == 0x5000 { 6 } else { 32 }];
        m[0..4].copy_from_slice(&version.to_be_bytes());
        m[4..6].copy_from_slice(&1u16.to_be_bytes());
        m
    }

    /// `cmap` header only.
    pub fn cmap(version: u16) -> Vec<u8> {
        let mut c = vec![0u8; 4];
        c[0..2].copy_from_slice(&version.to_be_bytes());
        c
    }

    /// `name` table with Windows English records.
    pub fn name(records: &[(u16, &str)]) -> Vec<u8> {
        let mut storage = Vec::new();
        let mut out = Vec::new();
        out.extend_from_slice(&0u16.to_be_bytes());
        out.extend_from_slice(&(records.len() as u16).to_be_bytes());
        out.extend_from_slice(&((6 + 12 * records.len()) as u16).to_be_bytes());
        for (id, s) in records {
            let bytes: Vec<u8> = s.encode_utf16().flat_map(|u| u.to_be_bytes()).collect();
            for v in [3u16, 1, 0x0409, *id, bytes.len() as u16, storage.len() as u16] {
                out.extend_from_slice(&v.to_be_bytes());
            }
            storage.extend_from_slice(&bytes);
        }
        out.extend_from_slice(&storage);
        out
    }

    /// A structurally valid TrueType-flavoured font.
    pub fn valid_tables() -> Vec<(Tag, Vec<u8>)> {
        vec![
            (*b"cmap", cmap(0)),
            (*b"glyf", vec![0; 4]),
            (*b"head", head(1000, 0, 0x5F0F_3CF5)),
            (*b"hhea", vec![0; 36]),
            (*b"hmtx", vec![0; 4]),
            (*b"loca", vec![0; 4]),
            (*b"maxp", maxp(0x0001_0000)),
            (*b"name", name(&[(1, "Test Family"), (2, "Regular")])),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::*;
    use super::*;

    fn replace(tables: &mut [(Tag, Vec<u8>)], tag: &Tag, data: Vec<u8>) {
        tables.iter_mut().find(|(t, _)| t == tag).unwrap().1 = data;
    }

    #[test]
    fn valid_font_passes() {
        let data = build_sfnt(0x0001_0000, &valid_tables());
        let face = SfntFace::open(&data, 0).unwrap();
        let (flavour, warnings) = face.validate().unwrap();
        assert_eq!(flavour, Flavour::TrueType);
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn zero_tables_rejected() {
        let mut data = build_sfnt(0x0001_0000, &valid_tables());
        data[4] = 0;
        data[5] = 0;
        assert_eq!(SfntFace::open(&data, 0).unwrap_err(), ValidationError::NoTables);
    }

    #[test]
    fn bad_signature_and_truncation() {
        let mut data = build_sfnt(0x0001_0000, &valid_tables());
        data[0] = b'X';
        assert!(matches!(SfntFace::open(&data, 0), Err(ValidationError::UnknownSignature(_))));
        let data = build_sfnt(0x0001_0000, &valid_tables());
        assert_eq!(SfntFace::open(&data[..40], 0).unwrap_err(), ValidationError::DirectoryOutOfBounds);
        assert_eq!(SfntFace::open(&data[..2], 0).unwrap_err(), ValidationError::TooShort);
    }

    #[test]
    fn head_rules() {
        let mut t = valid_tables();
        replace(&mut t, b"head", head(1000, 0, 0x1234_5678));
        let d = build_sfnt(0x0001_0000, &t);
        assert_eq!(SfntFace::open(&d, 0).unwrap().validate().unwrap_err(), ValidationError::BadHeadMagic);
        for (upem, ok) in [(15u16, false), (16, true), (16384, true), (16385, false)] {
            let mut t = valid_tables();
            replace(&mut t, b"head", head(upem, 0, 0x5F0F_3CF5));
            let d = build_sfnt(0x0001_0000, &t);
            assert_eq!(SfntFace::open(&d, 0).unwrap().validate().is_ok(), ok, "upem {upem}");
        }
        let mut t = valid_tables();
        replace(&mut t, b"head", head(1000, 1, 0x5F0F_3CF5));
        let d = build_sfnt(0x0001_0000, &t);
        assert_eq!(SfntFace::open(&d, 0).unwrap().validate().unwrap_err(), ValidationError::BadGlyphDataFormat(1));
    }

    #[test]
    fn maxp_is_flavour_discriminator() {
        let mut t = valid_tables();
        replace(&mut t, b"maxp", maxp(0x5000));
        let d = build_sfnt(TAG_OTTO, &t);
        // CFF flavour without a CFF table
        assert_eq!(SfntFace::open(&d, 0).unwrap().validate().unwrap_err(), ValidationError::MissingOutlines("CFF "));
        t.push((*b"CFF ", vec![1, 0, 4, 4]));
        let d = build_sfnt(TAG_OTTO, &t);
        assert_eq!(SfntFace::open(&d, 0).unwrap().validate().unwrap().0, Flavour::Cff);
        let mut t = valid_tables();
        replace(&mut t, b"maxp", maxp(0x0002_0000));
        let d = build_sfnt(0x0001_0000, &t);
        assert!(matches!(SfntFace::open(&d, 0).unwrap().validate(), Err(ValidationError::BadMaxpVersion(_))));
    }

    #[test]
    fn cmap_and_required_tables() {
        let mut t = valid_tables();
        replace(&mut t, b"cmap", cmap(2));
        let d = build_sfnt(0x0001_0000, &t);
        assert_eq!(SfntFace::open(&d, 0).unwrap().validate().unwrap_err(), ValidationError::BadCmapVersion(2));
        for tag in [b"head", b"maxp", b"cmap"] {
            let t: Vec<_> = valid_tables().into_iter().filter(|(x, _)| x != tag).collect();
            let d = build_sfnt(0x0001_0000, &t);
            assert!(matches!(SfntFace::open(&d, 0).unwrap().validate(), Err(ValidationError::MissingTable(_))));
        }
        let t: Vec<_> = valid_tables().into_iter().filter(|(x, _)| x != b"glyf").collect();
        let d = build_sfnt(0x0001_0000, &t);
        assert_eq!(SfntFace::open(&d, 0).unwrap().validate().unwrap_err(), ValidationError::MissingOutlines("glyf"));
        let t: Vec<_> = valid_tables().into_iter().filter(|(x, _)| x != b"hhea").collect();
        let d = build_sfnt(0x0001_0000, &t);
        let (_, w) = SfntFace::open(&d, 0).unwrap().validate().unwrap();
        assert_eq!(w.len(), 1);
    }

    #[test]
    fn quirk_list_rejects_known_bad_families() {
        let mut t = valid_tables();
        replace(&mut t, b"name", name(&[(1, "Nunito VF Beta"), (2, "Regular")]));
        let d = build_sfnt(0x0001_0000, &t);
        assert!(matches!(SfntFace::open(&d, 0).unwrap().validate(), Err(ValidationError::KnownProblemFont(_))));
    }

    #[test]
    fn ttc_face_selection() {
        let face = build_sfnt(0x0001_0000, &valid_tables());
        let mut ttc = Vec::new();
        ttc.extend_from_slice(&TAG_TTCF.to_be_bytes());
        ttc.extend_from_slice(&0x0001_0000u32.to_be_bytes());
        ttc.extend_from_slice(&1u32.to_be_bytes());
        ttc.extend_from_slice(&16u32.to_be_bytes());
        // the offsets inside the embedded face are relative to its own start: rebase them
        let mut f = face.clone();
        let n = be_u16(&f, 4).unwrap() as usize;
        for i in 0..n {
            let r = 12 + 16 * i + 8;
            let off = be_u32(&f, r).unwrap() + 16;
            f[r..r + 4].copy_from_slice(&off.to_be_bytes());
        }
        ttc.extend_from_slice(&f);
        assert_eq!(face_count(&ttc), 1);
        assert!(SfntFace::open(&ttc, 0).unwrap().validate().is_ok());
        assert_eq!(
            SfntFace::open(&ttc, 1).unwrap_err(),
            ValidationError::FaceIndexOutOfRange { index: 1, count: 1 }
        );
    }

    #[test]
    fn name_decoding() {
        let n = NameTable::parse(&name(&[(1, "Fam"), (4, "Fam Bold")]));
        assert_eq!(n.english(1), Some("Fam"));
        assert_eq!(n.english(4), Some("Fam Bold"));
        assert_eq!(decode_mac_roman(&[0x41, 0x80, 0xA9]), "AÄ©");
    }
}
