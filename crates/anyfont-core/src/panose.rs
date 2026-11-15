//! PANOSE font matching: missing-font substitution, exceptions, and Type 1 replacement suggestions.
//!
//! Evidence (dossier `09-features/feature-catalog.md` section 3, engine `WPanoseFontArray::
//! GetFontMatchingForMissingFonts` 0x1801A6A40, `WPanoseFontMatching::FillFontListCallback` 0x1801A7840):
//!
//! * Preferences: `ActiveIOFontMatchingType` = Never show / Text / Text and styles ([`PanoseMode`]),
//!   a default font, and an exception list (`WPanoseExceptionDialogDS`: Add Exception / Remove Exception).
//! * Results dialog (`WFontMatchingResultsDS`): per missing font a substitute, `SubstitutionType`
//!   (`Panose` / `Exception`), `SubstitutionStatus` (`Permanent` / `Temporary`), and the candidate list
//!   filtered by `FontFilterType` (`SameCodePage` / `ShowAll`).
//! * Inputs gathered per missing font (`PopulateBestPanoseMatchData`): PostScript name and style,
//!   technology, version, the 10 PANOSE digits, `uCharSet`, face/family names and the style.
//!
//! The distance function itself is not recoverable from the dossier; AnyFont uses the classic weighted
//! PANOSE 1.0 metric documented on [`distance`].

use crate::model::{FontId, FontRecord, Panose, Technology};
use serde::{Deserialize, Serialize};

/// When to show the font matching results (`ActiveIOFontMatchingType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum PanoseMode {
    /// "Never show": substitute silently.
    NeverShow,
    /// "Text": show results for missing text fonts.
    #[default]
    Text,
    /// "Text and styles": also for fonts referenced by styles.
    TextAndStyles,
}

impl PanoseMode {
    /// Caption of the radio button.
    pub fn caption(self) -> &'static str {
        match self {
            PanoseMode::NeverShow => "Never show",
            PanoseMode::Text => "Text",
            PanoseMode::TextAndStyles => "Text and styles",
        }
    }
    /// All modes in dialog order.
    pub const ALL: [PanoseMode; 3] = [PanoseMode::NeverShow, PanoseMode::Text, PanoseMode::TextAndStyles];
    /// True when the results dialog should be shown for a missing font (`from_style`: referenced by a style).
    pub fn shows_results(self, from_style: bool) -> bool {
        match self {
            PanoseMode::NeverShow => false,
            PanoseMode::Text => !from_style,
            PanoseMode::TextAndStyles => true,
        }
    }
}

/// How the substitute was chosen (`SubstitutionType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SubstitutionType {
    /// By PANOSE / name similarity.
    Panose,
    /// From the exception list.
    Exception,
}

/// Whether a substitution is remembered (`SubstitutionStatus`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum SubstitutionStatus {
    /// Stored and reused for every later document.
    Permanent,
    /// Only for the current document.
    #[default]
    Temporary,
}

/// Candidate list filter (`FontFilterType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum FontFilterType {
    /// Only fonts supporting the missing font's character set.
    #[default]
    SameCodePage,
    /// Every font.
    ShowAll,
}

/// A font referenced by a document but not available.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MissingFont {
    /// Family / face name as referenced.
    pub family: String,
    /// Style ("Bold Italic"), may be empty.
    pub style: String,
    /// PostScript name, may be empty.
    pub postscript_name: String,
    /// Weight class (400 if unknown).
    pub weight: u16,
    /// Italic.
    pub italic: bool,
    /// PANOSE digits when the document stored them.
    pub panose: Option<Panose>,
    /// Windows character set (`uCharSet`, 0 = ANSI, 2 = Symbol ...).
    pub charset: Option<u8>,
    /// Technology, if known.
    pub technology: Option<Technology>,
}

/// An entry of the exception list: always replace `missing` with `substitute`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanoseException {
    /// Missing font family.
    pub missing: String,
    /// Replacement family.
    pub substitute: String,
}

/// A stored (permanent) substitution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Substitution {
    /// Missing font family.
    pub missing: String,
    /// Replacement family.
    pub substitute: String,
    /// How it was chosen.
    pub kind: SubstitutionType,
    /// Permanent or temporary.
    pub status: SubstitutionStatus,
}

/// One row of the candidate list.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Candidate {
    /// Font id.
    pub font_id: FontId,
    /// Group family.
    pub family: String,
    /// PANOSE distance (`None` when not comparable).
    pub panose_distance: Option<u32>,
    /// Normalised name similarity 0..1.
    pub name_similarity: f32,
}

/// Why the substitute was chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum MatchReason {
    /// From the exception list.
    Exception,
    /// From a permanent substitution.
    StoredSubstitution,
    /// The font is in fact available under that name.
    ExactName,
    /// Same family, closest style.
    SameFamily,
    /// Nearest PANOSE.
    Panose,
    /// Most similar name.
    NameSimilarity,
    /// The default font of the preferences.
    DefaultFont,
}

/// Result for one missing font.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MatchResult {
    /// Chosen substitute.
    pub substitute: Option<FontId>,
    /// Why.
    pub reason: MatchReason,
    /// `SubstitutionType` shown in the dialog.
    pub kind: SubstitutionType,
    /// Candidates, best first, filtered by the requested [`FontFilterType`].
    pub candidates: Vec<Candidate>,
}

/// Per-digit weights for digits 1..9 (serif style, weight, proportion, contrast, stroke variation,
/// arm style, letterform, midline, x-height).
pub const DIGIT_WEIGHTS: [u32; 9] = [3, 6, 5, 2, 1, 1, 3, 1, 1];

/// Weighted PANOSE distance. `None` when the family kinds differ (or either font has no PANOSE).
///
/// For digits 1..9: a digit that is 0 ("Any") on either side costs nothing; 1 ("No Fit") on exactly one
/// side costs `weight * 9`; otherwise the cost is `weight * (a - b)^2`.
pub fn distance(a: &Panose, b: &Panose) -> Option<u32> {
    if a.is_empty() || b.is_empty() {
        return None;
    }
    let (fa, fb) = (a.0[0], b.0[0]);
    if fa != fb && fa > 1 && fb > 1 {
        return None;
    }
    let mut d = 0u32;
    for i in 1..10 {
        let (x, y) = (a.0[i] as i32, b.0[i] as i32);
        let w = DIGIT_WEIGHTS[i - 1];
        d += if x == 0 || y == 0 {
            0
        } else if (x == 1) != (y == 1) {
            w * 9
        } else {
            w * ((x - y) * (x - y)) as u32
        };
    }
    Some(d)
}

/// Levenshtein distance of two strings (chars).
pub fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// Normalises a family name for comparison (lower case, no spaces/hyphens/underscores).
pub fn normalize_name(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// Name similarity 0..1 (1 = identical after normalisation).
pub fn name_similarity(a: &str, b: &str) -> f32 {
    let (a, b) = (normalize_name(a), normalize_name(b));
    let len = a.chars().count().max(b.chars().count());
    if len == 0 {
        return 0.0;
    }
    1.0 - levenshtein(&a, &b) as f32 / len as f32
}

/// Code-page bit (`ulCodePageRange`) of a Windows character set.
pub fn charset_code_page_bit(charset: u8) -> Option<u32> {
    Some(match charset {
        0 => 0,
        238 => 1,
        204 => 2,
        161 => 3,
        162 => 4,
        177 => 5,
        178 => 6,
        186 => 7,
        163 => 8,
        222 => 16,
        128 => 17,
        134 => 18,
        129 => 19,
        136 => 20,
        130 => 21,
        2 => 31,
        _ => return None,
    })
}

fn supports_charset(f: &FontRecord, charset: Option<u8>) -> bool {
    let Some(bit) = charset.and_then(charset_code_page_bit) else { return true };
    if bit == 31 {
        return f.symbol || f.code_page_range[0] & (1 << 31) != 0;
    }
    let range = f.code_page_range;
    if range == [0, 0] {
        return bit == 0;
    }
    range[(bit / 32) as usize] & (1 << (bit % 32)) != 0
}

fn style_cost(f: &FontRecord, m: &MissingFont) -> u32 {
    let w = (f.weight_class as i32 - m.weight.max(1) as i32).unsigned_abs();
    w + if f.italic != m.italic { 1000 } else { 0 }
}

fn family_names(f: &FontRecord) -> [&str; 3] {
    [f.group_family(), f.family.as_str(), f.full_name.as_str()]
}

/// Finds the substitute of a missing font among `fonts` (usable faces only).
///
/// Stages: exception list -> permanent substitution -> exact name (family, full or PostScript name) ->
/// same family with the closest style -> nearest PANOSE (candidates restricted to the character set when
/// `filter` is [`FontFilterType::SameCodePage`]) -> most similar name -> default font.
pub fn find_substitute(
    missing: &MissingFont,
    fonts: &[FontRecord],
    exceptions: &[PanoseException],
    substitutions: &[Substitution],
    default_font: Option<&str>,
    filter: FontFilterType,
) -> MatchResult {
    let usable: Vec<&FontRecord> = fonts.iter().filter(|f| f.is_usable()).collect();
    let best_in_family = |family: &str| -> Option<FontId> {
        let n = normalize_name(family);
        usable
            .iter()
            .filter(|f| family_names(f).iter().any(|x| normalize_name(x) == n))
            .min_by_key(|f| style_cost(f, missing))
            .map(|f| f.id)
    };
    // candidates
    let mut candidates: Vec<Candidate> = usable
        .iter()
        .filter(|f| filter == FontFilterType::ShowAll || supports_charset(f, missing.charset))
        .map(|f| Candidate {
            font_id: f.id,
            family: f.group_family().to_string(),
            panose_distance: missing.panose.as_ref().and_then(|p| distance(p, &f.panose)),
            name_similarity: family_names(f)
                .iter()
                .map(|n| name_similarity(&missing.family, n))
                .fold(0.0, f32::max),
        })
        .collect();
    candidates.sort_by(|a, b| {
        let da = a.panose_distance.unwrap_or(u32::MAX);
        let db = b.panose_distance.unwrap_or(u32::MAX);
        da.cmp(&db)
            .then(b.name_similarity.partial_cmp(&a.name_similarity).unwrap_or(std::cmp::Ordering::Equal))
            .then(a.family.cmp(&b.family))
            .then(a.font_id.cmp(&b.font_id))
    });
    let result = |id: Option<FontId>, reason: MatchReason, kind: SubstitutionType, candidates: Vec<Candidate>| MatchResult {
        substitute: id,
        reason,
        kind,
        candidates,
    };
    let mn = normalize_name(&missing.family);
    if let Some(e) = exceptions.iter().find(|e| normalize_name(&e.missing) == mn) {
        if let Some(id) = best_in_family(&e.substitute) {
            return result(Some(id), MatchReason::Exception, SubstitutionType::Exception, candidates);
        }
    }
    if let Some(s) = substitutions
        .iter()
        .find(|s| s.status == SubstitutionStatus::Permanent && normalize_name(&s.missing) == mn)
    {
        if let Some(id) = best_in_family(&s.substitute) {
            return result(Some(id), MatchReason::StoredSubstitution, s.kind, candidates);
        }
    }
    if !missing.postscript_name.is_empty() {
        if let Some(f) = usable.iter().find(|f| f.postscript_name.eq_ignore_ascii_case(&missing.postscript_name)) {
            return result(Some(f.id), MatchReason::ExactName, SubstitutionType::Panose, candidates);
        }
    }
    if let Some(id) = best_in_family(&missing.family) {
        let f = usable.iter().find(|f| f.id == id).expect("id from usable");
        let reason = if style_cost(f, missing) == 0 { MatchReason::ExactName } else { MatchReason::SameFamily };
        return result(Some(id), reason, SubstitutionType::Panose, candidates);
    }
    if let Some(c) = candidates.iter().find(|c| c.panose_distance.is_some()) {
        let fam = c.family.clone();
        let id = best_in_family(&fam).unwrap_or(c.font_id);
        return result(Some(id), MatchReason::Panose, SubstitutionType::Panose, candidates);
    }
    if let Some(c) = candidates
        .iter()
        .filter(|c| c.name_similarity >= 0.5)
        .max_by(|a, b| a.name_similarity.partial_cmp(&b.name_similarity).unwrap_or(std::cmp::Ordering::Equal))
    {
        let id = best_in_family(&c.family).unwrap_or(c.font_id);
        return result(Some(id), MatchReason::NameSimilarity, SubstitutionType::Panose, candidates);
    }
    let def = default_font.and_then(best_in_family);
    result(def, MatchReason::DefaultFont, SubstitutionType::Panose, candidates)
}

/// Message of the Type 1 information dialog (`Type1InfoMessageDS`).
pub const TYPE1_MESSAGE: &str = "Type 1 fonts detected. Windows and many applications are ending support for \
Type 1 fonts; consider updating the text to use TrueType (TTF) or OpenType (OTF) fonts.";

/// A suggested replacement of a Type 1 face by an OpenType/TrueType face ("Replace fonts").
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Type1Replacement {
    /// The Type 1 face.
    pub type1: FontId,
    /// The suggested replacement (`None` when nothing similar exists).
    pub replacement: Option<FontId>,
}

fn strip_type1_suffixes(name: &str) -> String {
    let mut n = normalize_name(name);
    for suffix in ["std", "pro", "mt", "lt", "ps", "type1", "t1"] {
        if n.len() > suffix.len() + 2 && n.ends_with(suffix) {
            n.truncate(n.len() - suffix.len());
        }
    }
    n
}

/// For every Type 1 face of `fonts`, the best non-Type-1 face with the same (normalised) family and the
/// closest weight/italic, else the most similar family name (similarity >= 0.75).
pub fn type1_replacements(fonts: &[FontRecord]) -> Vec<Type1Replacement> {
    let modern: Vec<&FontRecord> =
        fonts.iter().filter(|f| f.is_usable() && f.technology != Technology::Type1).collect();
    fonts
        .iter()
        .filter(|f| f.technology == Technology::Type1)
        .map(|t1| {
            let want = MissingFont { family: t1.family.clone(), weight: t1.weight_class, italic: t1.italic, ..Default::default() };
            let fam = strip_type1_suffixes(&t1.family);
            let same: Option<&&FontRecord> = modern
                .iter()
                .filter(|f| family_names(f).iter().any(|n| strip_type1_suffixes(n) == fam))
                .min_by_key(|f| style_cost(f, &want));
            let replacement = same.map(|f| f.id).or_else(|| {
                modern
                    .iter()
                    .map(|f| (f, name_similarity(&fam, &strip_type1_suffixes(f.group_family()))))
                    .filter(|(_, s)| *s >= 0.75)
                    .max_by(|a, b| {
                        a.1.partial_cmp(&b.1)
                            .unwrap_or(std::cmp::Ordering::Equal)
                            .then_with(|| style_cost(b.0, &want).cmp(&style_cost(a.0, &want)))
                    })
                    .map(|(f, _)| f.id)
            });
            Type1Replacement { type1: t1.id, replacement }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn font(id: i64, family: &str, weight: u16, italic: bool, panose: [u8; 10], cp: u32) -> FontRecord {
        FontRecord {
            id,
            family: family.into(),
            weight_class: weight,
            italic,
            panose: Panose(panose),
            code_page_range: [cp, 0],
            paths: vec![format!("C:/f/{id}.ttf").into()],
            ..Default::default()
        }
    }

    const ARIAL: [u8; 10] = [2, 11, 6, 4, 2, 2, 2, 2, 2, 4];
    const HELV_BOLD: [u8; 10] = [2, 11, 8, 4, 2, 2, 2, 2, 2, 4];
    const TIMES: [u8; 10] = [2, 2, 6, 3, 5, 4, 5, 2, 3, 4];
    const SCRIPT: [u8; 10] = [3, 1, 1, 1, 1, 1, 1, 1, 1, 1];

    fn library() -> Vec<FontRecord> {
        vec![
            font(1, "Arial", 400, false, ARIAL, 1),
            font(2, "Arial", 700, false, HELV_BOLD, 1),
            font(3, "Times New Roman", 400, false, TIMES, 1),
            font(4, "Brush Script", 400, true, SCRIPT, 1),
            font(5, "Cyr Sans", 400, false, ARIAL, 1 << 2),
        ]
    }

    #[test]
    fn distance_ranks_similar_designs_first() {
        let a = Panose(ARIAL);
        assert_eq!(distance(&a, &a), Some(0));
        assert!(distance(&a, &Panose(HELV_BOLD)).unwrap() < distance(&a, &Panose(TIMES)).unwrap());
        assert_eq!(distance(&a, &Panose(SCRIPT)), None, "different family kind");
        assert_eq!(distance(&a, &Panose::default()), None);
    }

    #[test]
    fn panose_ranking_and_filters() {
        let lib = library();
        let m = MissingFont {
            family: "Helvetica".into(),
            weight: 700,
            panose: Some(Panose(HELV_BOLD)),
            charset: Some(0),
            ..Default::default()
        };
        let r = find_substitute(&m, &lib, &[], &[], Some("Arial"), FontFilterType::SameCodePage);
        assert_eq!(r.reason, MatchReason::Panose);
        assert_eq!(r.substitute, Some(2), "Arial family, bold face chosen by style");
        assert_eq!(r.candidates[0].font_id, 2);
        assert!(!r.candidates.iter().any(|c| c.font_id == 5), "SameCodePage hides the Cyrillic-only font");
        let all = find_substitute(&m, &lib, &[], &[], None, FontFilterType::ShowAll);
        assert!(all.candidates.iter().any(|c| c.font_id == 5));
        let last = r.candidates.last().unwrap();
        assert_eq!(last.panose_distance, None);
    }

    #[test]
    fn stages_exceptions_substitutions_names_default() {
        let lib = library();
        let m = MissingFont { family: "Helvetica".into(), weight: 400, ..Default::default() };
        let ex = [PanoseException { missing: "helvetica".into(), substitute: "Times New Roman".into() }];
        let r = find_substitute(&m, &lib, &ex, &[], None, FontFilterType::ShowAll);
        assert_eq!((r.substitute, r.reason, r.kind), (Some(3), MatchReason::Exception, SubstitutionType::Exception));
        let subs = [Substitution {
            missing: "Helvetica".into(),
            substitute: "Brush Script".into(),
            kind: SubstitutionType::Panose,
            status: SubstitutionStatus::Permanent,
        }];
        let r = find_substitute(&m, &lib, &[], &subs, None, FontFilterType::ShowAll);
        assert_eq!((r.substitute, r.reason), (Some(4), MatchReason::StoredSubstitution));
        let exact = MissingFont { family: "arial".into(), weight: 700, ..Default::default() };
        assert_eq!(find_substitute(&exact, &lib, &[], &[], None, FontFilterType::ShowAll).substitute, Some(2));
        let similar = MissingFont { family: "Times New Roman PS".into(), weight: 400, ..Default::default() };
        let r = find_substitute(&similar, &lib, &[], &[], None, FontFilterType::ShowAll);
        assert_eq!((r.substitute, r.reason), (Some(3), MatchReason::NameSimilarity));
        let none = MissingFont { family: "Zzzzzz".into(), weight: 400, ..Default::default() };
        let r = find_substitute(&none, &lib, &[], &[], Some("Arial"), FontFilterType::ShowAll);
        assert_eq!((r.substitute, r.reason), (Some(1), MatchReason::DefaultFont));
    }

    #[test]
    fn type1_replacement_suggestion() {
        let mut lib = library();
        let mut t1 = font(10, "Arial MT", 700, false, [0; 10], 1);
        t1.technology = Technology::Type1;
        let mut t1b = font(11, "Qwerty", 400, false, [0; 10], 1);
        t1b.technology = Technology::Type1;
        lib.push(t1);
        lib.push(t1b);
        let r = type1_replacements(&lib);
        assert_eq!(r, vec![
            Type1Replacement { type1: 10, replacement: Some(2) },
            Type1Replacement { type1: 11, replacement: None }
        ]);
    }

    #[test]
    fn modes() {
        assert!(!PanoseMode::NeverShow.shows_results(false));
        assert!(PanoseMode::Text.shows_results(false) && !PanoseMode::Text.shows_results(true));
        assert!(PanoseMode::TextAndStyles.shows_results(true));
    }
}
