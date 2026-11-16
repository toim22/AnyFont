//! Help ▸ Report a Bug... / Submit Ideas and Feedback... (owned by the services UI developer; ported from
//! AnyVector's `bugreport.rs`): file a GitHub issue on the AnyFont project, labelled `bug` or `enhancement`.
//!
//! Sending tries, in order:
//! 1. the issue relay (`services/issue-relay`, the Cloudflare Worker `anyfont-issues`): it creates the issue
//!    itself, so the user needs no GitHub account, and the issue's URL is shown;
//! 2. when the relay can't take the report (other than "wait a minute"), the browser opens GitHub's "New
//!    issue" page with the title, description and system information filled in (that needs an account).
//!
//! A report may carry one file of the types and sizes GitHub accepts when a file is dragged into an issue
//! (the relay's `src/attachment.ts`). On the browser path Explorer opens on the file, ready to drag in.
//! Contract: `docs/RELEASING.md` § Bug reports and ideas; payload: `services/issue-relay/src/report.ts`.

use crate::help::dlg;
use egui::{vec2, RichText};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{channel, Receiver};

/// The GitHub repository that receives bug reports (and publishes the releases the updater reads).
pub const REPO: &str = "toim22/AnyFont";

/// The relay that files the issues for people without a GitHub account: `services/issue-relay`.
/// The environment variable `ANYFONT_ISSUE_RELAY` points elsewhere (a local `wrangler dev`, a test).
pub const RELAY_URL: &str = "https://anyfont-issues.antoniocarlos85.workers.dev";

/// The program name the relay accepts.
pub const PROGRAM: &str = "AnyFont";

pub fn relay_url() -> String {
    std::env::var("ANYFONT_ISSUE_RELAY").ok().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| RELAY_URL.to_owned())
}

pub fn issues_url() -> String {
    format!("https://github.com/{REPO}/issues")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Bug, Idea }

impl Kind {
    /// The GitHub label of the issue.
    pub fn label(self) -> &'static str {
        match self {
            Kind::Bug => "bug",
            Kind::Idea => "enhancement",
        }
    }

    /// `kind` of the relay's JSON.
    pub fn relay_kind(self) -> &'static str {
        match self {
            Kind::Bug => "bug",
            Kind::Idea => "idea",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Kind::Bug => "Report a Bug",
            Kind::Idea => "Submit Ideas and Feedback",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Sent {
    /// Created by the relay; the issue URL.
    Created(String),
    /// The relay couldn't take it: the prefilled page opened in the browser.
    Browser,
    /// The relay asks to wait: several reports in a minute from this connection.
    Later,
    Failed(String),
}

const MB: u64 = 1024 * 1024;
/// Images, shown in the issue: 10 MB. GitHub's lists (docs.github.com › Attaching files) and the relay's
/// (`services/issue-relay/src/attachment.ts`).
pub const ATTACH_IMAGES: &[&str] = &["png", "gif", "jpg", "jpeg", "svg"];
/// Videos: 10 MB, as GitHub allows on a repository of a free account.
pub const ATTACH_VIDEOS: &[&str] = &["mp4", "mov", "webm"];
/// Every other type GitHub takes in an issue: 25 MB. Font files are not among them (zip them).
pub const ATTACH_OTHERS: &[&str] = &[
    "pdf", "docx", "pptx", "xlsx", "xls", "xlsm", "odt", "fodt", "ods", "fods", "odp", "fodp", "odg", "fodg", "odf", "rtf", "doc", "txt", "md", "copilotmd", "csv", "tsv", "log", "json", "jsonc", "c", "cs", "cpp", "css", "drawio", "dmp", "html", "htm", "java",
    "js", "ipynb", "patch", "php", "cpuprofile", "pdb", "py", "sh", "sql", "ts", "tsx", "xml", "yaml", "yml", "zip", "gz", "tgz", "debug", "msg", "eml", "bmp", "tif", "tiff", "mp3", "wav",
];

fn extension(path: &Path) -> String {
    path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// The largest file GitHub takes with this extension, or 0 when it doesn't take the type.
pub fn attach_limit(path: &Path) -> u64 {
    let ext = extension(path);
    if ATTACH_IMAGES.contains(&ext.as_str()) || ATTACH_VIDEOS.contains(&ext.as_str()) {
        10 * MB
    } else if ATTACH_OTHERS.contains(&ext.as_str()) {
        25 * MB
    } else {
        0
    }
}

/// Why GitHub would refuse a file of this name and size, or `None`.
pub fn attach_problem(path: &Path, size: u64) -> Option<String> {
    let name = file_name(path);
    let max = attach_limit(path);
    if max == 0 {
        let ext = extension(path);
        return Some(if ext.is_empty() { format!("GitHub doesn't accept \"{name}\" because it has no file type. Put it in a .zip file.") } else { format!("GitHub doesn't accept .{ext} files. Put \"{name}\" in a .zip file.") });
    }
    if size == 0 {
        return Some(format!("\"{name}\" is empty."));
    }
    (size > max).then(|| format!("\"{name}\" is {}. GitHub accepts at most {} MB for this type.", size_text(size), max / MB))
}

pub fn size_text(size: u64) -> String {
    if size < MB { format!("{} KB", size.div_ceil(1024).max(1)) } else { format!("{:.1} MB", size as f64 / MB as f64) }
}

fn content_type(path: &Path) -> &'static str {
    match extension(path).as_str() {
        "png" => "image/png",
        "gif" => "image/gif",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "txt" | "log" | "md" => "text/plain",
        _ => "application/octet-stream",
    }
}

/// The report in the part "report" and the file in the part "file", as the relay reads them.
pub fn multipart_body(boundary: &str, report: &str, file_name: &str, file_type: &str, data: &[u8]) -> Vec<u8> {
    let name = file_name.replace(['"', '\r', '\n'], "_");
    let mut b = Vec::with_capacity(data.len() + report.len() + 512);
    b.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"report\"\r\nContent-Type: application/json; charset=utf-8\r\n\r\n").as_bytes());
    b.extend_from_slice(report.as_bytes());
    b.extend_from_slice(format!("\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\nContent-Type: {file_type}\r\n\r\n").as_bytes());
    b.extend_from_slice(data);
    b.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    b
}

/// The fields of the form.
#[derive(Default, Clone)]
pub struct Form {
    pub kind: Option<Kind>,
    pub title: String,
    /// What happened / the idea.
    pub what_happened: String,
    /// Steps to reproduce / how it would help.
    pub steps: String,
    /// What the user expected (bug reports only).
    pub expected: String,
    pub include_system: bool,
    /// The file sent with the report, and its size.
    pub attachment: Option<(PathBuf, u64)>,
}

impl Form {
    pub fn new(kind: Kind) -> Form {
        Form { kind: Some(kind), include_system: kind == Kind::Bug, ..Default::default() }
    }

    pub fn kind(&self) -> Kind {
        self.kind.unwrap_or(Kind::Bug)
    }

    fn can_send(&self) -> bool {
        !self.title.trim().is_empty() && !self.what_happened.trim().is_empty()
    }
}

/// AnyFont version, Windows version and display information (the relay keeps at most 600 characters).
pub fn system_info(ctx: &egui::Context) -> String {
    let ppp = ctx.pixels_per_point();
    let screen = ctx.content_rect().size() * ppp;
    format!(
        "- AnyFont: {} ({}, source {})\n- Windows: {}\n- Window size: {:.0} × {:.0} px, scale {:.0}%",
        env!("CARGO_PKG_VERSION"),
        if cfg!(debug_assertions) { "debug build" } else { "release build" },
        crate::about::source_id(),
        crate::about::sysinfo::platform_with_build(),
        screen.x,
        screen.y,
        ppp * 100.0
    )
}

/// The issue body in GitHub Markdown (the browser fallback; the relay builds the same from the JSON).
pub fn issue_body(f: &Form, system: Option<&str>) -> String {
    let idea = f.kind() == Kind::Idea;
    let mut b = String::new();
    b.push_str(if idea { "### Idea or feedback\n\n" } else { "### What happened\n\n" });
    b.push_str(f.what_happened.trim());
    b.push_str("\n\n");
    if !f.steps.trim().is_empty() {
        b.push_str(if idea { "### Why it would help\n\n" } else { "### Steps to reproduce\n\n" });
        b.push_str(f.steps.trim());
        b.push_str("\n\n");
    }
    if !idea && !f.expected.trim().is_empty() {
        b.push_str("### Expected behavior\n\n");
        b.push_str(f.expected.trim());
        b.push_str("\n\n");
    }
    b.push_str("### AnyFont version\n\n");
    b.push_str(env!("CARGO_PKG_VERSION"));
    b.push_str("\n\n");
    if let Some(s) = system {
        b.push_str("### System information\n\n");
        b.push_str(s);
        b.push('\n');
    }
    b.push_str(if idea { "\n_Sent from AnyFont (Help ▸ Submit Ideas and Feedback)._\n" } else { "\n_Sent from AnyFont (Help ▸ Report a Bug)._\n" });
    b
}

/// Percent-encode a query-string value.
pub fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// GitHub's prefilled "New issue" page. The body is cut so the address stays below ~7000 characters
/// (longer addresses fail in some browsers and on GitHub).
pub fn new_issue_url(label: &str, title: &str, body: &str) -> String {
    let mut body = body.to_string();
    loop {
        let url = format!("https://github.com/{REPO}/issues/new?labels={label}&title={}&body={}", url_encode(title), url_encode(&body));
        if url.len() <= 7000 || body.is_empty() {
            return url;
        }
        let keep = body.chars().count() * 9 / 10;
        body = body.chars().take(keep).collect::<String>() + "…";
        if keep == 0 {
            body.clear();
        }
    }
}

/// Opens a URL in the default browser.
pub fn open_in_browser(url: &str) -> bool {
    dlg::open_url(url)
}

/// Open Explorer with the file selected.
fn show_in_explorer(path: &Path) {
    if crate::testing::intercept(crate::testing::External::Explorer(path.to_path_buf())) {
        return;
    }
    #[cfg(windows)]
    {
        let _ = Command::new("explorer").arg(format!("/select,{}", path.display())).spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = path.parent().map(|p| Command::new("xdg-open").arg(p).spawn());
    }
}

/// Runs the GitHub CLI without a console window; `None` when it is not installed.
pub fn gh(args: &[&str]) -> Option<std::process::Output> {
    let mut cmd = Command::new("gh");
    cmd.args(args);
    dlg::hide_console(&mut cmd);
    cmd.output().ok()
}

/// At most `max` characters of `s`, as the relay counts them: UTF-16 code units (JavaScript's
/// `String.length`), so a character outside the BMP (an emoji) counts twice. Never splits a character.
fn clip(s: &str, max: usize) -> String {
    let mut units = 0;
    s.chars()
        .take_while(|c| {
            units += c.len_utf16();
            units <= max
        })
        .collect()
}

/// The report as the relay reads it (`services/issue-relay/src/report.ts`), cut to its limits.
pub fn relay_body(f: &Form, system: Option<&str>) -> String {
    let idea = f.kind() == Kind::Idea;
    serde_json::json!({
        "kind": f.kind().relay_kind(),
        "title": clip(f.title.trim(), 200),
        "text": clip(f.what_happened.trim(), 8000),
        "more": clip(f.steps.trim(), 4000),
        "expected": if idea { String::new() } else { clip(f.expected.trim(), 2000) },
        "version": clip(env!("CARGO_PKG_VERSION"), 32),
        "program": PROGRAM,
        "system": system.map(|s| clip(s.trim(), 600)).unwrap_or_default(),
    })
    .to_string()
}

/// Posts the report (and the file) to the relay: the issue's URL, or the HTTP status (0 when unreachable) and why.
fn send_to_relay(base: &str, body: &str, file: Option<&Path>) -> Result<String, (u16, String)> {
    let (kind, payload) = match file {
        Some(path) => {
            let data = std::fs::read(path).map_err(|e| (0, format!("reading {}: {e}", path.display())))?;
            let boundary = format!("anyfont-{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
            (format!("multipart/form-data; boundary={boundary}"), multipart_body(&boundary, body, &file_name(path), content_type(path), &data))
        }
        None => ("application/json".to_string(), body.as_bytes().to_vec()),
    };
    // a 25 MB file needs time on a slow connection
    let secs = 20 + payload.len() as u64 / (64 * 1024);
    let agent: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).timeout_global(Some(std::time::Duration::from_secs(secs))).build().into();
    let url = format!("{}/issues", base.trim_end_matches('/'));
    let ua = format!("AnyFont/{}", env!("CARGO_PKG_VERSION"));
    match agent.post(&url).header("Content-Type", &kind).header("User-Agent", &ua).send(&payload[..]) {
        Ok(mut r) => {
            let status = r.status().as_u16();
            let text = r.body_mut().read_to_string().unwrap_or_default();
            if status == 201 || status == 200 {
                let url = serde_json::from_str::<serde_json::Value>(&text).ok().and_then(|v| v["url"].as_str().map(str::to_string)).unwrap_or_default();
                Ok(if url.starts_with("https://github.com/") { url } else { issues_url() })
            } else {
                Err((status, text))
            }
        }
        Err(e) => Err((0, e.to_string())),
    }
}

/// What to do with the relay's answer (`Err(())`: fall back to the browser).
#[allow(clippy::result_unit_err)]
pub fn outcome(answer: Result<String, (u16, String)>) -> Result<Sent, ()> {
    match answer {
        Ok(url) => Ok(Sent::Created(url)),
        Err((429, _)) => Ok(Sent::Later),
        // anything else (unreachable, refused, too large, GitHub failed): the browser
        Err(_) => Err(()),
    }
}

/// Sends in the background; the dialog polls the receiver.
fn send(relay: String, label: &'static str, title: String, body: String, file: Option<PathBuf>, ctx: egui::Context) -> Receiver<Sent> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let r = match outcome(send_to_relay(&relay_url(), &relay, file.as_deref())) {
            Ok(sent) => sent,
            Err(()) => {
                log::warn!("the issue relay did not take the report; opening the browser");
                if open_in_browser(&new_issue_url(label, &title, &body)) {
                    // GitHub's page can't be handed the file: show it in Explorer, ready to drag in
                    if let Some(f) = &file {
                        show_in_explorer(f);
                    }
                    Sent::Browser
                } else {
                    Sent::Failed(format!("Could not open the browser. Please send it at {}", issues_url()))
                }
            }
        };
        let _ = tx.send(r);
        ctx.request_repaint();
    });
    rx
}

pub struct BugReport {
    form: Option<Form>,
    attach_error: Option<String>,
    sending: Option<Receiver<Sent>>,
    result: Option<Sent>,
}

impl Default for BugReport {
    fn default() -> Self {
        Self::new()
    }
}

impl BugReport {
    pub fn new() -> Self {
        BugReport { form: None, attach_error: None, sending: None, result: None }
    }

    pub fn open(&mut self, kind: Kind) {
        // a report being typed is kept when the same form is opened again
        if self.form.as_ref().is_some_and(|f| f.kind() == kind) && self.sending.is_none() && self.result.is_none() {
            return;
        }
        self.form = Some(Form::new(kind));
        self.attach_error = None;
        self.sending = None;
        self.result = None;
    }

    /// `crash_report`: text of a crash report found at start-up, offered as the description.
    pub fn open_with_crash(&mut self, crash_report: String) {
        self.open(Kind::Bug);
        if let Some(f) = &mut self.form {
            f.title = "AnyFont closed unexpectedly".into();
            f.what_happened = clip(crash_report.trim(), 8000);
            f.include_system = true;
        }
    }

    /// True while a form is open.
    pub fn is_open(&self) -> bool {
        self.form.is_some()
    }

    pub fn ui(&mut self, ctx: &egui::Context) {
        let Some(mut f) = self.form.take() else { return };
        if let Some(rx) = &self.sending {
            if let Ok(r) = rx.try_recv() {
                self.sending = None;
                self.result = Some(r);
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
        }
        let system = system_info(ctx);
        let kind = f.kind();
        let idea = kind == Kind::Idea;
        let mut close = false;
        let ((), dismissed) = dlg::window(ctx, if idea { "feedback" } else { "bug_report" }, kind.title(), vec2(560.0, 650.0), |ui| {
            ui.spacing_mut().item_spacing.y = 5.0;
            let (top, bar) = dlg::split_bottom(ui, dlg::BUTTON_H);
            if let Some(r) = self.result.clone() {
                dlg::at(ui, top, |ui| {
                    ui.add_space(8.0);
                    dlg::heading(ui, match &r {
                        Sent::Created(_) if idea => "Thank you! Your idea was sent.",
                        Sent::Created(_) => "Thank you! Your bug report was sent.",
                        Sent::Browser => "Almost done: finish in your browser",
                        Sent::Later => "Please wait a minute",
                        Sent::Failed(_) if idea => "Your idea could not be sent",
                        Sent::Failed(_) => "The report could not be sent",
                    });
                    ui.add_space(6.0);
                    match &r {
                        Sent::Created(url) => {
                            ui.label("The issue was created on the AnyFont project on GitHub:");
                            if ui.link(url.as_str()).clicked() {
                                open_in_browser(url);
                            }
                        }
                        Sent::Browser => {
                            ui.label(if idea { "GitHub opened in your browser with your idea already filled in." } else { "GitHub opened in your browser with the report already filled in." });
                            if let Some((path, _)) = &f.attachment {
                                ui.label(format!("To attach \"{}\", drag it from the folder that opened into the description box on GitHub.", file_name(path)));
                            }
                            ui.label("Sign in if asked, review it and click \"Create\" to send it.");
                        }
                        Sent::Later => {
                            ui.label("Several reports were sent from this connection in the last minute. Try again in a minute.");
                        }
                        Sent::Failed(e) => dlg::error(ui, e),
                    }
                });
                dlg::at_right(ui, bar, |ui| {
                    if dlg::default_button(ui, "Close", true) || dlg::enter_pressed(ui) {
                        close = true;
                    }
                    if matches!(r, Sent::Failed(_) | Sent::Later) && dlg::button(ui, "Back", true) {
                        self.result = None;
                    }
                });
                return;
            }
            let enabled = self.sending.is_none();
            dlg::at(ui, top, |ui| {
                ui.add_space(6.0);
                ui.add_enabled_ui(enabled, |ui| {
                    if idea {
                        dlg::note(ui, "Tell us your idea or what you think of AnyFont. It is sent to the AnyFont project on GitHub as a new idea. You don't need a GitHub account.");
                        ui.add_space(4.0);
                        let l = ui.label("Title (your idea in a few words):");
                        ui.add(egui::TextEdit::singleline(&mut f.title).hint_text("e.g. Sort the font list by date added").desired_width(f32::INFINITY)).labelled_by(l.id);
                        let l = ui.label("Describe your idea or feedback:");
                        ui.add(egui::TextEdit::multiline(&mut f.what_happened).desired_rows(7).desired_width(f32::INFINITY)).labelled_by(l.id);
                        let l = ui.label("How would it help you? (optional)");
                        ui.add(egui::TextEdit::multiline(&mut f.steps).desired_rows(4).desired_width(f32::INFINITY)).labelled_by(l.id);
                    } else {
                        dlg::note(ui, "Describe the problem. It is sent to the AnyFont project on GitHub as a new issue. You don't need a GitHub account.");
                        ui.add_space(4.0);
                        let l = ui.label("Title (a short summary):");
                        ui.add(egui::TextEdit::singleline(&mut f.title).hint_text("e.g. The font list is empty after Add Folder").desired_width(f32::INFINITY)).labelled_by(l.id);
                        let l = ui.label("What happened?");
                        egui::ScrollArea::vertical().id_salt("what").max_height(110.0).show(ui, |ui| {
                            ui.add(egui::TextEdit::multiline(&mut f.what_happened).desired_rows(4).desired_width(f32::INFINITY)).labelled_by(l.id);
                        });
                        let l = ui.label("Steps to reproduce (optional):");
                        ui.add(egui::TextEdit::multiline(&mut f.steps).hint_text("1. ...\n2. ...").desired_rows(3).desired_width(f32::INFINITY)).labelled_by(l.id);
                        let l = ui.label("What did you expect to happen? (optional)");
                        ui.add(egui::TextEdit::multiline(&mut f.expected).desired_rows(2).desired_width(f32::INFINITY)).labelled_by(l.id);
                    }
                    self.attachment_row(ui, &mut f);
                    dlg::separator(ui);
                    ui.checkbox(&mut f.include_system, "Include system information");
                    if f.include_system {
                        ui.indent("sys", |ui| {
                            let c = dlg::dim_color(ui);
                            // the values are data, not interface text: a LayoutJob is not translated
                            ui.label(egui::text::LayoutJob::simple(system.clone(), egui::FontId::proportional(11.5), c, f32::INFINITY));
                        });
                    }
                });
            });
            dlg::at_left(ui, bar, |ui| {
                if ui.link(if idea { "View submitted ideas" } else { "View reported bugs" }).clicked() {
                    open_in_browser(&if idea { format!("{}?q=is%3Aissue+label%3Aenhancement", issues_url()) } else { format!("{}?q=is%3Aissue+label%3Abug", issues_url()) });
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if dlg::button(ui, "Cancel", true) {
                        close = true;
                    }
                    let label = if self.sending.is_some() { "Sending..." } else { "Send" };
                    if dlg::default_button(ui, label, enabled && f.can_send()) {
                        let sys = f.include_system.then_some(system.as_str());
                        let body = issue_body(&f, sys);
                        let file = f.attachment.as_ref().map(|(p, _)| p.clone());
                        self.sending = Some(send(relay_body(&f, sys), kind.label(), f.title.trim().to_string(), body, file, ui.ctx().clone()));
                    }
                    if self.sending.is_some() {
                        ui.spinner();
                    }
                });
            });
        });
        // closing while sending only hides the form: the report still goes out
        if !(close || dismissed) {
            self.form = Some(f);
        } else {
            self.result = None;
            self.attach_error = None;
            if self.sending.is_some() {
                self.form = None;
            }
        }
    }

    /// "Attach a file (optional)": Attach File..., then the chosen file with its size and Remove.
    fn attachment_row(&mut self, ui: &mut egui::Ui, f: &mut Form) {
        ui.label("Attach a file (optional):");
        ui.horizontal(|ui| {
            if dlg::button(ui, "Attach File...", true) {
                let mut all: Vec<&str> = ATTACH_IMAGES.iter().chain(ATTACH_VIDEOS).chain(ATTACH_OTHERS).copied().collect();
                all.sort_unstable();
                let picked = crate::testing::pick(|| rfd::FileDialog::new()
                    .set_title("Attach a File")
                    .add_filter("Files GitHub accepts", &all)
                    .add_filter("Images", ATTACH_IMAGES)
                    .add_filter("Videos", ATTACH_VIDEOS)
                    .add_filter("Archives (.zip, .gz, .tgz)", &["zip", "gz", "tgz"])
                    .pick_file());
                if let Some(p) = picked {
                    let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                    self.attach_error = attach_problem(&p, size);
                    if self.attach_error.is_none() {
                        f.attachment = Some((p, size));
                    }
                }
            }
            if let Some((path, size)) = &f.attachment {
                let text = format!("{} ({})", file_name(path), size_text(*size));
                ui.label(egui::text::LayoutJob::simple_singleline(text, dlg::body_font(ui), dlg::text_color(ui))).on_hover_text(path.display().to_string());
                if ui.link("Remove").clicked() {
                    f.attachment = None;
                }
            }
        });
        if let Some(e) = &self.attach_error {
            dlg::error(ui, e);
        } else {
            let c = dlg::dim_color(ui);
            ui.add(egui::Label::new(RichText::new("Images and videos up to 10 MB, other files up to 25 MB, of the types GitHub accepts. Put a font file (.ttf, .otf) in a .zip file.").color(c).size(11.5)).wrap());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(kind: Kind) -> Form {
        Form { title: "T".into(), what_happened: "Crash".into(), steps: "1. Open".into(), ..Form::new(kind) }
    }

    #[test]
    fn encodes_and_builds_url() {
        assert_eq!(url_encode("a b&c/ç"), "a%20b%26c%2F%C3%A7");
        let f = form(Kind::Bug);
        let body = issue_body(&f, Some("- AnyFont: 0.1.0"));
        assert!(body.contains("### What happened\n\nCrash"));
        assert!(body.contains("### Steps to reproduce"));
        assert!(!body.contains("### Expected behavior"));
        assert!(body.contains("Sent from AnyFont (Help ▸ Report a Bug)"));
        let url = new_issue_url(Kind::Bug.label(), "T", &body);
        assert!(url.starts_with("https://github.com/toim22/AnyFont/issues/new?labels=bug&title=T&body="));
        let idea = Form { title: "Sort".into(), what_happened: "Sort by date".into(), steps: "Faster".into(), expected: "x".into(), ..Form::new(Kind::Idea) };
        let body = issue_body(&idea, None);
        assert!(body.contains("### Idea or feedback\n\nSort by date"));
        assert!(body.contains("### Why it would help\n\nFaster"));
        assert!(!body.contains("System information") && !body.contains("Expected"));
        assert!(new_issue_url(Kind::Idea.label(), "Sort", &body).contains("labels=enhancement&"));
        // a huge body is cut so the address stays usable
        let long = Form { what_happened: "x".repeat(20000), ..form(Kind::Bug) };
        assert!(new_issue_url("bug", "T", &issue_body(&long, None)).len() <= 7000);
    }

    #[test]
    fn relay_payload_matches_report_ts() {
        let ts = include_str!("../../../services/issue-relay/src/report.ts");
        assert!(ts.contains("const PROGRAMS = [\"AnyFont\"]"), "the relay accepts the program name AnyFont");
        assert!(ts.contains("title: 200, text: 8000, more: 4000, expected: 2000, version: 32, program: 40, system: 600"));
        let f = form(Kind::Bug);
        let v: serde_json::Value = serde_json::from_str(&relay_body(&f, Some("- Windows: 11"))).unwrap();
        assert_eq!(v["kind"], "bug");
        assert_eq!(v["title"], "T");
        assert_eq!(v["text"], "Crash");
        assert_eq!(v["more"], "1. Open");
        assert_eq!(v["expected"], "");
        assert_eq!(v["program"], "AnyFont");
        assert_eq!(v["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(v["system"], "- Windows: 11");
        // every key of the Report interface, nothing else
        let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        for k in ["kind", "title", "text", "more", "expected", "version", "program", "system"] {
            assert!(keys.contains(&k), "{k}");
        }
        assert_eq!(keys.len(), 8);
        let idea = Form { expected: "x".into(), title: "é".repeat(300), ..form(Kind::Idea) };
        let v: serde_json::Value = serde_json::from_str(&relay_body(&idea, Some(&"s".repeat(900)))).unwrap();
        assert_eq!((v["kind"].as_str(), v["expected"].as_str()), (Some("idea"), Some("")));
        assert_eq!(v["title"].as_str().unwrap().chars().count(), 200);
        assert_eq!(v["system"].as_str().unwrap().chars().count(), 600);
        // the relay counts UTF-16 units: an emoji is two (BUG-101)
        assert_eq!(clip(&"\u{1F600}".repeat(150), 200).chars().count(), 100);
        assert_eq!(clip(&format!("a{}", "\u{1F600}".repeat(150)), 200).encode_utf16().count(), 199);
    }

    #[test]
    fn relay_answers() {
        assert_eq!(outcome(Ok("https://github.com/toim22/AnyFont/issues/7".into())), Ok(Sent::Created("https://github.com/toim22/AnyFont/issues/7".into())));
        assert_eq!(outcome(Err((429, String::new()))), Ok(Sent::Later));
        for status in [0, 400, 413, 502] {
            assert_eq!(outcome(Err((status, String::new()))), Err(()), "{status} falls back to the browser");
        }
    }

    #[test]
    fn attachments_follow_github_rules() {
        let p = |s: &str| PathBuf::from(s);
        assert_eq!(attach_problem(&p("C:/a/shot.PNG"), 10 * MB), None);
        assert!(attach_problem(&p("shot.png"), 10 * MB + 1).unwrap().contains("10 MB"));
        assert!(attach_problem(&p("clip.mp4"), 11 * MB).unwrap().contains("10 MB"));
        assert_eq!(attach_problem(&p("fonts.zip"), 25 * MB), None);
        assert!(attach_problem(&p("fonts.zip"), 25 * MB + 1).unwrap().contains("25 MB"));
        assert!(attach_problem(&p("Arial.ttf"), 100).unwrap().contains(".ttf"));
        assert!(attach_problem(&p("noext"), 100).unwrap().contains("no file type"));
        assert!(attach_problem(&p("a.txt"), 0).unwrap().contains("empty"));
        assert_eq!((size_text(1), size_text(2048), size_text(3 * MB)), ("1 KB".into(), "2 KB".into(), "3.0 MB".into()));
        let text = String::from_utf8(multipart_body("B", "{\"title\":\"T\"}", "my \"file\".zip", "application/zip", b"PK")).unwrap();
        assert_eq!(
            text,
            "--B\r\nContent-Disposition: form-data; name=\"report\"\r\nContent-Type: application/json; charset=utf-8\r\n\r\n{\"title\":\"T\"}\r\n--B\r\nContent-Disposition: form-data; name=\"file\"; filename=\"my _file_.zip\"\r\nContent-Type: application/zip\r\n\r\nPK\r\n--B--\r\n"
        );
    }

    #[test]
    fn crash_report_prefills_a_bug() {
        let mut b = BugReport::new();
        b.open_with_crash("panicked at src/app.rs:1".into());
        let f = b.form.as_ref().unwrap();
        assert_eq!(f.kind(), Kind::Bug);
        assert!(f.can_send());
        assert!(f.what_happened.contains("panicked"));
    }
}
