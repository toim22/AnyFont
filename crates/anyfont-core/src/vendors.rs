//! `OS/2.achVendID` -> vendor (foundry) name and web site.
//!
//! The original resolves the "Vendor URL" of the Font info tab and the "Get more from this foundry" link
//! (`SelectedVendor`, `SelectedFoundry` properties of the data source) from the font's name ID 11 and, when
//! that is empty, from a vendor-id table. AnyFont ships its own table of well-known registered vendor ids
//! (public data of the OpenType vendor registry); unknown ids simply have no URL.

use crate::model::FontRecord;

/// Known vendors: `(achVendID, name, url)`. Ids are compared case-sensitively after trimming spaces.
pub const VENDORS: &[(&str, &str, &str)] = &[
    ("ADBE", "Adobe", "https://www.adobe.com/products/type.html"),
    ("ADBO", "Adobe", "https://www.adobe.com/products/type.html"),
    ("AGFA", "Agfa Monotype", "https://www.monotype.com"),
    ("ALTS", "Altsys / Macromedia", ""),
    ("APPL", "Apple", "https://developer.apple.com/fonts/"),
    ("ARPH", "Arphic Technology", "https://www.arphic.com"),
    ("B&H ", "Bigelow & Holmes", "https://bigelowandholmes.typepad.com"),
    ("BITS", "Bitstream", "https://www.bitstream.com"),
    ("CTDL", "China Type Design", ""),
    ("DAMA", "Dalton Maag", "https://www.daltonmaag.com"),
    ("DYNA", "DynaComware", "https://www.dynacw.com"),
    ("EF  ", "Elsner+Flake", "https://www.elsner-flake.com"),
    ("FBI ", "The Font Bureau", "https://fontbureau.typenetwork.com"),
    ("FSI ", "FSI Fonts und Software", "https://www.fontshop.com"),
    ("GOOG", "Google", "https://fonts.google.com"),
    ("HL  ", "High-Logic", "https://www.high-logic.com"),
    ("HY  ", "HanYang Information & Communication", ""),
    ("IBM ", "IBM", "https://www.ibm.com/plex/"),
    ("ITC ", "International Typeface Corporation", "https://www.itcfonts.com"),
    ("KATF", "Kingsley/ATF", ""),
    ("LINO", "Linotype", "https://www.linotype.com"),
    ("LTRX", "Lighttracks", ""),
    ("MACR", "Macromedia", ""),
    ("MONO", "Monotype", "https://www.monotype.com"),
    ("MS  ", "Microsoft", "https://www.microsoft.com/typography"),
    ("MSFT", "Microsoft", "https://www.microsoft.com/typography"),
    ("MT  ", "Monotype", "https://www.monotype.com"),
    ("NB  ", "No Bodoni Typography", ""),
    ("PARA", "ParaType", "https://www.paratype.com"),
    ("PYRS", "Pyrus", "https://www.pyrus.com"),
    ("RICO", "Ricoh", "https://www.ricoh.com"),
    ("SAJA", "Sandoll Communication", "https://www.sandoll.co.kr"),
    ("SIL ", "SIL International", "https://software.sil.org/fonts/"),
    ("STF ", "Brian Sooy & Co", ""),
    ("TMC ", "The Monotype Corporation", "https://www.monotype.com"),
    ("TYPO", "Typodermic Fonts", "https://typodermicfonts.com"),
    ("UKWN", "Unknown", ""),
    ("URW ", "URW++", "https://www.urwtype.com"),
    ("VLKF", "Visualogik Technology & Design", "https://www.visualogik.com"),
    ("YOFO", "Yoon Design", ""),
    ("ZSFT", "Zsoft", ""),
    ("pyrs", "Pyrus", "https://www.pyrus.com"),
    ("1ASC", "Ascender Corporation", "https://www.ascendercorp.com"),
    ("ASC ", "Ascender Corporation", "https://www.ascendercorp.com"),
    ("TT  ", "TypeTogether", "https://www.type-together.com"),
    ("CANO", "Canon", ""),
    ("ZANE", "Zane Studio", ""),
];

fn lookup(vendor_id: &str) -> Option<&'static (&'static str, &'static str, &'static str)> {
    let id = vendor_id.trim_end_matches(['\0', ' ']);
    if id.is_empty() {
        return None;
    }
    VENDORS.iter().find(|(v, _, _)| v.trim_end() == id)
}

/// Vendor name for an `achVendID`.
pub fn vendor_name(vendor_id: &str) -> Option<&'static str> {
    lookup(vendor_id).map(|v| v.1)
}

/// Vendor web site for an `achVendID` (`None` when unknown or the table has no URL).
pub fn vendor_url_for_id(vendor_id: &str) -> Option<&'static str> {
    lookup(vendor_id).map(|v| v.2).filter(|u| !u.is_empty())
}

/// The "Vendor URL" of the Font info tab: name ID 11, else the vendor-id table.
pub fn vendor_url(font: &FontRecord) -> Option<String> {
    let own = font.vendor_url.trim();
    if !own.is_empty() {
        return Some(own.to_string());
    }
    vendor_url_for_id(&font.vendor_id).map(str::to_string)
}

/// Foundry shown for the selected font (`SelectedFoundry`): manufacturer (name ID 8), else the vendor name.
pub fn foundry(font: &FontRecord) -> Option<String> {
    let m = font.manufacturer.trim();
    if !m.is_empty() {
        return Some(m.to_string());
    }
    vendor_name(&font.vendor_id).filter(|n| *n != "Unknown").map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vendor_lookup_and_fallback() {
        assert_eq!(vendor_name("MS  "), Some("Microsoft"));
        assert_eq!(vendor_name("MS"), Some("Microsoft"));
        assert!(vendor_url_for_id("ZZZZ").is_none());
        let mut f = FontRecord { vendor_id: "GOOG".into(), ..Default::default() };
        assert_eq!(vendor_url(&f).as_deref(), Some("https://fonts.google.com"));
        f.vendor_url = "https://example.org".into();
        assert_eq!(vendor_url(&f).as_deref(), Some("https://example.org"));
        assert_eq!(foundry(&f).as_deref(), Some("Google"));
    }
}
