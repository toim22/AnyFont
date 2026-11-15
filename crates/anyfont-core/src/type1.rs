//! Adobe Type 1 fonts: `.pfb` (binary), `.pfa` (ASCII), `.pfm` (Windows metrics) and `.afm` (metrics).
//!
//! The original engine reads Type 1 fonts through `CFontInformerT1*` and bypasses the sfnt checks
//! (`font-files.md` §6.5). AnyFont parses the clear-text font dictionary (`/FontName`, `/FamilyName`,
//! `/FullName`, `/Weight`, `/ItalicAngle`, `/isFixedPitch`, `/Notice`, `/Copyright`, `/version`), decrypts
//! the `eexec` section to count and name the glyphs (`/CharStrings`), and merges the Windows metrics file
//! (`.pfm`: face name, weight, italic, charset) or the `.afm` when one sits next to the outline file.
//! Glyph names are mapped to Unicode with `uniXXXX`/`uXXXX` names and a Latin subset of the Adobe Glyph
//! List, enough for the Glyphs tab and the Type 1 -> OpenType replacement suggestion.

use std::path::{Path, PathBuf};

/// Data read from a `.pfb`/`.pfa`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Type1Font {
    /// `/FontName` (PostScript name).
    pub font_name: String,
    /// `/FamilyName`.
    pub family_name: String,
    /// `/FullName`.
    pub full_name: String,
    /// `/Weight` ("Bold").
    pub weight: String,
    /// `/ItalicAngle`.
    pub italic_angle: f32,
    /// `/isFixedPitch`.
    pub is_fixed_pitch: bool,
    /// `/Notice`.
    pub notice: String,
    /// `/Copyright`.
    pub copyright: String,
    /// `/version`.
    pub version: String,
    /// Glyph names of `/CharStrings` (empty when the private part could not be decrypted).
    pub glyph_names: Vec<String>,
}

/// Data read from a `.pfm`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pfm {
    /// Windows face (family) name.
    pub face_name: String,
    /// PostScript name (driver info).
    pub postscript_name: String,
    /// Weight (100..900).
    pub weight: u16,
    /// Italic flag.
    pub italic: bool,
    /// Windows charset (0 ANSI, 2 SYMBOL ...).
    pub charset: u8,
    /// Pitch and family byte.
    pub pitch_and_family: u8,
    /// Copyright string.
    pub copyright: String,
}

/// Data read from an `.afm`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Afm {
    /// FontName.
    pub font_name: String,
    /// FullName.
    pub full_name: String,
    /// FamilyName.
    pub family_name: String,
    /// Weight.
    pub weight: String,
    /// ItalicAngle.
    pub italic_angle: f32,
    /// IsFixedPitch.
    pub is_fixed_pitch: bool,
    /// Notice.
    pub notice: String,
    /// Version.
    pub version: String,
    /// Glyph names from `C ... ; N name ;` lines.
    pub glyph_names: Vec<String>,
}

/// True when the data looks like a Type 1 font (`.pfb` segment marker or `%!PS-AdobeFont`/`%!FontType1`).
pub fn is_type1(data: &[u8]) -> bool {
    (data.len() > 6 && data[0] == 0x80 && data[1] == 0x01)
        || data.starts_with(b"%!PS-AdobeFont")
        || data.starts_with(b"%!FontType1")
}

/// Splits a `.pfb` into (clear text, encrypted binary) or reads a `.pfa`.
fn split(data: &[u8]) -> Result<(Vec<u8>, Vec<u8>), String> {
    if data.first() == Some(&0x80) {
        let mut clear = Vec::new();
        let mut binary = Vec::new();
        let mut p = 0;
        while p + 2 <= data.len() && data[p] == 0x80 {
            let kind = data[p + 1];
            if kind == 3 {
                break;
            }
            let len = data
                .get(p + 2..p + 6)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
                .ok_or("truncated PFB segment")?;
            let seg = data.get(p + 6..p + 6 + len).ok_or("truncated PFB segment")?;
            match kind {
                1 if binary.is_empty() => clear.extend_from_slice(seg),
                1 => {} // trailing cleartomark
                2 => binary.extend_from_slice(seg),
                _ => return Err("bad PFB segment".into()),
            }
            p += 6 + len;
        }
        if clear.is_empty() {
            return Err("empty PFB".into());
        }
        Ok((clear, binary))
    } else {
        let text = data;
        let pos = find(text, b"eexec").ok_or("no eexec section")?;
        let clear = text[..pos + 5].to_vec();
        let hex: Vec<u8> = text[pos + 5..].iter().copied().filter(|c| c.is_ascii_hexdigit()).collect();
        let mut binary = Vec::with_capacity(hex.len() / 2);
        for pair in hex.as_chunks::<2>().0 {
            let s = std::str::from_utf8(pair).unwrap_or("00");
            binary.push(u8::from_str_radix(s, 16).unwrap_or(0));
        }
        Ok((clear, binary))
    }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// eexec decryption (r = 55665), dropping the 4 random leading bytes.
pub fn eexec_decrypt(data: &[u8]) -> Vec<u8> {
    let mut r: u16 = 55665;
    let mut out = Vec::with_capacity(data.len());
    for &c in data {
        out.push(c ^ (r >> 8) as u8);
        r = (c as u16).wrapping_add(r).wrapping_mul(52845).wrapping_add(22719);
    }
    out.into_iter().skip(4).collect()
}

/// eexec encryption (for test fixtures).
pub fn eexec_encrypt(plain: &[u8]) -> Vec<u8> {
    let mut r: u16 = 55665;
    let mut out = Vec::with_capacity(plain.len() + 4);
    for &p in [0u8, 0, 0, 0].iter().chain(plain.iter()) {
        let c = p ^ (r >> 8) as u8;
        out.push(c);
        r = (c as u16).wrapping_add(r).wrapping_mul(52845).wrapping_add(22719);
    }
    out
}

/// Reads a PostScript string `( ... )` or name `/...` value following `key` in the clear text.
fn ps_value(text: &str, key: &str) -> Option<String> {
    let mut search = 0;
    while let Some(i) = text[search..].find(key) {
        let at = search + i + key.len();
        search = at;
        // the key must be followed by a delimiter
        let rest = &text[at..];
        if rest.starts_with(|c: char| c.is_alphanumeric()) {
            continue;
        }
        let rest = rest.trim_start();
        if let Some(stripped) = rest.strip_prefix('(') {
            let mut depth = 1;
            let mut out = String::new();
            let mut chars = stripped.chars();
            while let Some(c) = chars.next() {
                match c {
                    '\\' => {
                        if let Some(n) = chars.next() {
                            out.push(n);
                        }
                    }
                    '(' => {
                        depth += 1;
                        out.push(c);
                    }
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                        out.push(c);
                    }
                    _ => out.push(c),
                }
            }
            return Some(out);
        }
        if let Some(stripped) = rest.strip_prefix('/') {
            return Some(stripped.split(|c: char| c.is_whitespace() || "/[]{}()<>".contains(c)).next()?.to_string());
        }
        return Some(rest.split_whitespace().next()?.to_string());
    }
    None
}

/// Parses `/CharStrings` glyph names from the decrypted private part.
fn charstring_names(private: &[u8]) -> Vec<String> {
    let mut names = Vec::new();
    let Some(start) = find(private, b"/CharStrings") else { return names };
    let mut p = start + 12;
    let n = private.len();
    // skip "<count> dict dup begin"
    while p < n && private[p] != b'/' {
        p += 1;
    }
    while p < n {
        // expect "/name <len> RD <bytes> ND"
        if private[p] != b'/' {
            if private[p..].starts_with(b"end") {
                break;
            }
            p += 1;
            continue;
        }
        let name_start = p + 1;
        let mut q = name_start;
        while q < n && !private[q].is_ascii_whitespace() {
            q += 1;
        }
        let name = String::from_utf8_lossy(&private[name_start..q]).into_owned();
        while q < n && private[q].is_ascii_whitespace() {
            q += 1;
        }
        let num_start = q;
        while q < n && private[q].is_ascii_digit() {
            q += 1;
        }
        let Ok(len) = std::str::from_utf8(&private[num_start..q]).unwrap_or("").parse::<usize>() else { break };
        while q < n && private[q].is_ascii_whitespace() {
            q += 1;
        }
        // RD or -|
        while q < n && !private[q].is_ascii_whitespace() {
            q += 1;
        }
        q += 1; // single space before the binary data
        q += len;
        names.push(name);
        // skip "ND" / "|-"
        while q < n && private[q] != b'/' && !private[q..].starts_with(b"end") {
            q += 1;
        }
        p = q;
    }
    names
}

/// Parses a `.pfb` or `.pfa`.
pub fn parse_type1(data: &[u8]) -> Result<Type1Font, String> {
    let (clear, binary) = split(data)?;
    let text = String::from_utf8_lossy(&clear).into_owned();
    let font_name = ps_value(&text, "/FontName").ok_or("no /FontName in the Type 1 font")?;
    let mut f = Type1Font {
        family_name: ps_value(&text, "/FamilyName").unwrap_or_default(),
        full_name: ps_value(&text, "/FullName").unwrap_or_default(),
        weight: ps_value(&text, "/Weight").unwrap_or_default(),
        italic_angle: ps_value(&text, "/ItalicAngle").and_then(|v| v.parse().ok()).unwrap_or(0.0),
        is_fixed_pitch: ps_value(&text, "/isFixedPitch").map(|v| v == "true").unwrap_or(false),
        notice: ps_value(&text, "/Notice").unwrap_or_default(),
        copyright: ps_value(&text, "/Copyright").unwrap_or_default(),
        version: ps_value(&text, "/version").unwrap_or_default(),
        font_name,
        glyph_names: Vec::new(),
    };
    if !binary.is_empty() {
        f.glyph_names = charstring_names(&eexec_decrypt(&binary));
    }
    if f.family_name.is_empty() {
        f.family_name = f.font_name.split('-').next().unwrap_or(&f.font_name).to_string();
    }
    if f.full_name.is_empty() {
        f.full_name = f.font_name.replace('-', " ");
    }
    Ok(f)
}

fn le_u16(d: &[u8], o: usize) -> Option<u16> {
    d.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]))
}
fn le_u32(d: &[u8], o: usize) -> Option<u32> {
    d.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}
fn c_string(d: &[u8], o: usize) -> String {
    match d.get(o..) {
        Some(s) => {
            let end = s.iter().position(|&b| b == 0).unwrap_or(s.len());
            s[..end].iter().map(|&b| b as char).collect()
        }
        None => String::new(),
    }
}

/// Parses a Windows `.pfm`.
pub fn parse_pfm(d: &[u8]) -> Result<Pfm, String> {
    if d.len() < 147 {
        return Err("PFM file is too short".into());
    }
    let size = le_u32(d, 2).unwrap_or(0) as usize;
    if size != 0 && size > d.len() + 16 {
        return Err("PFM size field does not match the file".into());
    }
    let face_off = le_u32(d, 105).unwrap_or(0) as usize;
    let driver_off = le_u32(d, 139).unwrap_or(0) as usize;
    Ok(Pfm {
        copyright: c_string(&d[6..66], 0).trim().to_string(),
        italic: d[80] != 0,
        weight: le_u16(d, 83).unwrap_or(400),
        charset: d[85],
        pitch_and_family: d[90],
        face_name: if face_off > 0 { c_string(d, face_off) } else { String::new() },
        postscript_name: if driver_off > 0 { c_string(d, driver_off) } else { String::new() },
    })
}

/// Parses an `.afm`.
pub fn parse_afm(text: &str) -> Afm {
    let mut a = Afm::default();
    for line in text.lines() {
        let line = line.trim();
        let (key, value) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let value = value.trim();
        match key {
            "FontName" => a.font_name = value.into(),
            "FullName" => a.full_name = value.into(),
            "FamilyName" => a.family_name = value.into(),
            "Weight" => a.weight = value.into(),
            "ItalicAngle" => a.italic_angle = value.parse().unwrap_or(0.0),
            "IsFixedPitch" => a.is_fixed_pitch = value == "true",
            "Notice" => a.notice = value.into(),
            "Version" => a.version = value.into(),
            "C" | "CH" => {
                if let Some(n) = value.split(';').map(str::trim).find_map(|p| p.strip_prefix("N ")) {
                    a.glyph_names.push(n.trim().to_string());
                }
            }
            _ => {}
        }
    }
    a
}

/// Weight class for a PostScript `/Weight` string.
pub fn weight_from_name(weight: &str) -> u16 {
    let w = weight.to_ascii_lowercase().replace([' ', '-'], "");
    match w.as_str() {
        "thin" | "hairline" => 100,
        "extralight" | "ultralight" => 200,
        "light" => 300,
        "medium" => 500,
        "semibold" | "demibold" | "demi" => 600,
        "bold" => 700,
        "extrabold" | "ultrabold" | "heavy" => 800,
        "black" | "ultra" | "ultrablack" | "extrablack" => 900,
        _ => 400,
    }
}

/// Companion files next to a Type 1 file: (`.pfb`/`.pfa`, `.pfm`, `.afm`) that exist on disk.
pub fn companions(path: &Path) -> (Option<PathBuf>, Option<PathBuf>, Option<PathBuf>) {
    let probe = |exts: &[&str]| -> Option<PathBuf> {
        for e in exts {
            for e in [e.to_string(), e.to_ascii_uppercase()] {
                let p = path.with_extension(&e);
                if p.is_file() {
                    return Some(p);
                }
            }
        }
        None
    };
    (probe(&["pfb", "pfa"]), probe(&["pfm"]), probe(&["afm"]))
}

/// Unicode value of a glyph name (`uniXXXX`, `uXXXX[XX]`, Adobe Glyph List Latin subset).
pub fn glyph_name_to_unicode(name: &str) -> Option<char> {
    let base = name.split('.').next().unwrap_or(name);
    if let Some(h) = base.strip_prefix("uni") {
        if h.len() == 4 {
            return u32::from_str_radix(h, 16).ok().and_then(char::from_u32);
        }
    }
    if let Some(h) = base.strip_prefix('u') {
        if (4..=6).contains(&h.len()) && h.chars().all(|c| c.is_ascii_hexdigit()) {
            return u32::from_str_radix(h, 16).ok().and_then(char::from_u32);
        }
    }
    if base.len() == 1 && base.chars().all(|c| c.is_ascii_alphabetic()) {
        return base.chars().next();
    }
    AGL_SUBSET.iter().find(|(n, _)| *n == base).and_then(|(_, u)| char::from_u32(*u))
}

/// Latin subset of the Adobe Glyph List (names of the standard and ISO Latin-1 encodings).
const AGL_SUBSET: &[(&str, u32)] = &[
    ("space", 0x20), ("exclam", 0x21), ("quotedbl", 0x22), ("numbersign", 0x23), ("dollar", 0x24),
    ("percent", 0x25), ("ampersand", 0x26), ("quotesingle", 0x27), ("quoteright", 0x2019),
    ("parenleft", 0x28), ("parenright", 0x29), ("asterisk", 0x2A), ("plus", 0x2B), ("comma", 0x2C),
    ("hyphen", 0x2D), ("period", 0x2E), ("slash", 0x2F), ("zero", 0x30), ("one", 0x31), ("two", 0x32),
    ("three", 0x33), ("four", 0x34), ("five", 0x35), ("six", 0x36), ("seven", 0x37), ("eight", 0x38),
    ("nine", 0x39), ("colon", 0x3A), ("semicolon", 0x3B), ("less", 0x3C), ("equal", 0x3D),
    ("greater", 0x3E), ("question", 0x3F), ("at", 0x40), ("bracketleft", 0x5B), ("backslash", 0x5C),
    ("bracketright", 0x5D), ("asciicircum", 0x5E), ("underscore", 0x5F), ("grave", 0x60),
    ("quoteleft", 0x2018), ("braceleft", 0x7B), ("bar", 0x7C), ("braceright", 0x7D),
    ("asciitilde", 0x7E), ("exclamdown", 0xA1), ("cent", 0xA2), ("sterling", 0xA3),
    ("currency", 0xA4), ("yen", 0xA5), ("brokenbar", 0xA6), ("section", 0xA7), ("dieresis", 0xA8),
    ("copyright", 0xA9), ("ordfeminine", 0xAA), ("guillemotleft", 0xAB), ("logicalnot", 0xAC),
    ("registered", 0xAE), ("macron", 0xAF), ("degree", 0xB0), ("plusminus", 0xB1),
    ("twosuperior", 0xB2), ("threesuperior", 0xB3), ("acute", 0xB4), ("mu", 0xB5),
    ("paragraph", 0xB6), ("periodcentered", 0xB7), ("cedilla", 0xB8), ("onesuperior", 0xB9),
    ("ordmasculine", 0xBA), ("guillemotright", 0xBB), ("onequarter", 0xBC), ("onehalf", 0xBD),
    ("threequarters", 0xBE), ("questiondown", 0xBF), ("Agrave", 0xC0), ("Aacute", 0xC1),
    ("Acircumflex", 0xC2), ("Atilde", 0xC3), ("Adieresis", 0xC4), ("Aring", 0xC5), ("AE", 0xC6),
    ("Ccedilla", 0xC7), ("Egrave", 0xC8), ("Eacute", 0xC9), ("Ecircumflex", 0xCA),
    ("Edieresis", 0xCB), ("Igrave", 0xCC), ("Iacute", 0xCD), ("Icircumflex", 0xCE),
    ("Idieresis", 0xCF), ("Eth", 0xD0), ("Ntilde", 0xD1), ("Ograve", 0xD2), ("Oacute", 0xD3),
    ("Ocircumflex", 0xD4), ("Otilde", 0xD5), ("Odieresis", 0xD6), ("multiply", 0xD7),
    ("Oslash", 0xD8), ("Ugrave", 0xD9), ("Uacute", 0xDA), ("Ucircumflex", 0xDB),
    ("Udieresis", 0xDC), ("Yacute", 0xDD), ("Thorn", 0xDE), ("germandbls", 0xDF), ("agrave", 0xE0),
    ("aacute", 0xE1), ("acircumflex", 0xE2), ("atilde", 0xE3), ("adieresis", 0xE4), ("aring", 0xE5),
    ("ae", 0xE6), ("ccedilla", 0xE7), ("egrave", 0xE8), ("eacute", 0xE9), ("ecircumflex", 0xEA),
    ("edieresis", 0xEB), ("igrave", 0xEC), ("iacute", 0xED), ("icircumflex", 0xEE),
    ("idieresis", 0xEF), ("eth", 0xF0), ("ntilde", 0xF1), ("ograve", 0xF2), ("oacute", 0xF3),
    ("ocircumflex", 0xF4), ("otilde", 0xF5), ("odieresis", 0xF6), ("divide", 0xF7), ("oslash", 0xF8),
    ("ugrave", 0xF9), ("uacute", 0xFA), ("ucircumflex", 0xFB), ("udieresis", 0xFC), ("yacute", 0xFD),
    ("thorn", 0xFE), ("ydieresis", 0xFF), ("dotlessi", 0x131), ("Lslash", 0x141), ("lslash", 0x142),
    ("OE", 0x152), ("oe", 0x153), ("Scaron", 0x160), ("scaron", 0x161), ("Ydieresis", 0x178),
    ("Zcaron", 0x17D), ("zcaron", 0x17E), ("florin", 0x192), ("circumflex", 0x2C6), ("caron", 0x2C7),
    ("breve", 0x2D8), ("dotaccent", 0x2D9), ("ring", 0x2DA), ("ogonek", 0x2DB), ("tilde", 0x2DC),
    ("hungarumlaut", 0x2DD), ("endash", 0x2013), ("emdash", 0x2014), ("quotesinglbase", 0x201A),
    ("quotedblleft", 0x201C), ("quotedblright", 0x201D), ("quotedblbase", 0x201E), ("dagger", 0x2020),
    ("daggerdbl", 0x2021), ("bullet", 0x2022), ("ellipsis", 0x2026), ("perthousand", 0x2030),
    ("guilsinglleft", 0x2039), ("guilsinglright", 0x203A), ("fraction", 0x2044), ("Euro", 0x20AC),
    ("trademark", 0x2122), ("minus", 0x2212), ("fi", 0xFB01), ("fl", 0xFB02),
];

#[cfg(test)]
pub(crate) mod testutil {
    //! Builds a synthetic `.pfb`.
    use super::*;

    /// A tiny Type 1 font with three glyphs.
    pub fn build_pfb(font_name: &str, family: &str, weight: &str, italic_angle: i32) -> Vec<u8> {
        let clear = format!(
            "%!PS-AdobeFont-1.0: {font_name} 001.000\n11 dict begin\n/FontInfo 10 dict dup begin\n\
             /version (001.000) readonly def\n/Notice (Test notice \\(c\\) AnyFont) readonly def\n\
             /FullName ({family} {weight}) readonly def\n/FamilyName ({family}) readonly def\n\
             /Weight ({weight}) readonly def\n/ItalicAngle {italic_angle} def\n/isFixedPitch false def\nend readonly def\n\
             /FontName /{font_name} def\n/Encoding StandardEncoding def\ncurrentdict end\ncurrentfile eexec\n"
        );
        let mut private = b"dup /Private 8 dict dup begin\n/RD{string currentfile exch readstring pop}executeonly def\n\
                             end\n2 index /CharStrings 3 dict dup begin\n"
            .to_vec();
        for (name, len) in [(".notdef", 4usize), ("A", 6), ("Aacute", 5)] {
            private.extend_from_slice(format!("/{name} {len} RD ").as_bytes());
            private.extend(std::iter::repeat_n(0xAAu8, len));
            private.extend_from_slice(b" ND\n");
        }
        private.extend_from_slice(b"end\nend\nreadonly put\nput\ndup /FontName get exch definefont pop\nmark currentfile closefile\n");
        let enc = eexec_encrypt(&private);
        let trailer = format!("{}cleartomark\n", "0".repeat(64));
        let mut out = Vec::new();
        for (kind, seg) in [(1u8, clear.as_bytes()), (2u8, enc.as_slice()), (1u8, trailer.as_bytes())] {
            out.push(0x80);
            out.push(kind);
            out.extend_from_slice(&(seg.len() as u32).to_le_bytes());
            out.extend_from_slice(seg);
        }
        out.extend_from_slice(&[0x80, 3]);
        out
    }

    /// A tiny `.pfm`.
    pub fn build_pfm(face: &str, ps: &str, weight: u16, italic: bool) -> Vec<u8> {
        let mut d = vec![0u8; 147];
        d[0..2].copy_from_slice(&0x0100u16.to_le_bytes());
        d[6..20].copy_from_slice(b"Copyright test");
        d[80] = italic as u8;
        d[83..85].copy_from_slice(&weight.to_le_bytes());
        d[85] = 0;
        let face_off = d.len() as u32;
        d.extend_from_slice(face.as_bytes());
        d.push(0);
        let ps_off = d.len() as u32;
        d.extend_from_slice(ps.as_bytes());
        d.push(0);
        d[105..109].copy_from_slice(&face_off.to_le_bytes());
        d[139..143].copy_from_slice(&ps_off.to_le_bytes());
        let len = d.len() as u32;
        d[2..6].copy_from_slice(&len.to_le_bytes());
        d
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::*;
    use super::*;

    #[test]
    fn parses_pfb_names_and_glyphs() {
        let pfb = build_pfb("TestSerif-Bold", "Test Serif", "Bold", 0);
        assert!(is_type1(&pfb));
        let f = parse_type1(&pfb).unwrap();
        assert_eq!(f.font_name, "TestSerif-Bold");
        assert_eq!(f.family_name, "Test Serif");
        assert_eq!(f.full_name, "Test Serif Bold");
        assert_eq!(f.weight, "Bold");
        assert_eq!(f.notice, "Test notice (c) AnyFont");
        assert_eq!(f.glyph_names, vec![".notdef", "A", "Aacute"]);
        assert_eq!(weight_from_name(&f.weight), 700);
    }

    #[test]
    fn parses_pfm_and_afm() {
        let pfm = parse_pfm(&build_pfm("Test Serif", "TestSerif-Bold", 700, true)).unwrap();
        assert_eq!(pfm.face_name, "Test Serif");
        assert_eq!(pfm.postscript_name, "TestSerif-Bold");
        assert_eq!(pfm.weight, 700);
        assert!(pfm.italic);
        let afm = parse_afm("StartFontMetrics 4.1\nFontName X-Bold\nFamilyName X\nWeight Bold\nItalicAngle -12\nC 65 ; WX 600 ; N A ; B 0 0 1 1 ;\n");
        assert_eq!(afm.family_name, "X");
        assert_eq!(afm.italic_angle, -12.0);
        assert_eq!(afm.glyph_names, vec!["A"]);
    }

    #[test]
    fn glyph_names_to_unicode() {
        assert_eq!(glyph_name_to_unicode("Aacute"), Some('Á'));
        assert_eq!(glyph_name_to_unicode("uni20AC"), Some('€'));
        assert_eq!(glyph_name_to_unicode("u1F600"), Some('😀'));
        assert_eq!(glyph_name_to_unicode("A.sc"), Some('A'));
        assert_eq!(glyph_name_to_unicode("whatever"), None);
    }
}
