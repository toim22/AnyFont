//! The Filters panel: groups, options and their predicates.
//!
//! Every option reproduces the SQL predicate the original builds in the font engine DLL (filter collections
//! registered by `sub_1800DDB60`, ids `FC_Status`, `FC_EmbeddingRights`, `FC_Technology`, `FC_Weight`,
//! `FC_Width`, `FC_Style`, `FC_Language`, `FC_OpenType`):
//!
//! | Group | Option -> predicate (evidence) |
//! |---|---|
//! | Font Status | Installed `DBInstalled = 1 AND DBSystem = 0`; Not installed `DBInstalled = 0`; Protected System Fonts `DBSystem = 1`; Duplicates `DBDuplicate != 0`; Corrupt (0x180113B50, 0x180113A60) |
//! | Embedding Rights | `DBEmbedding = n` (0x1801090B0), values of `GetFontEmbeddingPermissions` |
//! | Font Technology | `DBTech IN (...)` (0x180113560), Variable Fonts `DBVariable = 1` (0x180115360) |
//! | Weight | Light weight bucket `IN (100,200,300)`, Regular `(400,500)`, Bold `(600,700,800,900)` (0x180108600) |
//! | Width | Condensed `DBWidth IN (1,2,3)`, Normal `(4,5,6)`, Expanded `(7,8,9)` (0x180108A90) |
//! | Style | PANOSE sets `(DBPanose%d = %d)` (0x1801096E0, table in 0x1800A5970) |
//! | Character Range | code-page / unicode-range bits per language (0x180112320, `CGlbLang`) |
//! | OpenType | `DBFeature='tag' OR ...` per feature group (0x1801153B0, groups in 0x180116770) |
//!
//! Combination: options of one group are OR-ed, groups are AND-ed (each group contributes one parenthesised
//! `OR` clause to the `WHERE`). Only options matched by at least one font of the database are offered
//! (`SELECT DISTINCT ...` / `... LIMIT 1` probes), except the four Font Status options which are always shown.

use crate::model::{Embedding, FontRecord, Technology};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Filter group (collapsible section of the Filters panel), in panel order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum FilterGroup {
    /// Font Status.
    FontStatus,
    /// Embedding Rights.
    EmbeddingRights,
    /// Font Technology.
    FontTechnology,
    /// Weight.
    Weight,
    /// Width.
    Width,
    /// Style.
    Style,
    /// Character Range.
    CharacterRange,
    /// OpenType.
    OpenType,
}

impl FilterGroup {
    /// All groups in panel order.
    pub const ALL: [FilterGroup; 8] = [
        FilterGroup::FontStatus,
        FilterGroup::EmbeddingRights,
        FilterGroup::FontTechnology,
        FilterGroup::Weight,
        FilterGroup::Width,
        FilterGroup::Style,
        FilterGroup::CharacterRange,
        FilterGroup::OpenType,
    ];
    /// Group caption.
    pub fn caption(self) -> &'static str {
        match self {
            FilterGroup::FontStatus => "Font Status",
            FilterGroup::EmbeddingRights => "Embedding Rights",
            FilterGroup::FontTechnology => "Font Technology",
            FilterGroup::Weight => "Weight",
            FilterGroup::Width => "Width",
            FilterGroup::Style => "Style",
            FilterGroup::CharacterRange => "Character Range",
            FilterGroup::OpenType => "OpenType",
        }
    }
}

/// Font Status options.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum StatusFilter {
    /// Installed and not protected.
    Installed,
    /// Not installed.
    NotInstalled,
    /// Protected system fonts.
    ProtectedSystemFonts,
    /// Member of a duplicate set.
    Duplicates,
    /// File could not be read (shown only when such files exist).
    Corrupt,
}

/// Font Technology options.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TechnologyFilter {
    /// OpenType - PostScript.
    OpenTypePostScript,
    /// OpenType - TrueType.
    OpenTypeTrueType,
    /// TrueType.
    TrueType,
    /// Type 1.
    Type1,
    /// Variable Fonts.
    Variable,
}

/// Weight options.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum WeightFilter {
    /// 100-300.
    Light,
    /// 400-500.
    Regular,
    /// 600-900.
    Bold,
}

/// Width options.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum WidthFilter {
    /// Width class 1-3.
    Condensed,
    /// Width class 4-6.
    Normal,
    /// Width class 7-9.
    Expanded,
}

/// Style options (PANOSE classification).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum StyleCategory {
    /// PANOSE family 4 (Latin Decorative).
    Decorative,
    /// PANOSE family 2 (Latin Text) - the original's "Latin Text|Display" entry.
    Display,
    /// Latin Text proportion 9, Hand Written spacing 3, Decorative aspect 9 (+ `post.isFixedPitch`).
    Monospaced,
    /// Latin Text with serif style 11..15.
    SansSerif,
    /// PANOSE family 3 (Latin Hand Written).
    Script,
    /// Latin Text with serif style 2..10.
    Serif,
    /// PANOSE family 5 (Latin Symbol) or a symbol cmap.
    Symbol,
}

/// Character Range options (languages of `CGlbLang`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CharRange {
    /// Arabic.
    Arabic,
    /// Chinese Simplified.
    ChineseSimplified,
    /// Chinese Traditional.
    ChineseTraditional,
    /// Cyrillic.
    Cyrillic,
    /// Devanagari.
    Devanagari,
    /// Greek.
    Greek,
    /// Hebrew.
    Hebrew,
    /// Japanese.
    Japanese,
    /// Korean.
    Korean,
    /// Latin.
    Latin,
    /// Turkish.
    Turkish,
}

/// OpenType feature groups of the OpenType filter (order of `sub_180116770`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum OpenTypeGroup {
    /// nalt.
    AlternateAnnotationForms,
    /// smpl trad jp78 jp83 jp90 jp04 expt hojo nlck tnam.
    AsianForms,
    /// valt vhal.
    AsianVerticalMetrics,
    /// fwid hwid halt pwid twid pkna cpct.
    AsianWidths,
    /// cpsp.
    CapitalSpacing,
    /// cpsp titl smcp c2sc.
    Capitals,
    /// case.
    CaseSensitiveForms,
    /// calt.
    ContextualAlternates,
    /// clig.
    ContextualLigatures,
    /// dlig.
    DiscretionaryLigatures,
    /// numr dnom frac afrc.
    Fraction,
    /// hist.
    HistoricalForms,
    /// hkna.
    HorizontalKanaAlternates,
    /// hlig.
    HistoricalLigatures,
    /// pnum lnum onum tnum.
    NumberStyles,
    /// ordn.
    Ordinals,
    /// ornm.
    Ornaments,
    /// sups subs.
    Position,
    /// zero.
    SlashedZero,
    /// liga.
    StandardLigatures,
    /// salt.
    StylisticAlternate,
    /// ss01..ss20.
    StylisticSets,
    /// swsh.
    SwashVariants,
    /// vrt2.
    VerticalAlternatesAndRotation,
}

const SS_TAGS: [&str; 20] = [
    "ss01", "ss02", "ss03", "ss04", "ss05", "ss06", "ss07", "ss08", "ss09", "ss10", "ss11", "ss12", "ss13", "ss14",
    "ss15", "ss16", "ss17", "ss18", "ss19", "ss20",
];

impl OpenTypeGroup {
    /// All groups in panel order.
    pub const ALL: [OpenTypeGroup; 24] = [
        OpenTypeGroup::AlternateAnnotationForms,
        OpenTypeGroup::AsianForms,
        OpenTypeGroup::AsianVerticalMetrics,
        OpenTypeGroup::AsianWidths,
        OpenTypeGroup::CapitalSpacing,
        OpenTypeGroup::Capitals,
        OpenTypeGroup::CaseSensitiveForms,
        OpenTypeGroup::ContextualAlternates,
        OpenTypeGroup::ContextualLigatures,
        OpenTypeGroup::DiscretionaryLigatures,
        OpenTypeGroup::Fraction,
        OpenTypeGroup::HistoricalForms,
        OpenTypeGroup::HorizontalKanaAlternates,
        OpenTypeGroup::HistoricalLigatures,
        OpenTypeGroup::NumberStyles,
        OpenTypeGroup::Ordinals,
        OpenTypeGroup::Ornaments,
        OpenTypeGroup::Position,
        OpenTypeGroup::SlashedZero,
        OpenTypeGroup::StandardLigatures,
        OpenTypeGroup::StylisticAlternate,
        OpenTypeGroup::StylisticSets,
        OpenTypeGroup::SwashVariants,
        OpenTypeGroup::VerticalAlternatesAndRotation,
    ];

    /// Feature tags of the group.
    pub fn tags(self) -> &'static [&'static str] {
        match self {
            OpenTypeGroup::AlternateAnnotationForms => &["nalt"],
            OpenTypeGroup::AsianForms => &["smpl", "trad", "jp78", "jp83", "jp90", "jp04", "expt", "hojo", "nlck", "tnam"],
            OpenTypeGroup::AsianVerticalMetrics => &["valt", "vhal"],
            OpenTypeGroup::AsianWidths => &["fwid", "hwid", "halt", "pwid", "twid", "pkna", "cpct"],
            OpenTypeGroup::CapitalSpacing => &["cpsp"],
            OpenTypeGroup::Capitals => &["cpsp", "titl", "smcp", "c2sc"],
            OpenTypeGroup::CaseSensitiveForms => &["case"],
            OpenTypeGroup::ContextualAlternates => &["calt"],
            OpenTypeGroup::ContextualLigatures => &["clig"],
            OpenTypeGroup::DiscretionaryLigatures => &["dlig"],
            OpenTypeGroup::Fraction => &["numr", "dnom", "frac", "afrc"],
            OpenTypeGroup::HistoricalForms => &["hist"],
            OpenTypeGroup::HorizontalKanaAlternates => &["hkna"],
            OpenTypeGroup::HistoricalLigatures => &["hlig"],
            OpenTypeGroup::NumberStyles => &["pnum", "lnum", "onum", "tnum"],
            OpenTypeGroup::Ordinals => &["ordn"],
            OpenTypeGroup::Ornaments => &["ornm"],
            OpenTypeGroup::Position => &["sups", "subs"],
            OpenTypeGroup::SlashedZero => &["zero"],
            OpenTypeGroup::StandardLigatures => &["liga"],
            OpenTypeGroup::StylisticAlternate => &["salt"],
            OpenTypeGroup::StylisticSets => &SS_TAGS,
            OpenTypeGroup::SwashVariants => &["swsh"],
            OpenTypeGroup::VerticalAlternatesAndRotation => &["vrt2"],
        }
    }

    /// Caption.
    pub fn caption(self) -> &'static str {
        match self {
            OpenTypeGroup::AlternateAnnotationForms => "Alternate Annotation Forms",
            OpenTypeGroup::AsianForms => "Asian Forms",
            OpenTypeGroup::AsianVerticalMetrics => "Asian Vertical Metrics",
            OpenTypeGroup::AsianWidths => "Asian Widths",
            OpenTypeGroup::CapitalSpacing => "Capital Spacing",
            OpenTypeGroup::Capitals => "Capitals",
            OpenTypeGroup::CaseSensitiveForms => "Case-Sensitive Forms",
            OpenTypeGroup::ContextualAlternates => "Contextual Alternates",
            OpenTypeGroup::ContextualLigatures => "Contextual Ligatures",
            OpenTypeGroup::DiscretionaryLigatures => "Discretionary Ligatures",
            OpenTypeGroup::Fraction => "Fraction",
            OpenTypeGroup::HistoricalForms => "Historical Forms",
            OpenTypeGroup::HorizontalKanaAlternates => "Horizontal Kana Alternates",
            OpenTypeGroup::HistoricalLigatures => "Historical Ligatures",
            OpenTypeGroup::NumberStyles => "Number Styles",
            OpenTypeGroup::Ordinals => "Ordinals",
            OpenTypeGroup::Ornaments => "Ornaments",
            OpenTypeGroup::Position => "Position",
            OpenTypeGroup::SlashedZero => "Slashed Zero",
            OpenTypeGroup::StandardLigatures => "Standard Ligatures",
            OpenTypeGroup::StylisticAlternate => "Stylistic Alternate",
            OpenTypeGroup::StylisticSets => "Stylistic Sets",
            OpenTypeGroup::SwashVariants => "Swash Variants",
            OpenTypeGroup::VerticalAlternatesAndRotation => "Vertical Alternates and Rotation",
        }
    }
}

/// One checkbox of the Filters panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum FilterOption {
    /// Font Status.
    Status(StatusFilter),
    /// Embedding Rights.
    Embedding(Embedding),
    /// Font Technology.
    Technology(TechnologyFilter),
    /// Weight.
    Weight(WeightFilter),
    /// Width.
    Width(WidthFilter),
    /// Style.
    Style(StyleCategory),
    /// Character Range.
    CharRange(CharRange),
    /// OpenType.
    OpenType(OpenTypeGroup),
}

fn bit(range: &[u32], n: u32) -> bool {
    range.get((n / 32) as usize).map(|w| w & (1 << (n % 32)) != 0).unwrap_or(false)
}

impl CharRange {
    /// All options, panel order.
    pub const ALL: [CharRange; 11] = [
        CharRange::Arabic,
        CharRange::ChineseSimplified,
        CharRange::ChineseTraditional,
        CharRange::Cyrillic,
        CharRange::Devanagari,
        CharRange::Greek,
        CharRange::Hebrew,
        CharRange::Japanese,
        CharRange::Korean,
        CharRange::Latin,
        CharRange::Turkish,
    ];
    /// Caption.
    pub fn caption(self) -> &'static str {
        match self {
            CharRange::Arabic => "Arabic",
            CharRange::ChineseSimplified => "Chinese Simplified",
            CharRange::ChineseTraditional => "Chinese Traditional",
            CharRange::Cyrillic => "Cyrillic",
            CharRange::Devanagari => "Devanagari",
            CharRange::Greek => "Greek",
            CharRange::Hebrew => "Hebrew",
            CharRange::Japanese => "Japanese",
            CharRange::Korean => "Korean",
            CharRange::Latin => "Latin",
            CharRange::Turkish => "Turkish",
        }
    }
    /// True when the font covers the range: code-page bits first; unicode-range bits when the font has no
    /// code-page information, and always for Devanagari (no Windows code page).
    pub fn matches(self, cp: &[u32; 2], ur: &[u32; 4]) -> bool {
        let no_cp = cp[0] == 0 && cp[1] == 0;
        let c = |b| bit(cp, b);
        let u = |b| bit(ur, b);
        match self {
            CharRange::Arabic => c(6) || (no_cp && u(13)),
            CharRange::ChineseSimplified => c(18) || (no_cp && u(59) && !u(49)),
            CharRange::ChineseTraditional => c(20) || (no_cp && u(59) && !u(49)),
            CharRange::Cyrillic => c(2) || (no_cp && u(9)),
            CharRange::Devanagari => u(15),
            CharRange::Greek => c(3) || (no_cp && u(7)),
            CharRange::Hebrew => c(5) || (no_cp && u(11)),
            CharRange::Japanese => c(17) || (no_cp && (u(49) || u(50))),
            CharRange::Korean => c(19) || c(21) || (no_cp && (u(56) || u(28))),
            CharRange::Latin => c(0) || c(1) || (no_cp && (u(0) || u(1))),
            CharRange::Turkish => c(4),
        }
    }
}

impl StyleCategory {
    /// All options, panel order.
    pub const ALL: [StyleCategory; 7] = [
        StyleCategory::Decorative,
        StyleCategory::Display,
        StyleCategory::Monospaced,
        StyleCategory::SansSerif,
        StyleCategory::Script,
        StyleCategory::Serif,
        StyleCategory::Symbol,
    ];
    /// Caption.
    pub fn caption(self) -> &'static str {
        match self {
            StyleCategory::Decorative => "Decorative",
            StyleCategory::Display => "Display",
            StyleCategory::Monospaced => "Monospaced",
            StyleCategory::SansSerif => "Sans Serif",
            StyleCategory::Script => "Script",
            StyleCategory::Serif => "Serif",
            StyleCategory::Symbol => "Symbol",
        }
    }
    /// PANOSE predicate (+ the AnyFont additions noted on the variants).
    pub fn matches(self, f: &FontRecord) -> bool {
        let p = &f.panose.0;
        match self {
            StyleCategory::Decorative => p[0] == 4,
            StyleCategory::Display => p[0] == 2,
            StyleCategory::Monospaced => {
                (p[0] == 2 && p[3] == 9) || (p[0] == 3 && p[3] == 3) || (p[0] == 4 && p[3] == 9) || f.monospaced
            }
            StyleCategory::SansSerif => p[0] == 2 && (11..=15).contains(&p[1]),
            StyleCategory::Script => p[0] == 3,
            StyleCategory::Serif => p[0] == 2 && (2..=10).contains(&p[1]),
            StyleCategory::Symbol => p[0] == 5 || f.symbol,
        }
    }
}

impl FilterOption {
    /// Group of the option.
    pub fn group(&self) -> FilterGroup {
        match self {
            FilterOption::Status(_) => FilterGroup::FontStatus,
            FilterOption::Embedding(_) => FilterGroup::EmbeddingRights,
            FilterOption::Technology(_) => FilterGroup::FontTechnology,
            FilterOption::Weight(_) => FilterGroup::Weight,
            FilterOption::Width(_) => FilterGroup::Width,
            FilterOption::Style(_) => FilterGroup::Style,
            FilterOption::CharRange(_) => FilterGroup::CharacterRange,
            FilterOption::OpenType(_) => FilterGroup::OpenType,
        }
    }

    /// Checkbox caption, as in the screenshots.
    pub fn caption(&self) -> &'static str {
        match self {
            FilterOption::Status(s) => match s {
                StatusFilter::Installed => "Installed",
                StatusFilter::NotInstalled => "Not installed",
                StatusFilter::ProtectedSystemFonts => "Protected System Fonts",
                StatusFilter::Duplicates => "Duplicates",
                StatusFilter::Corrupt => "Corrupt",
            },
            FilterOption::Embedding(e) => e.caption(),
            FilterOption::Technology(t) => match t {
                TechnologyFilter::OpenTypePostScript => "OpenType - PostScript",
                TechnologyFilter::OpenTypeTrueType => "OpenType - TrueType",
                TechnologyFilter::TrueType => "TrueType",
                TechnologyFilter::Type1 => "Type 1",
                TechnologyFilter::Variable => "Variable Fonts",
            },
            FilterOption::Weight(w) => match w {
                WeightFilter::Light => "Light",
                WeightFilter::Regular => "Regular",
                WeightFilter::Bold => "Bold",
            },
            FilterOption::Width(w) => match w {
                WidthFilter::Condensed => "Condensed",
                WidthFilter::Normal => "Normal",
                WidthFilter::Expanded => "Expanded",
            },
            FilterOption::Style(s) => s.caption(),
            FilterOption::CharRange(c) => c.caption(),
            FilterOption::OpenType(o) => o.caption(),
        }
    }

    /// Predicate on one font.
    pub fn matches(&self, f: &FontRecord) -> bool {
        match self {
            FilterOption::Status(s) => match s {
                StatusFilter::Installed => f.installed && !f.protected,
                StatusFilter::NotInstalled => !f.installed,
                StatusFilter::ProtectedSystemFonts => f.protected,
                StatusFilter::Duplicates => f.duplicate.is_duplicate(),
                StatusFilter::Corrupt => f.corrupt.is_some(),
            },
            FilterOption::Embedding(e) => f.corrupt.is_none() && f.embedding == *e,
            FilterOption::Technology(t) => match t {
                TechnologyFilter::OpenTypePostScript => f.technology == Technology::OpenTypePostScript,
                TechnologyFilter::OpenTypeTrueType => f.technology == Technology::OpenTypeTrueType,
                TechnologyFilter::TrueType => f.technology == Technology::TrueType,
                TechnologyFilter::Type1 => f.technology == Technology::Type1,
                TechnologyFilter::Variable => f.is_variable,
            },
            FilterOption::Weight(w) => {
                let cw = f.weight_bucket();
                match w {
                    WeightFilter::Light => (100..=300).contains(&cw),
                    WeightFilter::Regular => (400..=500).contains(&cw),
                    WeightFilter::Bold => (600..=900).contains(&cw),
                }
            }
            FilterOption::Width(w) => match w {
                WidthFilter::Condensed => (1..=3).contains(&f.width_class),
                WidthFilter::Normal => (4..=6).contains(&f.width_class),
                WidthFilter::Expanded => (7..=9).contains(&f.width_class),
            },
            FilterOption::Style(s) => f.corrupt.is_none() && s.matches(f),
            FilterOption::CharRange(c) => c.matches(&f.code_page_range, &f.unicode_range),
            FilterOption::OpenType(o) => o.tags().iter().any(|t| f.features.binary_search_by(|x| x.as_str().cmp(t)).is_ok()),
        }
    }

    /// Every option the engine knows, in panel order.
    pub fn all() -> Vec<FilterOption> {
        let mut v = vec![
            FilterOption::Status(StatusFilter::Installed),
            FilterOption::Status(StatusFilter::NotInstalled),
            FilterOption::Status(StatusFilter::ProtectedSystemFonts),
            FilterOption::Status(StatusFilter::Duplicates),
            FilterOption::Status(StatusFilter::Corrupt),
        ];
        v.extend(Embedding::ALL.iter().map(|&e| FilterOption::Embedding(e)));
        v.extend(
            [
                TechnologyFilter::OpenTypePostScript,
                TechnologyFilter::OpenTypeTrueType,
                TechnologyFilter::TrueType,
                TechnologyFilter::Type1,
                TechnologyFilter::Variable,
            ]
            .map(FilterOption::Technology),
        );
        v.extend([WeightFilter::Light, WeightFilter::Regular, WeightFilter::Bold].map(FilterOption::Weight));
        v.extend([WidthFilter::Condensed, WidthFilter::Normal, WidthFilter::Expanded].map(FilterOption::Width));
        v.extend(StyleCategory::ALL.map(FilterOption::Style));
        v.extend(CharRange::ALL.map(FilterOption::CharRange));
        v.extend(OpenTypeGroup::ALL.map(FilterOption::OpenType));
        v
    }
}

/// The checked options. OR inside a group, AND across groups.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilterSet {
    /// Checked options.
    pub selected: BTreeSet<FilterOption>,
}

impl FilterSet {
    /// Empty set (no filtering).
    pub fn new() -> Self {
        Self::default()
    }
    /// Builder: adds an option.
    pub fn with(mut self, o: FilterOption) -> Self {
        self.selected.insert(o);
        self
    }
    /// Checks or unchecks an option.
    pub fn set(&mut self, o: FilterOption, checked: bool) {
        if checked {
            self.selected.insert(o);
        } else {
            self.selected.remove(&o);
        }
    }
    /// Toggles an option.
    pub fn toggle(&mut self, o: FilterOption) {
        if !self.selected.remove(&o) {
            self.selected.insert(o);
        }
    }
    /// True when the option is checked.
    pub fn is_checked(&self, o: &FilterOption) -> bool {
        self.selected.contains(o)
    }
    /// True when nothing is checked.
    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }
    /// Unchecks everything (the funnel button of the panel).
    pub fn clear(&mut self) {
        self.selected.clear();
    }
    /// Unchecks the options of one group.
    pub fn clear_group(&mut self, g: FilterGroup) {
        self.selected.retain(|o| o.group() != g);
    }
    /// True when the Corrupt status option is checked (corrupt files are listed only then).
    pub fn wants_corrupt(&self) -> bool {
        self.selected.contains(&FilterOption::Status(StatusFilter::Corrupt))
    }
    /// The predicate: for each group with checked options, at least one of them must match.
    pub fn matches(&self, f: &FontRecord) -> bool {
        let mut current: Option<FilterGroup> = None;
        let mut group_ok = true;
        for o in &self.selected {
            let g = o.group();
            if current != Some(g) {
                if !group_ok {
                    return false;
                }
                current = Some(g);
                group_ok = false;
            }
            if !group_ok && o.matches(f) {
                group_ok = true;
            }
        }
        group_ok
    }
}

/// The options to show for one group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilterGroupOptions {
    /// Group.
    pub group: FilterGroup,
    /// Options, panel order.
    pub options: Vec<FilterOption>,
}

/// Options offered by the panel for a set of fonts: the four Font Status options always, "Corrupt" only
/// when corrupt files exist, every other option only when at least one font matches it.
pub fn available_options<'a>(fonts: impl IntoIterator<Item = &'a FontRecord>) -> Vec<FilterGroupOptions> {
    let fonts: Vec<&FontRecord> = fonts.into_iter().collect();
    let mut out: Vec<FilterGroupOptions> =
        FilterGroup::ALL.iter().map(|&g| FilterGroupOptions { group: g, options: Vec::new() }).collect();
    for o in FilterOption::all() {
        let always = matches!(
            o,
            FilterOption::Status(
                StatusFilter::Installed
                    | StatusFilter::NotInstalled
                    | StatusFilter::ProtectedSystemFonts
                    | StatusFilter::Duplicates
            )
        );
        let present = always || fonts.iter().any(|f| (f.corrupt.is_none() || o == FilterOption::Status(StatusFilter::Corrupt)) && o.matches(f));
        if present {
            let idx = FilterGroup::ALL.iter().position(|&g| g == o.group()).unwrap_or(0);
            out[idx].options.push(o);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn font(weight: u16, width: u16, tech: Technology, panose: [u8; 10], features: &[&str]) -> FontRecord {
        FontRecord {
            weight_class: weight,
            width_class: width,
            technology: tech,
            panose: Panose(panose),
            features: {
                let mut v: Vec<String> = features.iter().map(|s| s.to_string()).collect();
                v.sort();
                v
            },
            installed: true,
            code_page_range: [1, 0],
            ..Default::default()
        }
    }

    #[test]
    fn or_within_group_and_across_groups() {
        let light = font(300, 5, Technology::TrueType, [2, 11, 0, 0, 0, 0, 0, 0, 0, 0], &["liga"]);
        let bold = font(700, 3, Technology::OpenTypeTrueType, [2, 2, 0, 9, 0, 0, 0, 0, 0, 0], &["smcp"]);
        let reg = font(400, 5, Technology::TrueType, [3, 0, 0, 3, 0, 0, 0, 0, 0, 0], &[]);
        let lb = FilterSet::new()
            .with(FilterOption::Weight(WeightFilter::Light))
            .with(FilterOption::Weight(WeightFilter::Bold));
        assert!(lb.matches(&light) && lb.matches(&bold) && !lb.matches(&reg));
        let lb_tt = lb.clone().with(FilterOption::Technology(TechnologyFilter::TrueType));
        assert!(lb_tt.matches(&light) && !lb_tt.matches(&bold) && !lb_tt.matches(&reg));
        let caps = FilterSet::new().with(FilterOption::OpenType(OpenTypeGroup::Capitals));
        assert!(caps.matches(&bold) && !caps.matches(&light));
        let cond = FilterSet::new().with(FilterOption::Width(WidthFilter::Condensed));
        assert!(cond.matches(&bold) && !cond.matches(&light));
        assert!(FilterSet::new().matches(&reg));
    }

    #[test]
    fn style_categories_follow_panose_table() {
        let sans = font(400, 5, Technology::TrueType, [2, 11, 6, 4, 2, 2, 2, 2, 2, 4], &[]);
        let serif = font(400, 5, Technology::TrueType, [2, 2, 6, 3, 5, 4, 5, 2, 3, 4], &[]);
        let mono = font(400, 5, Technology::TrueType, [2, 7, 3, 9, 2, 2, 5, 2, 4, 4], &[]);
        let script = font(400, 5, Technology::TrueType, [3, 1, 1, 1, 1, 1, 1, 1, 1, 1], &[]);
        let deco = font(400, 5, Technology::TrueType, [4, 3, 9, 5, 2, 0, 0, 0, 0, 0], &[]);
        let sym = font(400, 5, Technology::TrueType, [5, 0, 0, 0, 0, 0, 0, 0, 0, 0], &[]);
        assert!(StyleCategory::SansSerif.matches(&sans) && !StyleCategory::Serif.matches(&sans));
        assert!(StyleCategory::Serif.matches(&serif));
        assert!(StyleCategory::Monospaced.matches(&mono) && !StyleCategory::Monospaced.matches(&serif));
        assert!(StyleCategory::Script.matches(&script));
        assert!(StyleCategory::Decorative.matches(&deco));
        assert!(StyleCategory::Symbol.matches(&sym));
        assert!(StyleCategory::Display.matches(&serif) && !StyleCategory::Display.matches(&script));
    }

    #[test]
    fn status_and_availability() {
        let mut sys = font(400, 5, Technology::TrueType, [0; 10], &[]);
        sys.protected = true;
        let mut not_inst = font(400, 5, Technology::TrueType, [0; 10], &[]);
        not_inst.installed = false;
        let inst = FilterSet::new().with(FilterOption::Status(StatusFilter::Installed));
        assert!(!inst.matches(&sys), "Installed excludes protected system fonts");
        let ni = FilterSet::new().with(FilterOption::Status(StatusFilter::NotInstalled));
        assert!(ni.matches(&not_inst) && !ni.matches(&sys));
        let groups = available_options([&sys, &not_inst]);
        assert_eq!(groups[0].options.len(), 4, "status options always shown, Corrupt hidden");
        assert_eq!(groups[2].options, vec![FilterOption::Technology(TechnologyFilter::TrueType)]);
        assert!(groups[6].options.contains(&FilterOption::CharRange(CharRange::Latin)));
        assert!(groups[7].options.is_empty());
    }

    #[test]
    fn char_ranges() {
        assert!(CharRange::Cyrillic.matches(&[1 << 2, 0], &[0; 4]));
        assert!(CharRange::Japanese.matches(&[0, 0], &[0, 1 << 17, 0, 0]));
        assert!(CharRange::Devanagari.matches(&[1, 0], &[1 << 15, 0, 0, 0]));
        assert!(!CharRange::Turkish.matches(&[1, 0], &[0; 4]));
    }
}
