//! Duplicate detection.
//!
//! The original (`CFontDatabase` duplicate pass, `DBDuplicate` column) considers two faces duplicates when
//! they have the same default family, default sub-family, technology, weight, width and italic flag but come
//! from different files (identical files are already one row because rows are keyed by content hash).
//! Versions may differ. One face of each set is the master (`DBDuplicate = 1`, the one listed when
//! "Show duplicate fonts" is off: `DBDuplicate < 2`), the others are copies (`DBDuplicate = 2`).
//!
//! Master choice: installed first, then the higher version string, then the most recently modified file,
//! then the lowest id (stable).

use crate::model::{DuplicateState, FontRecord};
use std::collections::HashMap;

/// The grouping key of a face (case-insensitive names).
pub fn duplicate_key(f: &FontRecord) -> Option<String> {
    if f.corrupt.is_some() || f.needs_download() {
        return None;
    }
    let sub = f.typographic_subfamily.as_deref().filter(|s| !s.is_empty()).unwrap_or(&f.subfamily);
    Some(format!(
        "{}\u{1}{}\u{1}{}\u{1}{}\u{1}{}\u{1}{}",
        f.group_family().to_lowercase(),
        sub.to_lowercase(),
        f.technology.db_name(),
        f.weight_class,
        f.width_class,
        f.italic as u8
    ))
}

fn version_number(v: &str) -> Vec<u32> {
    v.split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty()).take(4).map(|s| s.parse().unwrap_or(0)).collect()
}

/// Computes the duplicate state of every face (index-aligned with `fonts`).
pub fn compute(fonts: &[FontRecord]) -> Vec<DuplicateState> {
    let refs: Vec<&FontRecord> = fonts.iter().collect();
    compute_refs(&refs)
}

/// [`compute`] over borrowed faces (used per library by the catalog).
pub fn compute_refs(fonts: &[&FontRecord]) -> Vec<DuplicateState> {
    let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, f) in fonts.iter().enumerate() {
        if let Some(k) = duplicate_key(f) {
            groups.entry(k).or_default().push(i);
        }
    }
    let mut out = vec![DuplicateState::Unique; fonts.len()];
    for idx in groups.values() {
        // faces of one file never duplicate each other (variable instances share family/style rarely)
        let distinct_files = {
            let mut shas: Vec<&str> = idx.iter().map(|&i| fonts[i].sha.as_str()).collect();
            shas.sort();
            shas.dedup();
            shas.len()
        };
        if idx.len() < 2 || distinct_files < 2 {
            continue;
        }
        let master = *idx
            .iter()
            .max_by(|&&a, &&b| {
                let fa = fonts[a];
                let fb = fonts[b];
                fa.installed
                    .cmp(&fb.installed)
                    .then_with(|| version_number(&fa.version).cmp(&version_number(&fb.version)))
                    .then_with(|| fa.modified.cmp(&fb.modified))
                    .then_with(|| fb.id.cmp(&fa.id))
            })
            .unwrap_or(&idx[0]);
        for &i in idx {
            out[i] = if i == master { DuplicateState::Master } else { DuplicateState::Copy };
        }
    }
    out
}

/// Applies [`compute`] to the records in place.
pub fn mark(fonts: &mut [FontRecord]) {
    let states = compute(fonts);
    for (f, s) in fonts.iter_mut().zip(states) {
        f.duplicate = s;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn f(id: i64, sha: &str, family: &str, sub: &str, version: &str, installed: bool) -> FontRecord {
        FontRecord {
            id,
            sha: sha.into(),
            family: family.into(),
            subfamily: sub.into(),
            version: version.into(),
            installed,
            weight_class: 400,
            width_class: 5,
            paths: vec![format!("C:/x/{id}.ttf").into()],
            ..Default::default()
        }
    }

    #[test]
    fn same_family_style_in_different_files() {
        let fonts = vec![
            f(1, "a", "Arial", "Regular", "Version 1.0", false),
            f(2, "b", "arial", "Regular", "Version 2.0", true),
            f(3, "c", "Arial", "Bold", "Version 1.0", false),
            f(4, "d", "Arial", "Regular", "Version 3.0", false),
        ];
        let s = compute(&fonts);
        assert_eq!(s[1], DuplicateState::Master, "installed copy wins");
        assert_eq!(s[0], DuplicateState::Copy);
        assert_eq!(s[3], DuplicateState::Copy);
        assert_eq!(s[2], DuplicateState::Unique);
        let mut uninstalled = fonts.clone();
        uninstalled[1].installed = false;
        assert_eq!(compute(&uninstalled)[3], DuplicateState::Master, "then the highest version");
    }

    #[test]
    fn same_file_or_corrupt_is_not_duplicate() {
        let mut a = f(1, "a", "X", "Regular", "", false);
        let b = f(2, "a", "X", "Regular", "", false);
        assert_eq!(compute(&[a.clone(), b.clone()]), vec![DuplicateState::Unique; 2]);
        a.sha = "z".into();
        a.corrupt = Some("bad".into());
        assert_eq!(compute(&[a, b]), vec![DuplicateState::Unique; 2]);
    }
}
