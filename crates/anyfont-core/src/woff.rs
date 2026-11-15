//! WOFF 1.0 decoding (to a plain sfnt) and WOFF 2.0 detection.
//!
//! WOFF 1.0 is cheap to support (zlib per table, `miniz_oxide`), so web fonts in a user folder are listed
//! like any other font. WOFF 2.0 needs Brotli plus the `glyf`/`loca` transform reconstruction; AnyFont
//! recognises the format and reports such files as unsupported (they appear under the "Corrupt" status
//! with the reason "WOFF2 files are not supported"). Windows cannot install either format.

use crate::sfnt::{be_u16, be_u32};

/// Decodes a WOFF 1.0 file into an sfnt byte stream.
pub fn decode_woff(data: &[u8]) -> Result<Vec<u8>, String> {
    if be_u32(data, 0) != Some(crate::sfnt::TAG_WOFF) {
        return Err("not a WOFF file".into());
    }
    let flavor = be_u32(data, 4).ok_or("truncated WOFF header")?;
    let num_tables = be_u16(data, 12).ok_or("truncated WOFF header")? as usize;
    if num_tables == 0 {
        return Err("the font has no tables".into());
    }
    struct Entry {
        tag: [u8; 4],
        offset: usize,
        comp_len: usize,
        orig_len: usize,
        checksum: u32,
    }
    let mut entries = Vec::with_capacity(num_tables);
    for i in 0..num_tables {
        let r = 44 + 20 * i;
        let tag = data.get(r..r + 4).ok_or("truncated WOFF directory")?;
        entries.push(Entry {
            tag: [tag[0], tag[1], tag[2], tag[3]],
            offset: be_u32(data, r + 4).ok_or("truncated WOFF directory")? as usize,
            comp_len: be_u32(data, r + 8).ok_or("truncated WOFF directory")? as usize,
            orig_len: be_u32(data, r + 12).ok_or("truncated WOFF directory")? as usize,
            checksum: be_u32(data, r + 16).ok_or("truncated WOFF directory")?,
        });
    }
    entries.sort_by_key(|e| e.tag);
    let mut out = Vec::new();
    out.extend_from_slice(&flavor.to_be_bytes());
    out.extend_from_slice(&(num_tables as u16).to_be_bytes());
    let mut pow = 1usize;
    let mut log = 0u16;
    while pow * 2 <= num_tables {
        pow *= 2;
        log += 1;
    }
    let search_range = (pow * 16) as u16;
    out.extend_from_slice(&search_range.to_be_bytes());
    out.extend_from_slice(&log.to_be_bytes());
    out.extend_from_slice(&((num_tables * 16) as u16 - search_range).to_be_bytes());
    let dir_start = out.len();
    out.resize(dir_start + 16 * num_tables, 0);
    for (i, e) in entries.iter().enumerate() {
        let src = data.get(e.offset..e.offset + e.comp_len).ok_or("WOFF table outside the file")?;
        let table = if e.comp_len < e.orig_len {
            miniz_oxide::inflate::decompress_to_vec_zlib(src).map_err(|_| "corrupt compressed WOFF table")?
        } else {
            src.to_vec()
        };
        if table.len() != e.orig_len {
            return Err("WOFF table has the wrong length".into());
        }
        while out.len() % 4 != 0 {
            out.push(0);
        }
        let off = out.len();
        out.extend_from_slice(&table);
        let r = dir_start + 16 * i;
        out[r..r + 4].copy_from_slice(&e.tag);
        out[r + 4..r + 8].copy_from_slice(&e.checksum.to_be_bytes());
        out[r + 8..r + 12].copy_from_slice(&(off as u32).to_be_bytes());
        out[r + 12..r + 16].copy_from_slice(&(e.orig_len as u32).to_be_bytes());
    }
    while out.len() % 4 != 0 {
        out.push(0);
    }
    Ok(out)
}

/// Encodes an sfnt as WOFF 1.0 (used by tests to build fixtures; tables are zlib-compressed when that
/// makes them smaller).
pub fn encode_woff(sfnt: &[u8]) -> Result<Vec<u8>, String> {
    let face = crate::sfnt::SfntFace::open(sfnt, 0).map_err(|e| e.to_string())?;
    let n = face.tables.len();
    let mut out = vec![0u8; 44 + 20 * n];
    out[0..4].copy_from_slice(&crate::sfnt::TAG_WOFF.to_be_bytes());
    out[4..8].copy_from_slice(&face.sfnt_version.to_be_bytes());
    out[12..14].copy_from_slice(&(n as u16).to_be_bytes());
    for (i, t) in face.tables.iter().enumerate() {
        let data = &sfnt[t.offset as usize..(t.offset + t.length) as usize];
        let comp = miniz_oxide::deflate::compress_to_vec_zlib(data, 6);
        let stored: &[u8] = if comp.len() < data.len() { &comp } else { data };
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }
        let off = out.len();
        out.extend_from_slice(stored);
        let r = 44 + 20 * i;
        out[r..r + 4].copy_from_slice(&t.tag);
        out[r + 4..r + 8].copy_from_slice(&(off as u32).to_be_bytes());
        out[r + 8..r + 12].copy_from_slice(&(stored.len() as u32).to_be_bytes());
        out[r + 12..r + 16].copy_from_slice(&t.length.to_be_bytes());
        out[r + 16..r + 20].copy_from_slice(&t.checksum.to_be_bytes());
    }
    let len = out.len() as u32;
    out[8..12].copy_from_slice(&len.to_be_bytes());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sfnt::testutil::*;

    #[test]
    fn woff_round_trip() {
        let sfnt = build_sfnt(0x0001_0000, &valid_tables());
        let woff = encode_woff(&sfnt).unwrap();
        let back = decode_woff(&woff).unwrap();
        let face = crate::sfnt::SfntFace::open(&back, 0).unwrap();
        assert!(face.validate().is_ok());
        assert_eq!(face.table(b"name"), crate::sfnt::SfntFace::open(&sfnt, 0).unwrap().table(b"name"));
        assert!(decode_woff(b"nope").is_err());
    }
}
