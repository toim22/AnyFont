//! `anyfont.exe /SelfTest:` — the scripted font operations that `.github/workflows/e2e-real.yml` runs on the real
//! font system of a disposable GitHub runner. Here the same script (`tests/real-system/selftest.json`) runs on the
//! **sandbox backend** (`ANYFONT_SANDBOX_ROOT`), through the real `anyfont.exe` and its `/InstallHelper:` child,
//! and the guard that keeps the real backend off this PC is checked. Nothing here touches this PC's fonts.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// `anyfont.exe` with an environment that can never select the real backend by accident.
fn anyfont(script: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_anyfont"));
    c.arg(format!("/SelfTest:{}", script.display()));
    for v in ["GITHUB_ACTIONS", "RUNNER_ENVIRONMENT", "ANYFONT_SANDBOX_ROOT", "ANYFONT_SANDBOX_SEED"] {
        c.env_remove(v);
    }
    c.env("USERNAME", "e2e-tester");
    c
}

fn script_for(dir: &Path, results: &Path) -> PathBuf {
    let text = std::fs::read_to_string(repo().join(r"tests\real-system\selftest.json")).unwrap();
    let mut s: Value = serde_json::from_str(&text).unwrap();
    s["results"] = Value::from(results.to_string_lossy().into_owned());
    s["fixtures"] = Value::from(repo().join(r"crates\anyfont-core\tests\fixtures").to_string_lossy().into_owned());
    s["work"] = Value::from(dir.join("work").to_string_lossy().into_owned());
    s.as_object_mut().unwrap().remove("holders_script");
    // the sandbox is seeded with 6 fonts (arial, arialbd, times, cour, segoeui, cambria), not a whole Windows
    for step in s["steps"].as_array_mut().unwrap() {
        if step["op"] == "list_installed" {
            step["min"] = Value::from(5);
        }
    }
    let file = dir.join("selftest.json");
    std::fs::write(&file, serde_json::to_vec_pretty(&s).unwrap()).unwrap();
    file
}

#[test]
fn the_real_system_script_passes_on_the_sandbox_backend() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("sandbox");
    let results = tmp.path().join("results.json");
    let script = script_for(tmp.path(), &results);
    let out = anyfont(&script).env("ANYFONT_SANDBOX_ROOT", &root).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let r: Value = serde_json::from_slice(&std::fs::read(&results).unwrap_or_else(|e| panic!("no results ({e}): {stdout} {}", String::from_utf8_lossy(&out.stderr))))
        .unwrap();
    let failed: Vec<String> = r["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["ok"] != true)
        .map(|s| format!("{} {}: {}", s["op"], s["args"], s["detail"]))
        .collect();
    assert!(failed.is_empty(), "failed steps:\n{}", failed.join("\n"));
    assert_eq!(out.status.code(), Some(0), "{stdout}");
    assert_eq!(r["steps"].as_array().unwrap().len(), 17);
    let env_step = &r["steps"][0]["detail"];
    assert!(env_step.as_str().unwrap().contains("backend Sandbox"), "{env_step}");
    // the deleted copy went to the sandbox's Recycle Bin, and everything happened inside the sandbox root
    assert_eq!(r["deleted"].as_array().unwrap().len(), 1);
    assert!(root.join(r"Windows\Fonts\arial.ttf").is_file());
    assert!(!root.join(r"Windows\Fonts\Hack-Regular.ttf").exists());
}

#[test]
fn the_real_backend_is_refused_outside_a_disposable_machine() {
    let tmp = tempfile::tempdir().unwrap();
    let results = tmp.path().join("results.json");
    let script = script_for(tmp.path(), &results);
    // no ANYFONT_SANDBOX_ROOT, not a GitHub-hosted runner, not Windows Sandbox: refused before anything happens
    let out = anyfont(&script).output().unwrap();
    assert_eq!(out.status.code(), Some(2), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stderr).contains("refused"));
    assert!(!results.exists(), "a refused self-test writes nothing");
    assert!(!tmp.path().join("work").exists());
    // a self-hosted runner is not disposable either
    let out = anyfont(&script).env("GITHUB_ACTIONS", "true").env("RUNNER_ENVIRONMENT", "self-hosted").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(!results.exists());
}

#[test]
fn the_script_must_say_it_is_disposable_even_on_the_sandbox_backend() {
    let tmp = tempfile::tempdir().unwrap();
    let results = tmp.path().join("results.json");
    let script = script_for(tmp.path(), &results);
    let mut s: Value = serde_json::from_slice(&std::fs::read(&script).unwrap()).unwrap();
    s["i_am_a_disposable_machine"] = Value::from(false);
    std::fs::write(&script, serde_json::to_vec(&s).unwrap()).unwrap();
    let out = anyfont(&script).env("ANYFONT_SANDBOX_ROOT", tmp.path().join("sandbox")).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(!results.exists());
}
