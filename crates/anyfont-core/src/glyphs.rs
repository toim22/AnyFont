//! The Glyphs tab: glyph enumeration (encoded, alternates, unencoded) and the glyph filters.
//!
//! Order of the glyph list, as observed in the screenshots of the original:
//! - one cell per encoded **character**, in code-point order. Algerian maps 236 characters to 175 glyphs
//!   (a-z share the A-Z glyphs, U+00A0 shares the space) and the capture shows "262 glyphs" with the
//!   capitals repeated after the backquote: 236 characters + 26 unencoded glyphs;
//! - each character is immediately followed by the glyphs that GSUB single/alternate/ligature substitutions
//!   produce from its glyph, encoded or not (Bahnschrift: "0 ⁰ 0 0 1 ¹ 1 1 ½ ¼ 2 ² 2 2 …", "A ª B",
//!   "986 glyphs" for a font of 961 glyphs). The same alternates are offered by the alternates pop-up
//!   (`e4939644`, [`GlyphEntry::alternates`]);
//! - then every glyph id that is not reachable at all (unencoded glyphs) in glyph-id order, without
//!   `.notdef` and without empty glyphs (`.null`, `nonmarkingreturn`).
//!
//! Filters: the original groups glyphs by Unicode script (the "Common" group holds Currency,
//! Mathematical Symbols, Marks, Numbers, Punctuation & Separators, Symbols) and by category (Uppercase,
//! Lowercase, Other ...). Only (script, category) pairs present in the font are offered.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use ttf_parser::{GlyphId, LazyArray16};
use unicode_general_category::{get_general_category, GeneralCategory};
use unicode_script::{Script, UnicodeScript};

/// Category of a glyph (the checkboxes inside a glyph filter group).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum GlyphCategory {
    /// Lu, Lt.
    Uppercase,
    /// Ll.
    Lowercase,
    /// Nd, Nl, No.
    Numbers,
    /// P*, Z*.
    PunctuationAndSeparators,
    /// Sc.
    Currency,
    /// Sm.
    MathematicalSymbols,
    /// Mn, Mc, Me.
    Marks,
    /// Sk, So.
    Symbols,
    /// Lm, Lo, C*, and unencoded glyphs.
    Other,
}

impl GlyphCategory {
    /// Caption.
    pub fn caption(self) -> &'static str {
        match self {
            GlyphCategory::Uppercase => "Uppercase",
            GlyphCategory::Lowercase => "Lowercase",
            GlyphCategory::Numbers => "Numbers",
            GlyphCategory::PunctuationAndSeparators => "Punctuation & Separators",
            GlyphCategory::Currency => "Currency",
            GlyphCategory::MathematicalSymbols => "Mathematical Symbols",
            GlyphCategory::Marks => "Marks",
            GlyphCategory::Symbols => "Symbols",
            GlyphCategory::Other => "Other",
        }
    }
    /// Category of a character.
    pub fn of(c: char) -> GlyphCategory {
        use GeneralCategory as G;
        match get_general_category(c) {
            G::UppercaseLetter | G::TitlecaseLetter => GlyphCategory::Uppercase,
            G::LowercaseLetter => GlyphCategory::Lowercase,
            G::DecimalNumber | G::LetterNumber | G::OtherNumber => GlyphCategory::Numbers,
            G::ConnectorPunctuation
            | G::DashPunctuation
            | G::OpenPunctuation
            | G::ClosePunctuation
            | G::InitialPunctuation
            | G::FinalPunctuation
            | G::OtherPunctuation
            | G::SpaceSeparator
            | G::LineSeparator
            | G::ParagraphSeparator => GlyphCategory::PunctuationAndSeparators,
            G::CurrencySymbol => GlyphCategory::Currency,
            G::MathSymbol => GlyphCategory::MathematicalSymbols,
            G::NonspacingMark | G::SpacingMark | G::EnclosingMark => GlyphCategory::Marks,
            G::ModifierSymbol | G::OtherSymbol => GlyphCategory::Symbols,
            _ => GlyphCategory::Other,
        }
    }
}

/// Script group name of a character ("Common" for Common and Inherited, "Latin", "Greek" ...).
pub fn script_group(c: char) -> String {
    match c.script() {
        Script::Common | Script::Inherited | Script::Unknown => "Common".to_string(),
        s => s.full_name().replace('_', " "),
    }
}

/// Name of the group holding glyphs without a code point.
pub const UNENCODED_GROUP: &str = "Unencoded";

/// How a glyph entered the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GlyphOrigin {
    /// Mapped by the cmap.
    Encoded,
    /// Produced by a GSUB substitution of the preceding encoded glyph.
    Alternate,
    /// Not reachable from the cmap.
    Unencoded,
}

/// One cell of the glyph grid.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlyphEntry {
    /// Glyph id (`0` for Type 1 entries is valid too: they are indexed by name order).
    pub glyph_id: u16,
    /// Code point (encoded glyphs; an alternate carries its base character for classification). The list
    /// holds one entry per character, so a glyph mapped by two characters (A and a in an all-caps font, the
    /// space and U+00A0) has two cells, as in the original.
    pub codepoint: Option<char>,
    /// Encoded entries: the GSUB alternates of the glyph (depth-first, the same glyphs that follow the
    /// entry in the list). The grid marks such cells and offers them in the alternates pop-up.
    #[serde(default)]
    pub alternates: Vec<u16>,
    /// Glyph name (`post`/CFF/Type 1), if any.
    pub name: Option<String>,
    /// Origin.
    pub origin: GlyphOrigin,
    /// Script group (see [`script_group`]; [`UNENCODED_GROUP`] for unencoded glyphs).
    pub script: String,
    /// Category.
    pub category: GlyphCategory,
}

impl GlyphEntry {
    /// "U+0041" style caption, or the glyph id for unencoded glyphs.
    pub fn caption(&self) -> String {
        match (self.codepoint, self.origin) {
            (Some(c), GlyphOrigin::Encoded) => format!("U+{:04X}", c as u32),
            _ => match &self.name {
                Some(n) => n.clone(),
                None => format!("glyph {}", self.glyph_id),
            },
        }
    }
}

/// The glyph list of one face.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlyphList {
    /// Entries in display order.
    pub glyphs: Vec<GlyphEntry>,
}

/// One glyph filter checkbox: `script` group + category.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct GlyphFilter {
    /// Script group.
    pub script: String,
    /// Category.
    pub category: GlyphCategory,
}

impl GlyphList {
    /// "262 glyphs".
    pub fn count_text(&self) -> String {
        crate::format::count_text(self.glyphs.len(), "glyph")
    }
    /// The filter groups present: script -> categories, "Common" first, unencoded last.
    pub fn available_filters(&self) -> Vec<(String, Vec<GlyphCategory>)> {
        let mut map: BTreeMap<String, BTreeSet<GlyphCategory>> = BTreeMap::new();
        for g in &self.glyphs {
            map.entry(g.script.clone()).or_default().insert(g.category);
        }
        let mut out: Vec<(String, Vec<GlyphCategory>)> =
            map.into_iter().map(|(k, v)| (k, v.into_iter().collect())).collect();
        out.sort_by_key(|(k, _)| (k != "Common", k == UNENCODED_GROUP, k.clone()));
        out
    }
    /// The entries passing the checked filters (all entries when `filters` is empty). OR semantics.
    pub fn filtered<'a>(&'a self, filters: &'a BTreeSet<GlyphFilter>) -> impl Iterator<Item = &'a GlyphEntry> + 'a {
        self.glyphs.iter().filter(move |g| {
            filters.is_empty() || filters.iter().any(|f| f.script == g.script && f.category == g.category)
        })
    }
}

fn entry(gid: u16, cp: Option<char>, name: Option<&str>, origin: GlyphOrigin) -> GlyphEntry {
    let (script, category) = match (cp, origin) {
        (Some(c), _) => (script_group(c), GlyphCategory::of(c)),
        (None, _) => (UNENCODED_GROUP.to_string(), GlyphCategory::Other),
    };
    GlyphEntry { glyph_id: gid, codepoint: cp, alternates: Vec::new(), name: name.map(str::to_string), origin, script, category }
}

fn array_get(a: &LazyArray16<GlyphId>, i: u16) -> Option<GlyphId> {
    a.get(i)
}

/// Substitution outputs of every glyph reachable through GSUB lookups of type 1, 3 and 4 (and the
/// extension wrappers), keyed by input glyph.
fn gsub_alternates(face: &ttf_parser::Face) -> BTreeMap<u16, Vec<u16>> {
    use ttf_parser::gsub::{SingleSubstitution, SubstitutionSubtable};
    let mut out: BTreeMap<u16, Vec<u16>> = BTreeMap::new();
    let Some(gsub) = face.tables().gsub else { return out };
    let n = face.number_of_glyphs();
    let mut subtables = Vec::new();
    for lookup in gsub.lookups {
        for st in lookup.subtables.into_iter::<SubstitutionSubtable>() {
            subtables.push(st);
        }
    }
    if subtables.is_empty() {
        return out;
    }
    for g in 0..n {
        let gid = GlyphId(g);
        for st in &subtables {
            let mut push = |o: GlyphId| {
                if o.0 != g && o.0 < n {
                    let v = out.entry(g).or_default();
                    if !v.contains(&o.0) {
                        v.push(o.0);
                    }
                }
            };
            match st {
                SubstitutionSubtable::Single(s) => {
                    let Some(idx) = s.coverage().get(gid) else { continue };
                    match s {
                        SingleSubstitution::Format1 { delta, .. } => {
                            push(GlyphId((g as i32 + *delta as i32).rem_euclid(65536) as u16));
                        }
                        SingleSubstitution::Format2 { substitutes, .. } => {
                            if let Some(o) = array_get(substitutes, idx) {
                                push(o);
                            }
                        }
                    }
                }
                SubstitutionSubtable::Alternate(a) => {
                    let Some(idx) = a.coverage.get(gid) else { continue };
                    if let Some(set) = a.alternate_sets.get(idx) {
                        for o in set.alternates {
                            push(o);
                        }
                    }
                }
                SubstitutionSubtable::Ligature(l) => {
                    let Some(idx) = l.coverage.get(gid) else { continue };
                    if let Some(set) = l.ligature_sets.get(idx) {
                        for lig in set {
                            push(lig.glyph);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// Enumerates the glyphs of face `index` of an sfnt (or WOFF) file.
pub fn enumerate_sfnt(data: &[u8], index: u32) -> Option<GlyphList> {
    let decoded;
    let data = if crate::sfnt::be_u32(data, 0) == Some(crate::sfnt::TAG_WOFF) {
        decoded = crate::woff::decode_woff(data).ok()?;
        &decoded[..]
    } else {
        data
    };
    let face = ttf_parser::Face::parse(data, index).ok()?;
    let n = face.number_of_glyphs();
    // code point -> glyph, best Unicode subtable(s); symbol fonts (3,0) map F0xx.
    let mut cmap: BTreeMap<u32, u16> = BTreeMap::new();
    if let Some(table) = face.tables().cmap {
        for st in table.subtables {
            let symbol = st.platform_id == ttf_parser::PlatformId::Windows && st.encoding_id == 0;
            if !st.is_unicode() && !symbol {
                continue;
            }
            st.codepoints(|cp| {
                if let Some(g) = st.glyph_index(cp) {
                    if g.0 != 0 && g.0 < n {
                        cmap.entry(cp).or_insert(g.0);
                    }
                }
            });
        }
    }
    let alternates = gsub_alternates(&face);
    let mut reached = vec![false; n as usize];
    reached[0] = true; // .notdef is never listed
    let mut list = GlyphList::default();
    let name_of = |g: u16| face.glyph_name(GlyphId(g)).map(str::to_string);
    for (&cp, &g) in &cmap {
        let ch = char::from_u32(cp);
        reached[g as usize] = true;
        // depth-first: alternates of alternates follow their base; each glyph once per character
        let mut chain: Vec<u16> = Vec::new();
        let mut stack: Vec<u16> = alternates.get(&g).cloned().unwrap_or_default();
        stack.reverse();
        while let Some(a) = stack.pop() {
            if a == g || chain.contains(&a) {
                continue;
            }
            chain.push(a);
            if let Some(more) = alternates.get(&a) {
                for &m in more.iter().rev() {
                    stack.push(m);
                }
            }
        }
        let mut e = entry(g, ch, name_of(g).as_deref(), GlyphOrigin::Encoded);
        e.alternates = chain.clone();
        list.glyphs.push(e);
        for a in chain {
            reached[a as usize] = true;
            list.glyphs.push(entry(a, ch, name_of(a).as_deref(), GlyphOrigin::Alternate));
        }
    }
    for g in 1..n {
        if !reached[g as usize] && !is_empty_glyph(&face, g) {
            list.glyphs.push(entry(g, None, name_of(g).as_deref(), GlyphOrigin::Unencoded));
        }
    }
    Some(list)
}

/// A glyph with nothing to draw (`.null`, `nonmarkingreturn`, unencoded spaces): no outline, no bitmap,
/// no SVG and no colour layers.
fn is_empty_glyph(face: &ttf_parser::Face, g: u16) -> bool {
    struct Sink;
    impl ttf_parser::OutlineBuilder for Sink {
        fn move_to(&mut self, _: f32, _: f32) {}
        fn line_to(&mut self, _: f32, _: f32) {}
        fn quad_to(&mut self, _: f32, _: f32, _: f32, _: f32) {}
        fn curve_to(&mut self, _: f32, _: f32, _: f32, _: f32, _: f32, _: f32) {}
        fn close(&mut self) {}
    }
    let gid = GlyphId(g);
    face.outline_glyph(gid, &mut Sink).is_none()
        && face.glyph_raster_image(gid, u16::MAX).is_none()
        && face.glyph_svg_image(gid).is_none()
        && !face.is_color_glyph(gid)
}

/// Glyph list of a Type 1 font from its glyph names (Adobe Glyph List subset for the code points).
pub fn enumerate_type1(glyph_names: &[String]) -> GlyphList {
    let mut encoded: Vec<(char, u16, &str)> = Vec::new();
    let mut unencoded: Vec<(u16, &str)> = Vec::new();
    for (i, n) in glyph_names.iter().enumerate() {
        let gid = i.min(u16::MAX as usize) as u16;
        if n == ".notdef" {
            continue;
        }
        match crate::type1::glyph_name_to_unicode(n) {
            Some(c) => encoded.push((c, gid, n)),
            None => unencoded.push((gid, n)),
        }
    }
    encoded.sort_by_key(|e| e.0);
    let mut list = GlyphList::default();
    for (c, g, n) in encoded {
        list.glyphs.push(entry(g, Some(c), Some(n), GlyphOrigin::Encoded));
    }
    for (g, n) in unencoded {
        list.glyphs.push(entry(g, None, Some(n), GlyphOrigin::Unencoded));
    }
    list
}

/// Glyph list of a font file on disk (`face_index` for collections). `None` when unreadable.
pub fn enumerate_file(path: &std::path::Path, face_index: u32) -> Option<GlyphList> {
    let data = std::fs::read(path).ok()?;
    if crate::type1::is_type1(&data) {
        let t = crate::type1::parse_type1(&data).ok()?;
        return Some(enumerate_type1(&t.glyph_names));
    }
    enumerate_sfnt(&data, face_index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories() {
        assert_eq!(GlyphCategory::of('A'), GlyphCategory::Uppercase);
        assert_eq!(GlyphCategory::of('a'), GlyphCategory::Lowercase);
        assert_eq!(GlyphCategory::of('7'), GlyphCategory::Numbers);
        assert_eq!(GlyphCategory::of('$'), GlyphCategory::Currency);
        assert_eq!(GlyphCategory::of('+'), GlyphCategory::MathematicalSymbols);
        assert_eq!(GlyphCategory::of(','), GlyphCategory::PunctuationAndSeparators);
        assert_eq!(GlyphCategory::of('\u{301}'), GlyphCategory::Marks);
        assert_eq!(GlyphCategory::of('©'), GlyphCategory::Symbols);
        assert_eq!(script_group('A'), "Latin");
        assert_eq!(script_group('$'), "Common");
        assert_eq!(script_group('\u{301}'), "Common");
        assert_eq!(script_group('Ж'), "Cyrillic");
    }

    #[test]
    fn type1_list_and_filters() {
        let names: Vec<String> = [".notdef", "B", "A", "dollar", "uni0416", "foo.alt"].iter().map(|s| s.to_string()).collect();
        let l = enumerate_type1(&names);
        let caps: Vec<_> = l.glyphs.iter().map(|g| g.caption()).collect();
        assert_eq!(caps[0], "U+0024");
        assert_eq!(caps[1], "U+0041");
        assert_eq!(l.glyphs.last().unwrap().origin, GlyphOrigin::Unencoded);
        assert_eq!(l.count_text(), "5 glyphs", ".notdef is not listed");
        let f = l.available_filters();
        assert_eq!(f[0].0, "Common");
        assert_eq!(f.last().unwrap().0, UNENCODED_GROUP);
        let mut sel = BTreeSet::new();
        sel.insert(GlyphFilter { script: "Latin".into(), category: GlyphCategory::Uppercase });
        assert_eq!(l.filtered(&sel).count(), 2);
    }
}
