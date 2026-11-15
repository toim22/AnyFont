//! The font record (one face) and the value types it is made of.
//!
//! [`FontRecord`] is AnyFont's equivalent of one `FONTSDATA` row of the original font database
//! (`05-data-models/file-formats/font-database.md`): identity, names, classification, OS/2 data,
//! OpenType features, variation axes and the library state (installed, protected, duplicate, corrupt).
//! Display helpers that reproduce the captions seen in the Font info panel live in [`crate::format`].

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Database row id of a face record. Stable while the face stays in the catalog.
pub type FontId = i64;
/// Id of a user folder.
pub type FolderId = i64;
/// Id of a collection (Favorites included).
pub type CollectionId = i64;

/// Content-based identity of a face: `sha256(file)` + face index + named-instance index.
///
/// Identical files at several paths are one face (the original keys `FONTSDATA` by `DBShaID` and keeps
/// all locations in `DBPaths`), so a copy of an installed font counts as installed.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub struct FontKey(pub String);

impl FontKey {
    /// Builds the key string `"<sha>:<face>:<instance or ->"`.
    pub fn new(sha: &str, face_index: u32, instance: Option<u16>) -> Self {
        match instance {
            Some(i) => FontKey(format!("{sha}:{face_index}:{i}")),
            None => FontKey(format!("{sha}:{face_index}:-")),
        }
    }
    /// The key as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for FontKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Container format of the file a face comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum FileFormat {
    /// `.ttf` (sfnt version 0x00010000 or `true`).
    #[default]
    TrueType,
    /// `.otf` (sfnt version `OTTO`).
    OpenTypeCff,
    /// `.ttc` collection with TrueType outlines.
    TrueTypeCollection,
    /// `.otc` collection with CFF outlines.
    OpenTypeCollection,
    /// WOFF 1.0 (decoded to sfnt).
    Woff,
    /// WOFF 2.0 (recognised, not decoded: reported as unsupported).
    Woff2,
    /// Type 1 binary (`.pfb`).
    Type1Binary,
    /// Type 1 ASCII (`.pfa`) or a lone `.pfm`/`.afm`.
    Type1Ascii,
    /// Not a recognised font file.
    Unknown,
}

impl FileFormat {
    /// Stable text used in the database.
    pub fn db_name(self) -> &'static str {
        match self {
            FileFormat::TrueType => "ttf",
            FileFormat::OpenTypeCff => "otf",
            FileFormat::TrueTypeCollection => "ttc",
            FileFormat::OpenTypeCollection => "otc",
            FileFormat::Woff => "woff",
            FileFormat::Woff2 => "woff2",
            FileFormat::Type1Binary => "pfb",
            FileFormat::Type1Ascii => "pfa",
            FileFormat::Unknown => "unknown",
        }
    }
    /// Inverse of [`FileFormat::db_name`].
    pub fn from_db_name(s: &str) -> Self {
        match s {
            "ttf" => FileFormat::TrueType,
            "otf" => FileFormat::OpenTypeCff,
            "ttc" => FileFormat::TrueTypeCollection,
            "otc" => FileFormat::OpenTypeCollection,
            "woff" => FileFormat::Woff,
            "woff2" => FileFormat::Woff2,
            "pfb" => FileFormat::Type1Binary,
            "pfa" => FileFormat::Type1Ascii,
            _ => FileFormat::Unknown,
        }
    }
}

/// Font technology, the values of `DBTech` (`CFontDatabase::s_strTrueType` ... in the font engine DLL).
///
/// Rule (Windows convention, matches the screenshots: Arial = "TrueType", Algerian = "OpenType - TrueType"):
/// CFF/CFF2 outlines -> OpenType-PostScript; TrueType outlines with a `DSIG` table -> OpenType-TrueType;
/// TrueType outlines without `DSIG` -> TrueType; Type 1 -> Type1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum Technology {
    /// Plain TrueType.
    #[default]
    TrueType,
    /// OpenType with TrueType outlines.
    OpenTypeTrueType,
    /// OpenType with PostScript (CFF/CFF2) outlines.
    OpenTypePostScript,
    /// Adobe Type 1.
    Type1,
    /// Unreadable file (corrupt records only).
    Unknown,
}

impl Technology {
    /// Value stored in the database (`DBTech`).
    pub fn db_name(self) -> &'static str {
        match self {
            Technology::TrueType => "TrueType",
            Technology::OpenTypeTrueType => "OpenType-TrueType",
            Technology::OpenTypePostScript => "OpenType-PostScript",
            Technology::Type1 => "Type1",
            Technology::Unknown => "Unknown",
        }
    }
    /// Inverse of [`Technology::db_name`].
    pub fn from_db_name(s: &str) -> Self {
        match s {
            "TrueType" => Technology::TrueType,
            "OpenType-TrueType" => Technology::OpenTypeTrueType,
            "OpenType-PostScript" => Technology::OpenTypePostScript,
            "Type1" => Technology::Type1,
            _ => Technology::Unknown,
        }
    }
    /// Caption shown in the Font info tab and the Font Technology filter ("OpenType - TrueType").
    pub fn caption(self) -> &'static str {
        match self {
            Technology::TrueType => "TrueType",
            Technology::OpenTypeTrueType => "OpenType - TrueType",
            Technology::OpenTypePostScript => "OpenType - PostScript",
            Technology::Type1 => "Type 1",
            Technology::Unknown => "Unknown",
        }
    }
    /// True for OpenType flavours (the list shows the "O" icon, otherwise the "TT" icon).
    pub fn is_opentype(self) -> bool {
        matches!(self, Technology::OpenTypeTrueType | Technology::OpenTypePostScript)
    }
}

/// Outline format of the face.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Outline {
    /// `glyf`/`loca`.
    #[default]
    TrueType,
    /// `CFF `.
    Cff,
    /// `CFF2`.
    Cff2,
    /// Bitmap only (`EBDT`/`CBDT`/`sbix`).
    Bitmap,
    /// Type 1 charstrings.
    Type1,
    /// No outlines (corrupt).
    None,
}

/// Colour font formats present (bit set).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct ColorFormats(pub u8);

impl ColorFormats {
    /// `COLR` table.
    pub const COLR: u8 = 1;
    /// `CPAL` table.
    pub const CPAL: u8 = 2;
    /// `SVG ` table.
    pub const SVG: u8 = 4;
    /// `sbix` table.
    pub const SBIX: u8 = 8;
    /// `CBDT`/`CBLC` tables.
    pub const CBDT: u8 = 16;
    /// True if any colour format is present.
    pub fn is_color(self) -> bool {
        self.0 & (Self::COLR | Self::SVG | Self::SBIX | Self::CBDT) != 0
    }
    /// Names of the formats present, e.g. `["COLR", "CPAL"]`.
    pub fn names(self) -> Vec<&'static str> {
        let mut v = Vec::new();
        for (bit, name) in [
            (Self::COLR, "COLR"),
            (Self::CPAL, "CPAL"),
            (Self::SVG, "SVG"),
            (Self::SBIX, "sbix"),
            (Self::CBDT, "CBDT"),
        ] {
            if self.0 & bit != 0 {
                v.push(name);
            }
        }
        v
    }
}

/// Embedding rights derived from `OS/2.fsType` exactly like
/// `CFontInformerOTF::GetFontEmbeddingPermissions` (engine 0x18017CE60). The numeric values are the
/// ones stored in `DBEmbedding` and used by the Embedding Rights filter (`DBEmbedding = %d`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum Embedding {
    /// fsType bit 2 (0x0004): preview & print.
    PreviewAndPrint = 1,
    /// fsType bit 3 (0x0008): editable.
    Editable = 2,
    /// fsType == 0: installable.
    #[default]
    Installable = 3,
    /// fsType bits 0-3 == 0x0002: restricted licence.
    Restricted = 4,
}

impl Embedding {
    /// Decodes `fsType` with the OS/2 table version (versions <= 3 allow combined bits).
    pub fn from_fs_type(fs_type: u16, os2_version: u16) -> Embedding {
        if fs_type == 0 {
            return Embedding::Installable;
        }
        let mut result: Option<Embedding> = None;
        if os2_version <= 3 {
            if fs_type & 0xE == 2 {
                result = Some(Embedding::Restricted);
            }
            if fs_type & 4 != 0 {
                result = Some(Embedding::PreviewAndPrint);
            }
            if fs_type & 8 != 0 {
                result = Some(Embedding::Editable);
            }
        } else if fs_type & 2 != 0 {
            result = Some(Embedding::Restricted);
        } else if fs_type & 4 != 0 {
            result = Some(Embedding::PreviewAndPrint);
        } else if fs_type & 8 != 0 {
            result = Some(Embedding::Editable);
        }
        // The original returns "no value" when only bits 8/9 are set; those fonts are installable
        // (bits 0-3 clear), which is what AnyFont reports.
        result.unwrap_or(Embedding::Installable)
    }
    /// Value stored in the database.
    pub fn db_value(self) -> i64 {
        self as i64
    }
    /// Inverse of [`Embedding::db_value`].
    pub fn from_db_value(v: i64) -> Embedding {
        match v {
            1 => Embedding::PreviewAndPrint,
            2 => Embedding::Editable,
            4 => Embedding::Restricted,
            _ => Embedding::Installable,
        }
    }
    /// Caption ("Editable", "Installable", "Preview and Print", "Restricted").
    pub fn caption(self) -> &'static str {
        match self {
            Embedding::PreviewAndPrint => "Preview and Print",
            Embedding::Editable => "Editable",
            Embedding::Installable => "Installable",
            Embedding::Restricted => "Restricted",
        }
    }
    /// All values, in filter-panel order.
    pub const ALL: [Embedding; 4] =
        [Embedding::Editable, Embedding::Installable, Embedding::PreviewAndPrint, Embedding::Restricted];
}

/// The 10 PANOSE digits (`OS/2.panose`, `DBPanose0..9`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct Panose(pub [u8; 10]);

impl Panose {
    /// Family kind (digit 0): 0 any, 1 no fit, 2 Latin Text, 3 Latin Hand Written, 4 Latin Decorative,
    /// 5 Latin Symbol.
    pub fn family(&self) -> u8 {
        self.0[0]
    }
    /// True when every digit is 0 ("any"), i.e. the font carries no PANOSE information.
    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|&d| d == 0)
    }
}

/// One variation axis of a variable font (`fvar` + names), with the value of this record's instance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct VariationAxis {
    /// Axis tag, e.g. `wght`.
    pub tag: String,
    /// Axis name from the `name` table ("Weight"), or the tag.
    pub name: String,
    /// Minimum value.
    pub min: f32,
    /// Default value.
    pub default: f32,
    /// Maximum value.
    pub max: f32,
    /// Value of this record's named instance (the default value for non-instance records).
    pub value: f32,
    /// `HIDDEN_AXIS` flag.
    pub hidden: bool,
}

/// A named instance of a variable font (`fvar`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct NamedInstance {
    /// Sub-family name ("Light Condensed").
    pub subfamily: String,
    /// PostScript name if the instance defines one.
    pub postscript_name: Option<String>,
    /// Coordinates, one per axis.
    pub coordinates: Vec<f32>,
}

/// Names of the face in one language (Windows LCID), from the `name` table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LocalizedName {
    /// Windows language id (0x0409 = English US, 0x0411 = Japanese ...).
    pub language: u16,
    /// Name ID 1.
    pub family: Option<String>,
    /// Name ID 2.
    pub subfamily: Option<String>,
    /// Name ID 4.
    pub full_name: Option<String>,
    /// Name ID 16.
    pub typographic_family: Option<String>,
    /// Name ID 17.
    pub typographic_subfamily: Option<String>,
}

/// One `(feature, script, language)` triple of the `GSUB`/`GPOS` tables (the original `OPENTYPE` table).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub struct FeatureRecord {
    /// Feature tag (`liga`).
    pub feature: String,
    /// Script tag (`latn`, `DFLT`).
    pub script: String,
    /// Language system tag (`dflt` for the default language system).
    pub language: String,
}

/// Duplicate state, the meaning of `DBDuplicate`: 0 = unique, 1 = the copy shown when duplicates are hidden
/// (`DBDuplicate<2`), 2 = an extra copy (shown only with "Show duplicate fonts").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum DuplicateState {
    /// No other face with the same family/style.
    #[default]
    Unique,
    /// Master copy of a duplicate set.
    Master,
    /// Another copy of the same family/style.
    Copy,
}

impl DuplicateState {
    /// Database value.
    pub fn db_value(self) -> i64 {
        match self {
            DuplicateState::Unique => 0,
            DuplicateState::Master => 1,
            DuplicateState::Copy => 2,
        }
    }
    /// Inverse of [`DuplicateState::db_value`].
    pub fn from_db_value(v: i64) -> Self {
        match v {
            1 => DuplicateState::Master,
            v if v >= 2 => DuplicateState::Copy,
            _ => DuplicateState::Unique,
        }
    }
    /// True for faces that belong to a duplicate set (Duplicates filter: `DBDuplicate != 0`).
    pub fn is_duplicate(self) -> bool {
        self != DuplicateState::Unique
    }
}

/// Download information of a font that exists only in an online catalog (`DBRemoteFont`, `DBAssetURL`,
/// `DBMetadataURL`, `DBProviderName`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RemoteInfo {
    /// URL of the font file.
    pub asset_url: String,
    /// URL of the metadata page.
    pub metadata_url: String,
    /// Provider name ("Google Fonts").
    pub provider: String,
}

/// One face of a font file with everything the UI and the filters need.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct FontRecord {
    // ---- identity -------------------------------------------------------------------------------
    /// Row id (0 until stored).
    pub id: FontId,
    /// Content identity.
    pub key: FontKey,
    /// SHA-256 of the file content (hex).
    pub sha: String,
    /// Face index inside a collection file (0 for single fonts).
    pub face_index: u32,
    /// Number of faces in the file.
    pub face_count: u32,
    /// Index of the `fvar` named instance this record represents.
    pub instance_index: Option<u16>,
    /// Every location of this file (installed copy, folder copies ...). The first one is the primary path.
    pub paths: Vec<PathBuf>,
    /// File size in bytes.
    pub file_size: u64,
    /// File modification time, seconds since the Unix epoch.
    pub modified: i64,
    /// Container format.
    pub format: FileFormat,

    // ---- names (English / Latin) ------------------------------------------------------------------
    /// Legacy family (name ID 1), English.
    pub family: String,
    /// Legacy sub-family (name ID 2) - or the named instance's sub-family.
    pub subfamily: String,
    /// Typographic family (name ID 16).
    pub typographic_family: Option<String>,
    /// Typographic sub-family (name ID 17).
    pub typographic_subfamily: Option<String>,
    /// Full name (name ID 4).
    pub full_name: String,
    /// PostScript name (name ID 6).
    pub postscript_name: String,
    /// Version string (name ID 5), e.g. "Version 1.57".
    pub version: String,
    /// Unique identifier (name ID 3).
    pub unique_id: String,
    /// Copyright (name ID 0).
    pub copyright: String,
    /// Trademark (name ID 7).
    pub trademark: String,
    /// Manufacturer / foundry (name ID 8).
    pub manufacturer: String,
    /// Designer (name ID 9).
    pub designer: String,
    /// Description (name ID 10).
    pub description: String,
    /// Vendor URL (name ID 11).
    pub vendor_url: String,
    /// Designer URL (name ID 12).
    pub designer_url: String,
    /// Licence (name ID 13).
    pub license: String,
    /// Licence URL (name ID 14).
    pub license_url: String,
    /// Sample text (name ID 19).
    pub sample_text: String,
    /// Names in other languages (non-English Windows records).
    pub localized_names: Vec<LocalizedName>,

    // ---- classification ----------------------------------------------------------------------------
    /// Technology (`DBTech`).
    pub technology: Technology,
    /// Outline format.
    pub outline: Outline,
    /// Variable font (`fvar` present, `DBVariable`).
    pub is_variable: bool,
    /// Colour formats.
    pub color: ColorFormats,
    /// `OS/2.usWeightClass` (or the instance's `wght`).
    pub weight_class: u16,
    /// `OS/2.usWidthClass` 1..9 (or derived from the instance's `wdth`).
    pub width_class: u16,
    /// Italic (fsSelection bit 0, macStyle bit 1, or the instance's `ital`/`slnt`).
    pub italic: bool,
    /// Oblique (fsSelection bit 9).
    pub oblique: bool,
    /// Raw `OS/2.fsSelection`.
    pub fs_selection: u16,
    /// Raw `head.macStyle`.
    pub mac_style: u16,
    /// Raw `OS/2.fsType`.
    pub fs_type: u16,
    /// Embedding rights (`DBEmbedding`).
    pub embedding: Embedding,
    /// `OS/2.ulCodePageRange1/2`.
    pub code_page_range: [u32; 2],
    /// `OS/2.ulUnicodeRange1..4`.
    pub unicode_range: [u32; 4],
    /// PANOSE (`DBPanose0..9`).
    pub panose: Panose,
    /// `OS/2.achVendID`, trimmed.
    pub vendor_id: String,
    /// Symbol font (Windows symbol cmap 3/0, `DBSymbol`).
    pub symbol: bool,
    /// Supports vertical text (`vert`/`vrt2` feature or `vhea`, `DBVertical`).
    pub vertical: bool,
    /// Fixed pitch (`post.isFixedPitch`).
    pub monospaced: bool,
    /// Number of glyphs.
    pub glyph_count: u32,
    /// `head.unitsPerEm`.
    pub units_per_em: u16,
    /// Distinct OpenType feature tags (GSUB + GPOS), sorted.
    pub features: Vec<String>,
    /// Feature/script/language triples.
    pub feature_records: Vec<FeatureRecord>,
    /// Variation axes with this instance's values.
    pub axes: Vec<VariationAxis>,
    /// All named instances of the file's face.
    pub instances: Vec<NamedInstance>,
    /// Non-fatal validation findings (missing `hhea`, bad checksums ...).
    pub warnings: Vec<String>,

    // ---- library state ----------------------------------------------------------------------------
    /// Installed in Windows (one of [`FontRecord::paths`] is in the installed set given by the caller).
    pub installed: bool,
    /// Protected system font (caller supplied; cannot be uninstalled or deleted).
    pub protected: bool,
    /// Duplicate state.
    pub duplicate: DuplicateState,
    /// Why the file could not be read; `None` for good fonts.
    pub corrupt: Option<String>,
    /// Online-only font (Download).
    pub remote: Option<RemoteInfo>,
    /// Collections this face belongs to (filled by the catalog).
    pub collections: Vec<CollectionId>,
}

impl FontRecord {
    /// Family used to group the list (typographic family, else legacy family).
    pub fn group_family(&self) -> &str {
        match &self.typographic_family {
            Some(f) if !f.is_empty() => f,
            _ => &self.family,
        }
    }
    /// Primary file path (first location).
    pub fn path(&self) -> Option<&std::path::Path> {
        self.paths.first().map(|p| p.as_path())
    }
    /// True when the face can be rendered/installed (not corrupt, has a local file).
    pub fn is_usable(&self) -> bool {
        self.corrupt.is_none() && !self.paths.is_empty()
    }
    /// True for an online-only font that still has to be downloaded.
    pub fn needs_download(&self) -> bool {
        self.remote.is_some() && self.paths.is_empty()
    }
    /// Localised names for `language` (exact LCID, then same primary language).
    pub fn localized(&self, language: u16) -> Option<&LocalizedName> {
        self.localized_names
            .iter()
            .find(|n| n.language == language)
            .or_else(|| self.localized_names.iter().find(|n| n.language & 0x3FF == language & 0x3FF))
    }
    /// Family shown in the list. With `show_latin` (View > Show Latin names) or when no localised name
    /// exists for `language`, the English name is used.
    pub fn display_family(&self, language: u16, show_latin: bool) -> String {
        if !show_latin {
            if let Some(n) = self.localized(language) {
                if let Some(f) = n.typographic_family.as_ref().or(n.family.as_ref()) {
                    return f.clone();
                }
            }
        }
        self.group_family().to_string()
    }
    /// Legacy family shown in face labels, honouring Show Latin names.
    pub fn display_legacy_family(&self, language: u16, show_latin: bool) -> String {
        if !show_latin {
            if let Some(f) = self.localized(language).and_then(|n| n.family.clone()) {
                return f;
            }
        }
        self.family.clone()
    }
    /// The face label of the font list, e.g. `Arial (Regular, Normal)`, `Arial (Bold-Italic)`,
    /// `Arial Black` (see [`crate::format::face_label`]).
    pub fn label(&self, language: u16, show_latin: bool) -> String {
        crate::format::face_label(self, language, show_latin)
    }
    /// Weight bucket used by the original filters (the weight-bucket column): weight class rounded to 100..900.
    pub fn weight_bucket(&self) -> u16 {
        let w = self.weight_class.clamp(1, 1000);
        (((w + 50) / 100) * 100).clamp(100, 900)
    }
}

/// An installed font file supplied by the Windows layer (`anyfont-sys`): the content of "My fonts".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledFont {
    /// Font file path (for Type 1 the `.pfm` or `.pfb`).
    pub path: PathBuf,
    /// Protected system font (lock icon; Uninstall and Delete disabled).
    pub protected: bool,
}

impl InstalledFont {
    /// Convenience constructor.
    pub fn new(path: impl Into<PathBuf>, protected: bool) -> Self {
        InstalledFont { path: path.into(), protected }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedding_from_fs_type() {
        assert_eq!(Embedding::from_fs_type(0, 4), Embedding::Installable);
        assert_eq!(Embedding::from_fs_type(2, 4), Embedding::Restricted);
        assert_eq!(Embedding::from_fs_type(4, 4), Embedding::PreviewAndPrint);
        assert_eq!(Embedding::from_fs_type(8, 4), Embedding::Editable);
        assert_eq!(Embedding::from_fs_type(0x0100, 4), Embedding::Installable, "only bit 8 set");
        // version <= 3: the last matching bit wins (combined bits allowed)
        assert_eq!(Embedding::from_fs_type(4 | 8, 3), Embedding::Editable);
        assert_eq!(Embedding::from_fs_type(2 | 8, 4), Embedding::Restricted);
        for e in Embedding::ALL {
            assert_eq!(Embedding::from_db_value(e.db_value()), e);
        }
    }

    #[test]
    fn latin_names_and_weight_bucket() {
        let f = FontRecord {
            family: "MS Gothic".into(),
            localized_names: vec![LocalizedName {
                language: 0x0411,
                family: Some("ＭＳ ゴシック".into()),
                ..Default::default()
            }],
            weight_class: 350,
            ..Default::default()
        };
        assert_eq!(f.display_family(0x0411, false), "ＭＳ ゴシック");
        assert_eq!(f.display_family(0x0411, true), "MS Gothic", "Show Latin names");
        assert_eq!(f.display_family(0x0409, false), "MS Gothic");
        assert_eq!(f.weight_bucket(), 400);
    }
}
