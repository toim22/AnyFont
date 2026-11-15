//! AnyFont – font manager for Windows. The start-up sequence: the elevated install helper
//! (`/InstallHelper:`), the single instance (later instances forward their files and exit), logging and the
//! crash hook, the font system, the settings, then the main window (which starts as the splash). After the
//! window closes, the updater's setup runs when it asked for it.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyfont::app::{AnyFontApp, StartOptions};
use anyfont::settings::{AppSettings, Paths};
use anyfont_sys::single_instance::{self, Instance, Message};
use anyfont_sys::CommandLine;
use parking_lot::Mutex;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

const APP: &str = "AnyFont";
const ICON_PNG: &[u8] = include_bytes!("../assets/icon-256.png");

fn window_icon() -> Option<egui::IconData> {
    let img = image::load_from_memory(ICON_PNG).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some(egui::IconData { rgba: img.into_raw(), width: w, height: h })
}

/// True while Shift is held (reset of the font database at start-up, UI-SPEC §11 #20).
fn shift_held() -> bool {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_SHIFT};
        return (GetAsyncKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000) != 0;
    }
    #[allow(unreachable_code)]
    false
}

fn main() -> eframe::Result<()> {
    let cl = CommandLine::from_env();
    if let Some(request) = cl.install_helper() {
        let _ = anyfont_sys::logging::init();
        std::process::exit(anyfont_sys::helper::helper_main(&request));
    }
    if let Err(e) = anyfont_sys::logging::init() {
        eprintln!("AnyFont: cannot open the log file: {e}");
    }
    anyfont_sys::crash::install_panic_hook();
    for s in cl.unknown_switches() {
        log::info!("ignoring the switch /{}", s.name);
    }

    // single instance: a later instance hands its files to the first one and exits
    let ctx_cell: Arc<OnceLock<egui::Context>> = Arc::new(OnceLock::new());
    let (tx, rx) = std::sync::mpsc::channel::<Message>();
    let _guard = match single_instance::acquire(APP) {
        Ok(Instance::Secondary) => {
            let msg = Message::from_command_line(&cl);
            match single_instance::forward_to_primary(APP, &msg, Duration::from_secs(5)) {
                Ok(()) => {
                    log::info!("AnyFont is already running: arguments forwarded");
                    return Ok(());
                }
                Err(e) => {
                    log::warn!("cannot reach the running instance ({e}); starting anyway");
                    None
                }
            }
        }
        Ok(Instance::Primary(guard)) => {
            let cell = ctx_cell.clone();
            if let Err(e) = guard.listen(move |m| {
                let _ = tx.send(m);
                anyfont::platform::bring_to_front(0);
                if let Some(ctx) = cell.get() {
                    ctx.request_repaint();
                }
            }) {
                log::warn!("single instance listener: {e}");
            }
            Some(guard)
        }
        Err(e) => {
            log::warn!("single instance check failed: {e}");
            None
        }
    };

    let fs: Arc<dyn anyfont_sys::FontSystem> = Arc::from(anyfont_sys::system());
    let removed = fs.process_pending();
    if removed > 0 {
        log::info!("{removed} font file(s) deleted after the restart");
    }
    let paths = Paths::resolve(fs.paths());
    if shift_held() {
        log::warn!("Shift held at start-up: the font database is rebuilt");
        let _ = std::fs::remove_file(&paths.db_file);
    }
    let settings = AppSettings::load(&paths.settings_file);
    let crash_report = anyfont_sys::crash::pending_reports().into_iter().next();
    let splash = !cl.no_splash();
    let updater_out = Arc::new(Mutex::new(None));

    let w = &settings.core.window;
    let mut viewport = egui::ViewportBuilder::default().with_title(APP).with_app_id(APP).with_decorations(false);
    if splash {
        viewport = viewport.with_inner_size([640.0, 360.0]).with_resizable(false);
    } else {
        viewport = viewport.with_inner_size([w.size[0].max(800.0), w.size[1].max(600.0)]).with_min_inner_size([800.0, 600.0]).with_maximized(w.maximized);
        if let Some(p) = w.position {
            viewport = viewport.with_position([p[0], p[1]]);
        }
    }
    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(Arc::new(icon));
    }
    let options = eframe::NativeOptions { viewport, centered: splash || w.position.is_none(), persist_window: false, renderer: eframe::Renderer::Wgpu, ..Default::default() };
    let opts = StartOptions {
        fs,
        paths,
        settings,
        files: cl.forwardable_files(),
        lang: cl.lang().map(str::to_string),
        splash,
        crash_report,
        incoming: Some(rx),
        updater_out: updater_out.clone(),
    };
    let cell = ctx_cell.clone();
    let result = eframe::run_native(
        APP,
        options,
        Box::new(move |cc| {
            let _ = cell.set(cc.egui_ctx.clone());
            Ok(Box::new(AnyFontApp::new(cc, opts)))
        }),
    );
    if let Err(e) = &result {
        log::error!("the window failed: {e}");
    }
    let setup = updater_out.lock().take();
    if let Some(setup) = setup {
        anyfont::updater::run_setup_after_exit(&setup);
    }
    result
}
