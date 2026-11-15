//! Command line parsing (dossier `10-behavior/command-line.md`).
//!
//! Grammar (`CrlUtils::CommandLine::ParseSwitch`, 0x180011C10):
//! - a switch starts with `/` or `-`; any other argument is a file name;
//! - an optional value follows the first colon: `/NoSplash:1` -> switch `NoSplash`, value `1`;
//! - the value is trimmed of `"`;
//! - switch names are looked up case-insensitively (`SwitchSpecified`, 0x180010D00).
//!
//! Recognised switches: the launcher's `NoSplash`, `NoStartupInitialAction`, `NoUI`, `DDE`,
//! `Automation`, `Embedding`, `Print`, `P`, `Sdi`, `Mdi` (kept for compatibility; only `NoSplash`,
//! `NoStartupInitialAction` and `NoUI` change AnyFont's behaviour), plus AnyFont's own
//! `InstallHelper:<request.json>` (elevated helper, see [`crate::helper`]) and `Lang:<code>`.
//! Unknown switches are accepted and ignored. Every non-switch argument is a font file (or folder)
//! to open.

use std::ffi::OsString;
use std::path::PathBuf;

/// Switch used to start the elevated install helper (`anyfont.exe /InstallHelper:<file>`).
pub const INSTALL_HELPER_SWITCH: &str = "InstallHelper";
/// Switch that selects the UI language (`/Lang:pt-BR`).
pub const LANG_SWITCH: &str = "Lang";

/// Launcher switches of the original program (all accepted).
pub const KNOWN_SWITCHES: &[&str] = &[
    "NoSplash",
    "NoStartupInitialAction",
    "NoUI",
    "DDE",
    "Automation",
    "Embedding",
    "Print",
    "P",
    "Sdi",
    "Mdi",
    INSTALL_HELPER_SWITCH,
    LANG_SWITCH,
];

/// One parsed switch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Switch {
    /// Name as written (without the leading `/` or `-`).
    pub name: String,
    /// Value after the first `:` with `"` trimmed; `None` when there was no colon.
    pub value: Option<String>,
}

/// The parsed command line.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandLine {
    pub switches: Vec<Switch>,
    /// Non-switch arguments: font files or folders to open, in order.
    pub files: Vec<PathBuf>,
}

impl CommandLine {
    /// Parse the current process's arguments (without the program name).
    pub fn from_env() -> CommandLine {
        Self::parse(std::env::args_os().skip(1))
    }

    /// Parse already-split arguments (program name excluded).
    pub fn parse<I, S>(args: I) -> CommandLine
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        let mut cl = CommandLine::default();
        for arg in args {
            let arg: OsString = arg.into();
            let text = arg.to_string_lossy().into_owned();
            match parse_switch(&text) {
                Some(Some(sw)) => cl.switches.push(sw),
                Some(None) => {} // a lone "/" or "-": ignored
                None => {
                    let file = text.trim().trim_matches('"');
                    if !file.is_empty() {
                        cl.files.push(PathBuf::from(file));
                    }
                }
            }
        }
        cl
    }

    /// Parse a raw command line string (as `lpCmdLine`, program name excluded), splitting it with
    /// the Windows rules (`CommandLineToArgvW`: quotes group, `\"` escapes, `2n` backslashes).
    pub fn parse_str(line: &str) -> CommandLine {
        Self::parse(split_command_line(line))
    }

    /// Switch present (case-insensitive)?
    pub fn has(&self, name: &str) -> bool {
        self.switch(name).is_some()
    }

    /// The last occurrence of a switch (case-insensitive).
    pub fn switch(&self, name: &str) -> Option<&Switch> {
        self.switches.iter().rev().find(|s| s.name.eq_ignore_ascii_case(name))
    }

    /// Value of a switch (`/Name:value`), if given.
    pub fn value(&self, name: &str) -> Option<&str> {
        self.switch(name).and_then(|s| s.value.as_deref())
    }

    pub fn no_splash(&self) -> bool {
        self.has("NoSplash")
    }
    /// Skip the start-up action (welcome screen / last library).
    pub fn no_startup_initial_action(&self) -> bool {
        self.has("NoStartupInitialAction")
    }
    /// No user interface: process the file arguments / helper and exit.
    pub fn no_ui(&self) -> bool {
        self.has("NoUI")
    }
    pub fn dde(&self) -> bool {
        self.has("DDE")
    }
    pub fn automation(&self) -> bool {
        self.has("Automation")
    }
    pub fn embedding(&self) -> bool {
        self.has("Embedding")
    }
    /// `Print` or its short form `P` (accepted, AnyFont has no print-and-exit mode).
    pub fn print(&self) -> bool {
        self.has("Print") || self.has("P")
    }
    /// `Some(true)` = `/Sdi`, `Some(false)` = `/Mdi`, `None` = application default. The later switch wins.
    pub fn sdi(&self) -> Option<bool> {
        self.switches.iter().rev().find_map(|s| {
            if s.name.eq_ignore_ascii_case("Sdi") {
                Some(true)
            } else if s.name.eq_ignore_ascii_case("Mdi") {
                Some(false)
            } else {
                None
            }
        })
    }
    /// `/InstallHelper:<request file>`: run as the elevated helper (see [`crate::helper::helper_main`]).
    pub fn install_helper(&self) -> Option<PathBuf> {
        self.value(INSTALL_HELPER_SWITCH).filter(|v| !v.is_empty()).map(PathBuf::from)
    }
    /// `/Lang:<code>`: UI language override (e.g. `en`, `pt-BR`).
    pub fn lang(&self) -> Option<&str> {
        self.value(LANG_SWITCH).filter(|v| !v.is_empty())
    }
    /// Switches that are not in [`KNOWN_SWITCHES`] (accepted and ignored; useful for logging).
    pub fn unknown_switches(&self) -> Vec<&Switch> {
        self.switches
            .iter()
            .filter(|s| !KNOWN_SWITCHES.iter().any(|k| k.eq_ignore_ascii_case(&s.name)))
            .collect()
    }
    /// Arguments to forward to an already running instance (files only).
    pub fn forwardable_files(&self) -> Vec<PathBuf> {
        self.files
            .iter()
            .map(|f| if f.is_relative() { std::env::current_dir().map(|d| d.join(f)).unwrap_or_else(|_| f.clone()) } else { f.clone() })
            .collect()
    }
}

/// `None` = not a switch; `Some(None)` = empty switch; `Some(Some(sw))` = switch.
fn parse_switch(arg: &str) -> Option<Option<Switch>> {
    let arg = arg.trim();
    let rest = if let Some(r) = arg.strip_prefix('/') {
        r
    } else if let Some(r) = arg.strip_prefix('-') {
        // tolerate GNU-style "--name"
        r.strip_prefix('-').unwrap_or(r)
    } else {
        return None;
    };
    let (name, value) = match rest.find(':') {
        Some(i) => (&rest[..i], Some(rest[i + 1..].trim().trim_matches('"').to_string())),
        None => (rest, None),
    };
    let name = name.trim().trim_matches('"');
    if name.is_empty() {
        return Some(None);
    }
    Some(Some(Switch { name: name.to_string(), value }))
}

/// Split a command line string like `CommandLineToArgvW` does for arguments after the first.
pub fn split_command_line(line: &str) -> Vec<String> {
    let mut args = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    let n = chars.len();
    loop {
        while i < n && (chars[i] == ' ' || chars[i] == '\t') {
            i += 1;
        }
        if i >= n {
            break;
        }
        let mut cur = String::new();
        let mut in_quotes = false;
        while i < n {
            let c = chars[i];
            if c == '\\' {
                let mut bs = 0;
                while i < n && chars[i] == '\\' {
                    bs += 1;
                    i += 1;
                }
                if i < n && chars[i] == '"' {
                    cur.extend(std::iter::repeat_n('\\', bs / 2));
                    if bs % 2 == 1 {
                        cur.push('"');
                        i += 1;
                    }
                } else {
                    cur.extend(std::iter::repeat_n('\\', bs));
                }
                continue;
            }
            if c == '"' {
                if in_quotes && i + 1 < n && chars[i + 1] == '"' {
                    cur.push('"');
                    i += 2;
                    continue;
                }
                in_quotes = !in_quotes;
                i += 1;
                continue;
            }
            if !in_quotes && (c == ' ' || c == '\t') {
                break;
            }
            cur.push(c);
            i += 1;
        }
        args.push(cur);
    }
    args
}

/// Quote one argument for a Windows command line (inverse of [`split_command_line`]).
pub fn quote_arg(arg: &str) -> String {
    if !arg.is_empty() && !arg.contains([' ', '\t', '"']) {
        return arg.to_string();
    }
    let mut out = String::from("\"");
    let mut bs = 0;
    for c in arg.chars() {
        match c {
            '\\' => bs += 1,
            '"' => {
                out.extend(std::iter::repeat_n('\\', bs * 2 + 1));
                out.push('"');
                bs = 0;
            }
            _ => {
                out.extend(std::iter::repeat_n('\\', bs));
                out.push(c);
                bs = 0;
            }
        }
    }
    out.extend(std::iter::repeat_n('\\', bs * 2));
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slash_and_dash_switches() {
        let cl = CommandLine::parse(["/NoSplash", "-NoUI", "font.ttf"]);
        assert!(cl.no_splash());
        assert!(cl.no_ui());
        assert_eq!(cl.files, vec![PathBuf::from("font.ttf")]);
    }

    #[test]
    fn values_after_colon_and_quotes_trimmed() {
        let cl = CommandLine::parse(["/NoSplash:1", "/Lang:\"pt-BR\"", r#"/InstallHelper:"C:\Temp\req 1.json""#]);
        assert_eq!(cl.value("nosplash"), Some("1"));
        assert_eq!(cl.lang(), Some("pt-BR"));
        assert_eq!(cl.install_helper(), Some(PathBuf::from(r"C:\Temp\req 1.json")));
    }

    #[test]
    fn value_keeps_later_colons() {
        let cl = CommandLine::parse([r"/InstallHelper:C:\x\y.json"]);
        assert_eq!(cl.install_helper(), Some(PathBuf::from(r"C:\x\y.json")));
    }

    #[test]
    fn case_insensitive_lookup() {
        let cl = CommandLine::parse(["/nosplash", "/NOSTARTUPINITIALACTION", "-dde", "/AUTOMATION", "/embedding"]);
        assert!(cl.no_splash());
        assert!(cl.no_startup_initial_action());
        assert!(cl.dde());
        assert!(cl.automation());
        assert!(cl.embedding());
        assert!(cl.has("NoSplash"));
    }

    #[test]
    fn dossier_examples() {
        let cl = CommandLine::parse_str("/NoSplash");
        assert!(cl.no_splash());
        let cl = CommandLine::parse_str("-Print");
        assert!(cl.print());
        let cl = CommandLine::parse_str("/NoUI:Sdi");
        assert!(cl.no_ui());
        assert_eq!(cl.value("NoUI"), Some("Sdi"));
        assert_eq!(cl.sdi(), None);
    }

    #[test]
    fn print_short_form_and_sdi_mdi() {
        assert!(CommandLine::parse(["/P"]).print());
        assert_eq!(CommandLine::parse(["/Sdi"]).sdi(), Some(true));
        assert_eq!(CommandLine::parse(["/Mdi"]).sdi(), Some(false));
        assert_eq!(CommandLine::parse(["/Sdi", "/mdi"]).sdi(), Some(false));
        assert_eq!(CommandLine::parse(Vec::<String>::new()).sdi(), None);
    }

    #[test]
    fn unknown_switches_are_accepted() {
        let cl = CommandLine::parse(["/Whatever:3", "/NoSplash", "a.otf"]);
        assert!(cl.no_splash());
        assert_eq!(cl.unknown_switches().len(), 1);
        assert_eq!(cl.unknown_switches()[0].name, "Whatever");
        assert_eq!(cl.files.len(), 1);
    }

    #[test]
    fn lone_dash_and_empty_are_ignored() {
        let cl = CommandLine::parse(["/", "-", "", "  "]);
        assert!(cl.switches.is_empty());
        assert!(cl.files.is_empty());
    }

    #[test]
    fn files_with_spaces_from_raw_line() {
        let cl = CommandLine::parse_str(r#"/NoSplash "C:\My Fonts\A B.ttf" C:\f\c.otf /Lang:de"#);
        assert_eq!(cl.files, vec![PathBuf::from(r"C:\My Fonts\A B.ttf"), PathBuf::from(r"C:\f\c.otf")]);
        assert_eq!(cl.lang(), Some("de"));
    }

    #[test]
    fn windows_splitting_rules() {
        assert_eq!(split_command_line(r#"a "b c" d"#), vec!["a", "b c", "d"]);
        assert_eq!(split_command_line(r#""a\"b""#), vec![r#"a"b"#]);
        assert_eq!(split_command_line(r#"a\\\\"b c" d"#), vec![r"a\\b c", "d"]);
        assert_eq!(split_command_line(r"C:\path\ x"), vec![r"C:\path\", "x"]);
        assert_eq!(split_command_line("   "), Vec::<String>::new());
    }

    #[test]
    fn quote_round_trip() {
        for s in [r"C:\a b\c.json", r#"we"ird"#, r"trail\", "plain", r"sp ace\"] {
            let q = quote_arg(s);
            assert_eq!(split_command_line(&q), vec![s.to_string()], "{q}");
        }
    }

    #[test]
    fn gnu_style_tolerated() {
        let cl = CommandLine::parse(["--lang:fr"]);
        assert_eq!(cl.lang(), Some("fr"));
    }
}
