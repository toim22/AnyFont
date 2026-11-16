//! Help ▸ Report a Bug... and Help ▸ Submit Ideas and Feedback...: the form filled like a user, a file
//! attached (the file dialog answered by `anyfont::testing::queue_pick`) and the report sent to a fake issue
//! relay on this computer (`ANYFONT_ISSUE_RELAY`). The payload is checked against the rules of the real
//! relay (`services/issue-relay/src/report.ts` › `parseReport`, `index.ts` › the multipart parts). When the
//! relay is down or refuses, the browser fallback (GitHub's prefilled new-issue page) and Explorer (the
//! attached file) are recorded, never opened; nothing is ever posted to the real relay or to GitHub.
mod common;
mod svc;

use anyfont::testing::{self, External};
use common::http::{Request, Response, Server};
use common::*;
use egui::accesskit::{Role, Toggled};
use egui_kittest::kittest::{by, NodeT, Queryable};
use serde_json::Value;
use svc::*;

const SIZE: (f32, f32) = (1200.0, 1000.0);
const VERSION: &str = env!("CARGO_PKG_VERSION");

// --- the relay's validation (report.ts), to check what AnyFont sends ---

const LIMITS: [(&str, usize); 7] = [("title", 200), ("text", 8000), ("more", 4000), ("expected", 2000), ("version", 32), ("program", 40), ("system", 600)];

fn is_version(v: &str) -> bool {
    // ^\d+\.\d+\.\d+([-.+][0-9A-Za-z.-]+)?$
    let (core, rest) = match v.find(['-', '+']) {
        Some(i) => (&v[..i], Some(&v[i + 1..])),
        None => (v, None),
    };
    let nums: Vec<&str> = core.split('.').collect();
    let core_ok = nums.len() >= 3 && nums[..3].iter().all(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
    // a 4th dotted part counts as the "[.]suffix"
    let dotted_suffix_ok = nums.len() == 3 || nums[3..].iter().all(|p| p.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
    core_ok && dotted_suffix_ok && rest.is_none_or(|r| !r.is_empty() && r.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-'))
}

/// `parseReport` of the relay: `Ok` when it would accept the JSON (lengths in UTF-16 units, as JavaScript counts).
fn relay_accepts(v: &Value) -> Result<(), String> {
    let o = v.as_object().ok_or("not a JSON object")?;
    if !matches!(o.get("kind").and_then(Value::as_str), Some("bug" | "idea")) {
        return Err("kind must be bug or idea".into());
    }
    let mut t = std::collections::HashMap::new();
    for (k, max) in LIMITS {
        let s = match o.get(k) {
            None | Some(Value::Null) => String::new(),
            Some(Value::String(s)) => s.replace("\r\n", "\n").trim().to_string(),
            Some(_) => return Err(format!("{k}: not text")),
        };
        let n = s.encode_utf16().count();
        if n > max {
            return Err(format!("{k}: {n} UTF-16 units, longer than {max} characters"));
        }
        t.insert(k, s);
    }
    if t["title"].is_empty() || t["text"].is_empty() {
        return Err("the title and the description are required".into());
    }
    if !is_version(&t["version"]) {
        return Err("version is not a version number".into());
    }
    if t["program"] != "AnyFont" {
        return Err("unknown program".into());
    }
    Ok(())
}

/// The JSON keys AnyFont must send, no more, no less.
fn assert_keys(v: &Value) {
    let mut keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["expected", "kind", "more", "program", "system", "text", "title", "version"]);
}

/// The parts of a multipart body: (headers, data).
fn multipart(req: &Request) -> Vec<(String, Vec<u8>)> {
    let ct = req.header("content-type").expect("content type");
    let boundary = ct.split("boundary=").nth(1).expect("boundary").trim_matches('"').to_string();
    let delim = format!("--{boundary}");
    let body = &req.body;
    let find = |from: usize, pat: &[u8]| body[from..].windows(pat.len()).position(|w| w == pat).map(|i| i + from);
    let mut parts = Vec::new();
    let mut pos = find(0, delim.as_bytes()).expect("first boundary") + delim.len();
    loop {
        if body[pos..].starts_with(b"--") {
            break;
        }
        assert!(body[pos..].starts_with(b"\r\n"), "CRLF after the boundary");
        pos += 2;
        let head_end = find(pos, b"\r\n\r\n").expect("part headers");
        let headers = String::from_utf8_lossy(&body[pos..head_end]).into_owned();
        let data_start = head_end + 4;
        let next = find(data_start, format!("\r\n{delim}").as_bytes()).expect("next boundary");
        parts.push((headers, body[data_start..next].to_vec()));
        pos = next + 2 + delim.len();
    }
    parts
}

// --- the form ---

/// A window whose relay is a local server answering `answer` to POST /issues.
fn world(answer: Option<Response>) -> (Env, Option<Server>, EnvVar) {
    let env = Env::new();
    match answer {
        Some(a) => {
            let srv = Server::start();
            srv.route("POST", "/issues", a);
            let v = EnvVar::set("ANYFONT_ISSUE_RELAY", &srv.url(""));
            (env, Some(srv), v)
        }
        None => (env, None, EnvVar::set("ANYFONT_ISSUE_RELAY", DEAD_URL)),
    }
}

fn created(url: &str) -> Response {
    Response::json(201, &serde_json::json!({ "url": url, "number": 4242 }))
}

fn field<'t>(h: &'t H<'_>, label: &'t str) -> egui_kittest::Node<'t> {
    h.query_all(by().label(label).predicate(|n| matches!(n.role(), Role::TextInput | Role::MultilineTextInput)))
        .last()
        .unwrap_or_else(|| panic!("no text box {label:?}; shown: {:?}", all_texts(h)))
}

fn fill(h: &mut H<'_>, label: &str, text: &str) {
    field(h, label).focus();
    settle(h);
    field(h, label).type_text(text);
    settle(h);
}

fn field_value(h: &H<'_>, label: &str) -> String {
    field(h, label).accesskit_node().value().unwrap_or_default()
}

fn system_checked(h: &H<'_>) -> bool {
    let n = h.get_by_role_and_label(Role::CheckBox, "Include system information");
    n.accesskit_node().toggled() == Some(Toggled::True)
}

fn fill_bug(h: &mut H<'_>, title: &str) {
    click_menu(h, "Help", "Report a Bug...");
    assert!(!button_enabled(h, "Send"), "Send waits for a title and a description");
    fill(h, "Title (a short summary):", title);
    assert!(!button_enabled(h, "Send"), "and a description");
    fill(h, "What happened?", "The font list stays empty after Add Folder.");
    assert!(button_enabled(h, "Send"), "{:?}", button_labels(h));
}

fn wait_heading(h: &mut H<'_>, heading: &str) {
    wait_text(h, 30, heading);
}

#[test]
fn bug_report_is_sent_to_the_relay_as_json() {
    let _lock = env_lock();
    let issue = format!("https://github.com/toim22/AnyFont/issues/{}", 4000 + std::process::id() % 1000);
    let (env, srv, _v) = world(Some(created(&issue)));
    let srv = srv.unwrap();
    let mut h = harness(env.config(), SIZE);
    fill_bug(&mut h, "  Empty list after Add Folder  ");
    fill(&mut h, "Steps to reproduce (optional):", "1. File > Add Folder\n2. Pick a folder of fonts");
    fill(&mut h, "What did you expect to happen? (optional)", "The fonts of the folder.");
    assert!(system_checked(&h), "a bug report includes the system information by default");
    // View reported bugs (recorded, no browser)
    click_label(&mut h, "View reported bugs");
    assert!(testing::urls().contains(&"https://github.com/toim22/AnyFont/issues?q=is%3Aissue+label%3Abug".to_string()), "{:?}", testing::urls());
    click_button(&mut h, "Send");
    wait_heading(&mut h, "Thank you! Your bug report was sent.");
    assert!(has_text(&h, &issue), "the new issue's address: {:?}", all_texts(&h));
    // what the relay got
    let reqs = srv.requests_to("/issues");
    assert_eq!(reqs.len(), 1, "one POST");
    let r = &reqs[0];
    assert_eq!(r.method, "POST");
    assert_eq!(r.header("content-type"), Some("application/json"));
    assert_eq!(r.header("user-agent"), Some(format!("AnyFont/{VERSION}").as_str()));
    let v = r.json();
    assert_keys(&v);
    assert_eq!(v["kind"], "bug");
    assert_eq!(v["title"], "Empty list after Add Folder", "trimmed");
    assert_eq!(v["text"], "The font list stays empty after Add Folder.");
    assert_eq!(v["more"], "1. File > Add Folder\n2. Pick a folder of fonts");
    assert_eq!(v["expected"], "The fonts of the folder.");
    assert_eq!(v["version"], VERSION);
    assert_eq!(v["program"], "AnyFont");
    let sys = v["system"].as_str().unwrap();
    assert!(sys.starts_with(&format!("- AnyFont: {VERSION} (")), "{sys}");
    assert!(sys.contains("- Windows: ") && sys.contains("- Window size: "), "{sys}");
    relay_accepts(&v).expect("the relay accepts the report");
    // the link opens the issue (recorded)
    click_label(&mut h, &issue);
    assert!(testing::urls().contains(&issue));
    click_button(&mut h, "Close");
    assert!(!has_text(&h, "Thank you!"), "Close closes the dialog");
    assert!(buttons(&h, "Send").is_empty());
}

#[test]
fn idea_with_an_attachment_is_sent_as_multipart() {
    let _lock = env_lock();
    let issue = "https://github.com/toim22/AnyFont/issues/77";
    let (env, srv, _v) = world(Some(created(issue)));
    let srv = srv.unwrap();
    let notes = env.path("notes.txt");
    let data: Vec<u8> = b"line one\r\nline two \xC3\xA9\r\n".repeat(40);
    std::fs::write(&notes, &data).unwrap();
    let font = env.path("MyFont.ttf");
    std::fs::write(&font, b"not really a font").unwrap();
    let mut h = harness(env.config(), SIZE);
    click_menu(&mut h, "Help", "Submit Ideas and Feedback...");
    assert!(!system_checked(&h), "an idea leaves the system information out by default");
    fill(&mut h, "Title (your idea in a few words):", "Sort by date added");
    fill(&mut h, "Describe your idea or feedback:", "A column with the date each font was added.");
    fill(&mut h, "How would it help you? (optional)", "I find new fonts faster.");
    // a font file is refused (GitHub doesn't take it)
    testing::queue_pick(Some(font.clone()));
    click_button(&mut h, "Attach File...");
    assert!(has_text(&h, "GitHub doesn't accept .ttf files. Put \"MyFont.ttf\" in a .zip file."), "{:?}", all_texts(&h));
    // a cancelled dialog changes nothing
    testing::queue_pick(None);
    click_button(&mut h, "Attach File...");
    assert!(!has_text(&h, "notes.txt"));
    // attach, remove, attach again
    testing::queue_pick(Some(notes.clone()));
    click_button(&mut h, "Attach File...");
    let shown = format!("notes.txt ({})", anyfont::bugreport::size_text(data.len() as u64));
    assert!(has_text(&h, &shown), "{shown}: {:?}", all_texts(&h));
    assert!(!has_text(&h, "GitHub doesn't accept"), "the error goes away");
    click_label(&mut h, "Remove");
    assert!(!has_text(&h, "notes.txt"), "Remove drops the file");
    testing::queue_pick(Some(notes.clone()));
    click_button(&mut h, "Attach File...");
    assert!(has_text(&h, &shown));
    click_button(&mut h, "Send");
    wait_heading(&mut h, "Thank you! Your idea was sent.");
    assert!(has_text(&h, issue));
    // the multipart body: the report, then the file
    let r = &srv.requests_to("/issues")[0];
    let ct = r.header("content-type").unwrap();
    assert!(ct.starts_with("multipart/form-data; boundary=anyfont-"), "{ct}");
    assert_eq!(r.header("user-agent"), Some(format!("AnyFont/{VERSION}").as_str()));
    let parts = multipart(r);
    assert_eq!(parts.len(), 2, "report and file");
    let (h0, d0) = &parts[0];
    assert!(h0.contains("Content-Disposition: form-data; name=\"report\"") && !h0.contains("filename="), "the report is a text part: {h0}");
    assert!(h0.contains("Content-Type: application/json; charset=utf-8"), "{h0}");
    let v: Value = serde_json::from_slice(d0).expect("the report part is JSON");
    assert_keys(&v);
    assert_eq!(v["kind"], "idea");
    assert_eq!(v["title"], "Sort by date added");
    assert_eq!(v["text"], "A column with the date each font was added.");
    assert_eq!(v["more"], "I find new fonts faster.");
    assert_eq!(v["expected"], "", "no expected behaviour for an idea");
    assert_eq!(v["system"], "", "left out");
    relay_accepts(&v).expect("the relay accepts the report");
    let (h1, d1) = &parts[1];
    assert!(h1.contains("Content-Disposition: form-data; name=\"file\"; filename=\"notes.txt\""), "{h1}");
    assert!(h1.contains("Content-Type: text/plain"), "{h1}");
    assert_eq!(d1, &data, "the file's bytes, unchanged");
}

#[test]
fn relay_down_falls_back_to_the_browser_and_shows_the_file() {
    let _lock = env_lock();
    let (env, _srv, _v) = world(None);
    let shot = env.path("screenshot.png");
    std::fs::write(&shot, b"\x89PNG\r\n\x1a\nfake image data").unwrap();
    let mut h = harness(env.config(), SIZE);
    let title = format!("Crash on open {}", unique("t"));
    fill_bug(&mut h, &title);
    testing::queue_pick(Some(shot.clone()));
    click_button(&mut h, "Attach File...");
    click_button(&mut h, "Send");
    wait_heading(&mut h, "Almost done: finish in your browser");
    assert!(has_text(&h, "To attach \"screenshot.png\", drag it"), "{:?}", all_texts(&h));
    let prefix = format!("https://github.com/toim22/AnyFont/issues/new?labels=bug&title={}&body=", anyfont::bugreport::url_encode(&title));
    let url = testing::urls().into_iter().find(|u| u.starts_with(&prefix)).unwrap_or_else(|| panic!("the prefilled new-issue page: {:?}", testing::urls()));
    let body = &url[prefix.len()..];
    assert!(body.starts_with(&anyfont::bugreport::url_encode("### What happened\n\nThe font list stays empty after Add Folder.")), "{body}");
    assert!(body.contains(&anyfont::bugreport::url_encode("### AnyFont version")));
    assert!(url.len() <= 7000);
    assert!(testing::requests().contains(&External::Explorer(shot.clone())), "Explorer shows the file to drag in: {:?}", testing::requests());
    click_button(&mut h, "Close");
    assert!(!has_text(&h, "Almost done"));
}

#[test]
fn relay_error_falls_back_to_the_browser() {
    let _lock = env_lock();
    let (env, srv, _v) = world(Some(Response::new(500, "application/json", r#"{"error":"GitHub failed"}"#)));
    let srv = srv.unwrap();
    let mut h = harness(env.config(), SIZE);
    click_menu(&mut h, "Help", "Submit Ideas and Feedback...");
    let title = format!("Idea {}", unique("i"));
    fill(&mut h, "Title (your idea in a few words):", &title);
    fill(&mut h, "Describe your idea or feedback:", "Dark title bar.");
    click_button(&mut h, "Send");
    wait_heading(&mut h, "Almost done: finish in your browser");
    assert!(has_text(&h, "GitHub opened in your browser with your idea already filled in."));
    assert_eq!(srv.requests_to("/issues").len(), 1, "the relay was asked first");
    let prefix = format!("https://github.com/toim22/AnyFont/issues/new?labels=enhancement&title={}&body=", anyfont::bugreport::url_encode(&title));
    assert!(testing::urls().iter().any(|u| u.starts_with(&prefix)), "{:?}", testing::urls());
}

#[test]
fn rate_limited_asks_to_try_later_and_back_keeps_the_form() {
    let _lock = env_lock();
    let (env, srv, _v) = world(Some(Response::new(429, "application/json", r#"{"error":"too many"}"#)));
    let srv = srv.unwrap();
    let mut h = harness(env.config(), SIZE);
    let title = format!("Later {}", unique("l"));
    fill_bug(&mut h, &title);
    let before = testing::urls().len();
    click_button(&mut h, "Send");
    wait_heading(&mut h, "Please wait a minute");
    assert!(has_text(&h, "Several reports were sent from this connection in the last minute. Try again in a minute."));
    assert!(!testing::urls().iter().skip(before).any(|u| u.contains(&anyfont::bugreport::url_encode(&title))), "no browser for a rate limit");
    click_button(&mut h, "Back");
    assert_eq!(field_value(&h, "Title (a short summary):"), title, "the form keeps what was typed");
    assert!(button_enabled(&h, "Send"));
    // a second try now succeeds
    srv.route("POST", "/issues", created("https://github.com/toim22/AnyFont/issues/5"));
    click_button(&mut h, "Send");
    wait_heading(&mut h, "Thank you! Your bug report was sent.");
    assert_eq!(srv.requests_to("/issues").len(), 2);
}

#[test]
fn cancel_closes_and_the_form_is_kept_for_the_same_kind() {
    let env = Env::new();
    let mut h = harness(env.config(), SIZE);
    click_menu(&mut h, "Help", "Report a Bug...");
    fill(&mut h, "Title (a short summary):", "Kept");
    click_button(&mut h, "Cancel");
    assert!(buttons(&h, "Send").is_empty(), "Cancel closes the form");
    click_menu(&mut h, "Help", "Report a Bug...");
    assert_eq!(field_value(&h, "Title (a short summary):"), "", "a cancelled report starts empty again");
    h.key_press(egui::Key::Escape);
    settle(&mut h);
    assert!(buttons(&h, "Send").is_empty(), "Esc closes the form");
}

#[test]
fn long_ascii_fields_are_cut_to_the_relay_limits() {
    let _lock = env_lock();
    let (env, srv, _v) = world(Some(created("https://github.com/toim22/AnyFont/issues/6")));
    let srv = srv.unwrap();
    let mut h = harness(env.config(), SIZE);
    click_menu(&mut h, "Help", "Report a Bug...");
    fill(&mut h, "Title (a short summary):", &"t".repeat(260));
    fill(&mut h, "What happened?", &"w".repeat(8100));
    fill(&mut h, "What did you expect to happen? (optional)", &"e".repeat(2100));
    click_button(&mut h, "Send");
    wait_heading(&mut h, "Thank you! Your bug report was sent.");
    let v = srv.requests_to("/issues")[0].json();
    assert_eq!(v["title"].as_str().unwrap().len(), 200);
    assert_eq!(v["text"].as_str().unwrap().len(), 8000);
    assert_eq!(v["expected"].as_str().unwrap().len(), 2000);
    relay_accepts(&v).expect("the relay accepts the cut report");
}

#[test]
fn non_bmp_title_is_cut_to_the_relay_limit() {
    let _lock = env_lock();
    let (env, srv, _v) = world(Some(created("https://github.com/toim22/AnyFont/issues/7")));
    let srv = srv.unwrap();
    let mut h = harness(env.config(), SIZE);
    click_menu(&mut h, "Help", "Report a Bug...");
    // 150 emoji = 150 characters for AnyFont, 300 UTF-16 units for the relay (limit 200)
    fill(&mut h, "Title (a short summary):", &"\u{1F600}".repeat(150));
    fill(&mut h, "What happened?", "Emoji in the title.");
    click_button(&mut h, "Send");
    wait_heading(&mut h, "Thank you! Your bug report was sent.");
    let v = srv.requests_to("/issues")[0].json();
    relay_accepts(&v).expect("the real relay would accept this report (it answers 400 and AnyFont falls back to the browser)");
}
