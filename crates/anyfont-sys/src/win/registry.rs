//! Font registry keys (HKLM / HKCU `...\Fonts` and `...\Type 1 Installer\Type 1 Fonts`).
//! Access masks as in the original: read `0x20119` (KEY_READ | KEY_WOW64_64KEY), write `0x20106`
//! (KEY_WRITE | KEY_WOW64_64KEY), create `0xF013F` for the Type 1 key (CdrFnt 0x1802010F0).

use std::io;

use windows::core::{HSTRING, PCWSTR, PWSTR};
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_NO_MORE_ITEMS, ERROR_SUCCESS, WIN32_ERROR};
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegEnumValueW, RegOpenKeyExW, RegQueryInfoKeyW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY, KEY_WRITE, REG_EXPAND_SZ, REG_MULTI_SZ, REG_OPTION_NON_VOLATILE,
    REG_SZ, REG_VALUE_TYPE,
};

use crate::engine::RegData;
use crate::regname::{FONTS_KEY, TYPE1_KEY};
use crate::{RegKind, Scope};

fn root(scope: Scope) -> HKEY {
    match scope {
        Scope::AllUsers => HKEY_LOCAL_MACHINE,
        _ => HKEY_CURRENT_USER,
    }
}

fn subkey(kind: RegKind) -> &'static str {
    match kind {
        RegKind::Type1 => TYPE1_KEY,
        _ => FONTS_KEY,
    }
}

fn err(e: WIN32_ERROR) -> io::Error {
    io::Error::from_raw_os_error(e.0 as i32)
}

struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

fn open(scope: Scope, kind: RegKind, write: bool) -> io::Result<Option<Key>> {
    let mut h = HKEY::default();
    let sam = if write { KEY_WRITE | KEY_WOW64_64KEY } else { KEY_READ | KEY_WOW64_64KEY };
    let r = unsafe { RegOpenKeyExW(root(scope), &HSTRING::from(subkey(kind)), None, sam, &mut h) };
    if r == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    if r != ERROR_SUCCESS {
        return Err(err(r));
    }
    Ok(Some(Key(h)))
}

fn utf16_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

fn multi_sz(buf: &[u16]) -> Vec<String> {
    buf.split(|c| *c == 0).map(String::from_utf16_lossy).filter(|s| !s.is_empty()).collect()
}

pub(crate) fn list(scope: Scope, kind: RegKind) -> io::Result<Vec<(String, RegData)>> {
    let Some(key) = open(scope, kind, false)? else { return Ok(Vec::new()) };
    let (mut max_name, mut max_data) = (0u32, 0u32);
    let r = unsafe {
        RegQueryInfoKeyW(key.0, None, None, None, None, None, None, None, Some(&mut max_name), Some(&mut max_data), None, None)
    };
    if r != ERROR_SUCCESS {
        return Err(err(r));
    }
    let mut name_buf = vec![0u16; max_name as usize + 2];
    let mut data_buf = vec![0u8; max_data as usize + 4];
    let mut out = Vec::new();
    let mut index = 0u32;
    loop {
        let mut name_len = name_buf.len() as u32;
        let mut data_len = data_buf.len() as u32;
        let mut ty = 0u32;
        let r = unsafe {
            RegEnumValueW(
                key.0,
                index,
                Some(PWSTR(name_buf.as_mut_ptr())),
                &mut name_len,
                None,
                Some(&mut ty),
                Some(data_buf.as_mut_ptr()),
                Some(&mut data_len),
            )
        };
        if r == ERROR_NO_MORE_ITEMS {
            break;
        }
        index += 1;
        if r != ERROR_SUCCESS {
            // e.g. a value grew between the size query and the read: skip it.
            continue;
        }
        let name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
        let words: Vec<u16> = data_buf[..data_len as usize].as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
        let ty = REG_VALUE_TYPE(ty);
        let data = if ty == REG_MULTI_SZ {
            RegData::MultiSz(multi_sz(&words))
        } else if ty == REG_SZ || ty == REG_EXPAND_SZ {
            RegData::Sz(utf16_to_string(&words))
        } else {
            continue;
        };
        out.push((name, data));
    }
    Ok(out)
}

pub(crate) fn set(scope: Scope, kind: RegKind, name: &str, data: &RegData) -> io::Result<()> {
    crate::win::forbid_in_tests!("write the font registry");
    let mut h = HKEY::default();
    let r = unsafe {
        RegCreateKeyExW(
            root(scope),
            &HSTRING::from(subkey(kind)),
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE | KEY_WOW64_64KEY,
            None,
            &mut h,
            None,
        )
    };
    if r != ERROR_SUCCESS {
        return Err(err(r));
    }
    let key = Key(h);
    let (ty, words): (REG_VALUE_TYPE, Vec<u16>) = match data {
        RegData::Sz(s) => (REG_SZ, s.encode_utf16().chain([0]).collect()),
        RegData::MultiSz(items) => {
            let mut w: Vec<u16> = Vec::new();
            for it in items {
                w.extend(it.encode_utf16());
                w.push(0);
            }
            w.push(0);
            (REG_MULTI_SZ, w)
        }
    };
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    let r = unsafe { RegSetValueExW(key.0, &HSTRING::from(name), None, ty, Some(&bytes)) };
    if r != ERROR_SUCCESS {
        return Err(err(r));
    }
    Ok(())
}

pub(crate) fn delete(scope: Scope, kind: RegKind, name: &str) -> io::Result<bool> {
    crate::win::forbid_in_tests!("delete a font registry value");
    let Some(key) = open(scope, kind, true)? else { return Ok(false) };
    let r = unsafe { RegDeleteValueW(key.0, &HSTRING::from(name)) };
    if r == ERROR_FILE_NOT_FOUND {
        return Ok(false);
    }
    if r != ERROR_SUCCESS {
        return Err(err(r));
    }
    Ok(true)
}
