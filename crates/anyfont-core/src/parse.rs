//! Turning font files into [`FontRecord`]s.
//!
//! The equivalent of `DumpFontInformationFromFontDataForDatabase` (engine 0x18004E530): one record per
//! face, one record per named instance for variable fonts (the original lists Bahnschrift's instances such
//! as "Light Condensed" as separate fonts and stores their coordinates in `FONTVARIABLEAXES`).
//!
//! Supported inputs: `.ttf .otf .ttc .otc` (sfnt, validated by [`crate::sfnt`]), `.woff` (decoded),
//! `.woff2` (recognised, reported unsupported), `.pfb .pfa` (+ `.pfm`/`.afm` companions), a lone `.pfm`.
//! Nothing here panics on bad input: a file that cannot be read yields a single record whose
//! [`FontRecord::corrupt`] holds the reason.

use crate::model::*;
use crate::sfnt::{self, NameTable, Os2, SfntFace};
use crate::type1;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// File extensions scanned in folders (lower case).
pub const FONT_EXTENSIONS: &[&str] = &["ttf", "otf", "ttc", "otc", "woff", "woff2", "pfb", "pfa", "pfm"];

/// True when the path has a font extension.
pub fn is_font_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| FONT_EXTENSIONS.iter().any(|x| x.eq_ignore_ascii_case(e)))
        .unwrap_or(false)
}

/// Result of parsing one file.
#[derive(Clone, Debug, Default)]
pub struct ParsedFile {
    /// Container format.
    pub format: FileFormat,
    /// SHA-256 of the content (hex).
    pub sha: String,
    /// Faces (at least one; a corrupt file yields one corrupt record).
    pub faces: Vec<FontRecord>,
}

/// SHA-256 of bytes as lower-case hex.
pub fn sha256_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    let mut s = String::with_capacity(64);
    for b in digest.iter() {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Reads and parses a font file, filling path, size, modification time and Type 1 companions.
pub fn parse_font_file(path: &Path) -> ParsedFile {
    let meta = std::fs::metadata(path).ok();
    let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
    let modified = meta.as_ref().map(file_mtime).unwrap_or(0);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let mut parsed = match std::fs::read(path) {
        Ok(data) => {
            if ext == "pfm" {
                parse_lone_pfm(&data, path)
            } else {
                let mut p = parse_font_bytes(&data);
                if matches!(p.format, FileFormat::Type1Binary | FileFormat::Type1Ascii) {
                    merge_type1_companions(&mut p, path);
                }
                p
            }
        }
        Err(e) => corrupt_file(String::new(), FileFormat::Unknown, format!("cannot read the file: {e}")),
    };
    for f in &mut parsed.faces {
        f.paths = vec![path.to_path_buf()];
        f.file_size = size;
        f.modified = modified;
        if f.family.is_empty() {
            f.family = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        }
    }
    parsed
}

/// File modification time in seconds since the Unix epoch.
pub fn file_mtime(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Parses font bytes (no path information).
pub fn parse_font_bytes(data: &[u8]) -> ParsedFile {
    let sha = sha256_hex(data);
    match sfnt::be_u32(data, 0) {
        Some(sfnt::TAG_WOFF) => match crate::woff::decode_woff(data) {
            Ok(decoded) => {
                let mut p = parse_sfnt(&decoded, sha);
                p.format = FileFormat::Woff;
                for f in &mut p.faces {
                    f.format = FileFormat::Woff;
                }
                p
            }
            Err(e) => corrupt_file(sha, FileFormat::Woff, e),
        },
        Some(sfnt::TAG_WOFF2) => corrupt_file(sha, FileFormat::Woff2, "WOFF2 files are not supported".into()),
        _ if sfnt::is_sfnt(data) => parse_sfnt(data, sha),
        _ if type1::is_type1(data) => parse_type1_bytes(data, sha),
        _ => corrupt_file(sha, FileFormat::Unknown, "not a font file".into()),
    }
}

fn corrupt_file(sha: String, format: FileFormat, reason: String) -> ParsedFile {
    let rec = FontRecord {
        key: FontKey::new(&sha, 0, None),
        sha: sha.clone(),
        face_count: 1,
        format,
        technology: Technology::Unknown,
        outline: Outline::None,
        corrupt: Some(reason),
        width_class: 5,
        weight_class: 400,
        ..Default::default()
    };
    ParsedFile { format, sha, faces: vec![rec] }
}

fn parse_sfnt(data: &[u8], sha: String) -> ParsedFile {
    let count = sfnt::face_count(data).max(1);
    let is_collection = sfnt::be_u32(data, 0) == Some(sfnt::TAG_TTCF);
    let mut faces = Vec::new();
    let mut any_cff = false;
    for index in 0..count.min(256) {
        match build_sfnt_records(data, index, count, &sha) {
            Ok(mut recs) => {
                any_cff |= recs.iter().any(|r| r.outline == Outline::Cff || r.outline == Outline::Cff2);
                faces.append(&mut recs);
            }
            Err(reason) => {
                let mut f = corrupt_file(sha.clone(), FileFormat::Unknown, reason).faces.remove(0);
                f.face_index = index;
                f.face_count = count;
                f.key = FontKey::new(&sha, index, None);
                faces.push(f);
            }
        }
    }
    let format = match (is_collection, any_cff || sfnt::be_u32(data, 0) == Some(sfnt::TAG_OTTO)) {
        (true, true) => FileFormat::OpenTypeCollection,
        (true, false) => FileFormat::TrueTypeCollection,
        (false, true) => FileFormat::OpenTypeCff,
        (false, false) => FileFormat::TrueType,
    };
    for f in &mut faces {
        f.format = format;
    }
    ParsedFile { format, sha, faces }
}

fn width_class_from_wdth(wdth: f32) -> u16 {
    const STEPS: [f32; 9] = [50.0, 62.5, 75.0, 87.5, 100.0, 112.5, 125.0, 150.0, 200.0];
    let mut best = 4;
    let mut best_d = f32::MAX;
    for (i, s) in STEPS.iter().enumerate() {
        let d = (wdth - s).abs();
        if d < best_d {
            best_d = d;
            best = i;
        }
    }
    best as u16 + 1
}

fn build_sfnt_records(data: &[u8], index: u32, count: u32, sha: &str) -> Result<Vec<FontRecord>, String> {
    let face = SfntFace::open(data, index).map_err(|e| e.to_string())?;
    let (flavour, warnings) = face.validate().map_err(|e| e.to_string())?;
    let tt = ttf_parser::Face::parse(data, index).map_err(|e| format!("cannot parse the font: {e}"))?;
    let names = face.table(b"name").map(NameTable::parse).unwrap_or_default();
    let os2 = face.table(b"OS/2").and_then(Os2::parse);
    let head = face.table(b"head").unwrap_or(&[]);
    let mac_style = sfnt::be_u16(head, 44).unwrap_or(0);
    let units_per_em = sfnt::be_u16(head, 18).unwrap_or(1000);
    let en = |id: u16| names.english(id).unwrap_or("").trim().to_string();
    let opt = |id: u16| names.english(id).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());

    let outline = if face.has(b"CFF2") {
        Outline::Cff2
    } else if face.has(b"CFF ") {
        Outline::Cff
    } else if face.has(b"glyf") {
        Outline::TrueType
    } else {
        Outline::Bitmap
    };
    let technology = match flavour {
        sfnt::Flavour::Cff => Technology::OpenTypePostScript,
        sfnt::Flavour::TrueType if face.has(b"DSIG") => Technology::OpenTypeTrueType,
        sfnt::Flavour::TrueType => Technology::TrueType,
    };
    let mut color = 0u8;
    for (tag, bit) in [
        (b"COLR", ColorFormats::COLR),
        (b"CPAL", ColorFormats::CPAL),
        (b"SVG ", ColorFormats::SVG),
        (b"sbix", ColorFormats::SBIX),
        (b"CBDT", ColorFormats::CBDT),
    ] {
        if face.has(tag) {
            color |= bit;
        }
    }

    // OpenType features (GSUB + GPOS), with script/language triples.
    let mut feature_records = Vec::new();
    for table in [tt.tables().gsub, tt.tables().gpos].into_iter().flatten() {
        for script in table.scripts {
            let script_tag = script.tag.to_string();
            let mut systems: Vec<(String, ttf_parser::opentype_layout::LanguageSystem)> = Vec::new();
            if let Some(d) = script.default_language {
                systems.push(("dflt".into(), d));
            }
            for l in script.languages {
                systems.push((l.tag.to_string(), l));
            }
            for (lang, sys) in systems {
                let idx = sys.feature_indices.into_iter().chain(sys.required_feature);
                for fi in idx {
                    if let Some(f) = table.features.get(fi) {
                        feature_records.push(FeatureRecord {
                            feature: f.tag.to_string(),
                            script: script_tag.clone(),
                            language: lang.clone(),
                        });
                    }
                }
            }
        }
        // features not referenced by any script still count as supported
        for f in table.features {
            if !feature_records.iter().any(|r| r.feature == f.tag.to_string()) {
                feature_records.push(FeatureRecord {
                    feature: f.tag.to_string(),
                    script: "DFLT".into(),
                    language: "dflt".into(),
                });
            }
        }
    }
    feature_records.sort();
    feature_records.dedup();
    let mut features: Vec<String> = feature_records.iter().map(|r| r.feature.clone()).collect();
    features.sort();
    features.dedup();

    let symbol = tt
        .tables()
        .cmap
        .map(|c| c.subtables.into_iter().any(|s| s.platform_id == ttf_parser::PlatformId::Windows && s.encoding_id == 0))
        .unwrap_or(false);
    let vertical = face.has(b"vhea") || features.iter().any(|f| f == "vert" || f == "vrt2");
    let monospaced = face.table(b"post").and_then(|p| sfnt::be_u32(p, 12)).map(|v| v != 0).unwrap_or(false);

    let (weight_class, width_class, fs_type, os2_version, fs_selection, panose, unicode_range, code_page_range, vendor) =
        match &os2 {
            Some(o) => (
                o.weight_class,
                o.width_class,
                o.fs_type,
                o.version,
                o.fs_selection,
                o.panose,
                o.unicode_range,
                o.code_page_range,
                o.vendor(),
            ),
            None => (
                if mac_style & 1 != 0 { 700 } else { 400 },
                5,
                0,
                0,
                0,
                [0; 10],
                [0; 4],
                [0; 2],
                String::new(),
            ),
        };
    let italic = fs_selection & 1 != 0 || mac_style & 2 != 0;
    let oblique = fs_selection & (1 << 9) != 0;

    // localised names: every Windows language other than English
    let mut localized_names = Vec::new();
    for lang in names.windows_languages() {
        if lang & 0x3FF == 0x09 {
            continue;
        }
        let w = |id| names.windows(id, lang).map(|s| s.trim().to_string());
        let n = LocalizedName {
            language: lang,
            family: w(1),
            subfamily: w(2),
            full_name: w(4),
            typographic_family: w(16),
            typographic_subfamily: w(17),
        };
        if n.family.is_some() || n.typographic_family.is_some() || n.full_name.is_some() {
            localized_names.push(n);
        }
    }

    let family = en(1);
    let base = FontRecord {
        key: FontKey::new(sha, index, None),
        sha: sha.to_string(),
        face_index: index,
        face_count: count,
        family: if family.is_empty() { en(16) } else { family },
        subfamily: en(2),
        typographic_family: opt(16),
        typographic_subfamily: opt(17),
        full_name: en(4),
        postscript_name: en(6),
        version: en(5),
        unique_id: en(3),
        copyright: en(0),
        trademark: en(7),
        manufacturer: en(8),
        designer: en(9),
        description: en(10),
        vendor_url: en(11),
        designer_url: en(12),
        license: en(13),
        license_url: en(14),
        sample_text: en(19),
        localized_names,
        technology,
        outline,
        is_variable: face.has(b"fvar"),
        color: ColorFormats(color),
        weight_class: if weight_class == 0 { 400 } else { weight_class },
        width_class: if (1..=9).contains(&width_class) { width_class } else { 5 },
        italic,
        oblique,
        fs_selection,
        mac_style,
        fs_type,
        embedding: Embedding::from_fs_type(fs_type, os2_version),
        code_page_range,
        unicode_range,
        panose: Panose(panose),
        vendor_id: vendor,
        symbol,
        vertical,
        monospaced,
        glyph_count: tt.number_of_glyphs() as u32,
        units_per_em,
        features,
        feature_records,
        warnings,
        ..Default::default()
    };

    if !base.is_variable {
        return Ok(vec![base]);
    }
    let fvar = face.table(b"fvar").map(sfnt::Fvar::parse).unwrap_or_default();
    let stat_names = face.table(b"STAT").map(sfnt::stat_axes).unwrap_or_default();
    let axes: Vec<VariationAxis> = fvar
        .axes
        .iter()
        .map(|a| {
            let name = names
                .english(a.name_id)
                .map(str::to_string)
                .or_else(|| stat_names.iter().find(|(t, _)| *t == a.tag).and_then(|(_, id)| names.english(*id)).map(str::to_string))
                .unwrap_or_else(|| sfnt::tag_str(&a.tag));
            VariationAxis {
                tag: sfnt::tag_str(&a.tag),
                name,
                min: a.min,
                default: a.default,
                max: a.max,
                value: a.default,
                hidden: a.flags & 1 != 0,
            }
        })
        .collect();
    let instances: Vec<NamedInstance> = fvar
        .instances
        .iter()
        .map(|i| NamedInstance {
            subfamily: names.english(i.subfamily_name_id).unwrap_or("").trim().to_string(),
            postscript_name: i.postscript_name_id.and_then(|id| names.english(id)).map(|s| s.trim().to_string()),
            coordinates: i.coordinates.clone(),
        })
        .collect();
    if instances.is_empty() {
        let mut r = base;
        r.axes = axes;
        return Ok(vec![r]);
    }
    let group = base.group_family().to_string();
    let mut out = Vec::with_capacity(instances.len());
    for (n, inst) in instances.iter().enumerate() {
        let mut r = base.clone();
        r.key = FontKey::new(sha, index, Some(n as u16));
        r.instance_index = Some(n as u16);
        r.axes = axes
            .iter()
            .zip(inst.coordinates.iter().chain(std::iter::repeat(&f32::NAN)))
            .map(|(a, &v)| VariationAxis { value: if v.is_nan() { a.default } else { v }, ..a.clone() })
            .collect();
        r.instances = instances.clone();
        if !inst.subfamily.is_empty() {
            r.subfamily = inst.subfamily.clone();
            r.typographic_subfamily = Some(inst.subfamily.clone());
            r.full_name = format!("{group} {}", inst.subfamily);
        }
        r.postscript_name = inst.postscript_name.clone().unwrap_or_else(|| {
            let ps_family = base.postscript_name.split('-').next().unwrap_or(&group).replace(' ', "");
            format!("{ps_family}-{}", inst.subfamily.replace(' ', ""))
        });
        for a in &r.axes {
            match a.tag.as_str() {
                "wght" => r.weight_class = a.value.round().clamp(1.0, 1000.0) as u16,
                "wdth" => r.width_class = width_class_from_wdth(a.value),
                "ital" => r.italic = a.value >= 0.5,
                "slnt" => r.italic = r.italic || a.value.abs() > 0.01,
                _ => {}
            }
        }
        let lower = inst.subfamily.to_ascii_lowercase();
        if lower.contains("italic") || lower.contains("oblique") {
            r.italic = true;
        }
        // localised instance names are not in fvar; keep only the family names
        for l in &mut r.localized_names {
            l.subfamily = None;
            l.full_name = None;
            l.typographic_subfamily = None;
        }
        out.push(r);
    }
    Ok(out)
}

fn parse_type1_bytes(data: &[u8], sha: String) -> ParsedFile {
    let format = if data.first() == Some(&0x80) { FileFormat::Type1Binary } else { FileFormat::Type1Ascii };
    match type1::parse_type1(data) {
        Ok(t) => {
            let weight = type1::weight_from_name(&t.weight);
            let italic = t.italic_angle.abs() > 0.01;
            let rec = FontRecord {
                key: FontKey::new(&sha, 0, None),
                sha: sha.clone(),
                face_count: 1,
                format,
                family: t.family_name.clone(),
                subfamily: ribbi(weight, italic).into(),
                full_name: t.full_name.clone(),
                postscript_name: t.font_name.clone(),
                version: if t.version.is_empty() || t.version.starts_with("Version") {
                    t.version.clone()
                } else {
                    format!("Version {}", t.version)
                },
                copyright: if t.copyright.is_empty() { t.notice.clone() } else { t.copyright.clone() },
                trademark: if t.copyright.is_empty() { String::new() } else { t.notice.clone() },
                technology: Technology::Type1,
                outline: Outline::Type1,
                weight_class: weight,
                width_class: 5,
                italic,
                embedding: Embedding::Installable,
                code_page_range: [1, 0],
                unicode_range: [1, 0, 0, 0],
                monospaced: t.is_fixed_pitch,
                glyph_count: t.glyph_names.len() as u32,
                units_per_em: 1000,
                ..Default::default()
            };
            ParsedFile { format, sha, faces: vec![rec] }
        }
        Err(e) => corrupt_file(sha, format, e),
    }
}

fn ribbi(weight: u16, italic: bool) -> &'static str {
    match (weight >= 600, italic) {
        (false, false) => "Regular",
        (true, false) => "Bold",
        (false, true) => "Italic",
        (true, true) => "Bold Italic",
    }
}

/// Merges `.pfm` / `.afm` information found next to a Type 1 outline file.
fn merge_type1_companions(p: &mut ParsedFile, path: &Path) {
    let (_, pfm, afm) = type1::companions(path);
    let Some(rec) = p.faces.first_mut() else { return };
    if rec.corrupt.is_some() {
        return;
    }
    if let Some(afm) = afm.and_then(|a| std::fs::read_to_string(a).ok()).map(|t| type1::parse_afm(&t)) {
        if !afm.family_name.is_empty() {
            rec.family = afm.family_name.clone();
        }
        if rec.glyph_count == 0 {
            rec.glyph_count = afm.glyph_names.len() as u32;
        }
    }
    if let Some(pfm) = pfm.and_then(|p| std::fs::read(p).ok()).and_then(|d| type1::parse_pfm(&d).ok()) {
        apply_pfm(rec, &pfm);
    }
}

fn apply_pfm(rec: &mut FontRecord, pfm: &type1::Pfm) {
    if !pfm.face_name.is_empty() {
        rec.family = pfm.face_name.clone();
    }
    if (1..=1000).contains(&pfm.weight) {
        rec.weight_class = pfm.weight;
    }
    rec.italic = rec.italic || pfm.italic;
    rec.subfamily = ribbi(rec.weight_class, rec.italic).into();
    if pfm.charset == 2 {
        rec.symbol = true;
        rec.code_page_range = [1 << 31, 0];
    }
    if rec.postscript_name.is_empty() {
        rec.postscript_name = pfm.postscript_name.clone();
    }
    if rec.copyright.is_empty() {
        rec.copyright = pfm.copyright.clone();
    }
}

fn parse_lone_pfm(data: &[u8], path: &Path) -> ParsedFile {
    let (outline, _, _) = type1::companions(path);
    if let Some(outline) = outline {
        // a .pfm with its outline file: parse the outline (the scanner normally passes the .pfb itself)
        let mut p = std::fs::read(&outline).map(|d| parse_font_bytes(&d)).unwrap_or_default();
        if !p.faces.is_empty() {
            merge_type1_companions(&mut p, &outline);
            return p;
        }
    }
    let sha = sha256_hex(data);
    match type1::parse_pfm(data) {
        Ok(pfm) => {
            let mut rec = corrupt_file(sha.clone(), FileFormat::Type1Ascii, "Type 1 outline file (.pfb) is missing".into())
                .faces
                .remove(0);
            rec.technology = Technology::Type1;
            rec.outline = Outline::None;
            rec.postscript_name = pfm.postscript_name.clone();
            rec.full_name = pfm.face_name.clone();
            apply_pfm(&mut rec, &pfm);
            ParsedFile { format: FileFormat::Type1Ascii, sha, faces: vec![rec] }
        }
        Err(e) => corrupt_file(sha, FileFormat::Type1Ascii, e),
    }
}

/// For a Type 1 path, the outline file that represents the font (`.pfm` -> sibling `.pfb`/`.pfa`).
pub fn canonical_font_path(path: &Path) -> PathBuf {
    let is_pfm = path.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("pfm")).unwrap_or(false);
    if is_pfm {
        if let (Some(outline), _, _) = type1::companions(path) {
            return outline;
        }
    }
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sfnt::testutil::*;

    #[test]
    fn corrupt_bytes_never_panic() {
        for data in [&b""[..], b"\x00\x01\x00\x00", b"wOF2xxxx", b"hello world", &[0x80, 0x01, 0xff, 0xff, 0xff, 0x7f]] {
            let p = parse_font_bytes(data);
            assert_eq!(p.faces.len(), 1);
            assert!(p.faces[0].corrupt.is_some());
        }
    }

    #[test]
    fn synthetic_font_is_rejected_by_ttf_parser_or_parsed() {
        // structurally valid for our rules; ttf-parser needs hhea data, which is zeros here
        let data = build_sfnt(0x0001_0000, &valid_tables());
        let p = parse_font_bytes(&data);
        assert_eq!(p.faces.len(), 1);
    }

    #[test]
    fn type1_record() {
        let pfb = crate::type1::testutil::build_pfb("TestSerif-BoldItalic", "Test Serif", "Bold", -12);
        let p = parse_font_bytes(&pfb);
        let r = &p.faces[0];
        assert!(r.corrupt.is_none(), "{:?}", r.corrupt);
        assert_eq!(r.technology, Technology::Type1);
        assert_eq!(r.family, "Test Serif");
        assert_eq!(r.subfamily, "Bold Italic");
        assert_eq!(r.weight_class, 700);
        assert!(r.italic);
        assert_eq!(r.glyph_count, 3);
        assert_eq!(r.version, "Version 001.000");
    }

    #[test]
    fn wdth_mapping() {
        assert_eq!(width_class_from_wdth(75.0), 3);
        assert_eq!(width_class_from_wdth(100.0), 5);
        assert_eq!(width_class_from_wdth(200.0), 9);
    }
}
