//! Windows integration of the main window: the custom frame (DWM shadow and rounded corners, native hit
//! testing so Windows snap, snap layouts on the maximize button, the system menu and Aero drag work), bringing
//! the window to the front, the app-mode (light/dark) preference.
//!
//! The title bar (`titlebar.rs`) publishes its regions every frame with [`set_hit_regions`] (physical client
//! pixels). A window subclass answers `WM_NCHITTEST` from them: the resize edges, `HTCAPTION` for the empty
//! caption, `HTMAXBUTTON` for the maximize button (Windows 11 snap layouts), `HTCLIENT` elsewhere. Without
//! the subclass (tests, other platforms) the title bar falls back to `ViewportCommand::StartDrag/BeginResize`.

#[cfg(windows)]
pub use win::*;

#[cfg(not(windows))]
mod fallback {
    pub fn install_frame(_hwnd: isize) -> bool {
        false
    }
    pub fn frame_installed() -> bool {
        false
    }
    pub fn set_hit_regions(_r: super::HitRegions) {}
    pub fn max_button_state() -> (bool, bool) {
        (false, false)
    }
    pub fn take_max_click() -> bool {
        false
    }
    pub fn bring_to_front(_hwnd: isize) {}
    pub fn set_border_color(_hwnd: isize, _rgb: [u8; 3]) {}
    pub fn apps_use_light_theme() -> Option<bool> {
        None
    }
    pub fn show_system_menu(_hwnd: isize, _x: i32, _y: i32) {}
    pub fn hwnd_of(_h: &impl raw_window_handle::HasWindowHandle) -> Option<isize> {
        None
    }
}
#[cfg(not(windows))]
pub use fallback::*;

/// Regions of the title bar in physical client pixels (`[x0, y0, x1, y1]`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HitRegions {
    /// The draggable caption (title bar minus its buttons).
    pub caption: [i32; 4],
    /// The maximize/restore button.
    pub max_button: [i32; 4],
    /// Width of the invisible resize border (0 when maximized).
    pub border: i32,
}

#[cfg(windows)]
mod win {
    use super::HitRegions;
    use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
    use std::sync::Mutex;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
    use windows::Win32::Graphics::Dwm::{
        DwmExtendFrameIntoClientArea, DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE,
        DWMWCP_ROUND,
    };
    use windows::Win32::Graphics::Gdi::ScreenToClient;
    use windows::Win32::UI::Controls::MARGINS;
    use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
    use windows::Win32::UI::WindowsAndMessaging::*;

    static REGIONS: Mutex<HitRegions> = Mutex::new(HitRegions { caption: [0; 4], max_button: [0; 4], border: 8 });
    static INSTALLED: AtomicBool = AtomicBool::new(false);
    static MAX_HOT: AtomicBool = AtomicBool::new(false);
    static MAX_PRESSED: AtomicBool = AtomicBool::new(false);
    static MAX_CLICKED: AtomicBool = AtomicBool::new(false);
    static HWND_VAL: AtomicIsize = AtomicIsize::new(0);

    fn hwnd(v: isize) -> HWND {
        HWND(v as *mut core::ffi::c_void)
    }

    /// The HWND of an eframe window.
    pub fn hwnd_of(h: &impl raw_window_handle::HasWindowHandle) -> Option<isize> {
        match h.window_handle().ok()?.as_raw() {
            raw_window_handle::RawWindowHandle::Win32(w) => Some(w.hwnd.get()),
            _ => None,
        }
    }

    /// Installs the custom frame on the main window (once). Returns true when the native hit testing is active.
    pub fn install_frame(h: isize) -> bool {
        if INSTALLED.load(Ordering::Relaxed) {
            return true;
        }
        if h == 0 {
            return false;
        }
        HWND_VAL.store(h, Ordering::Relaxed);
        unsafe {
            let w = hwnd(h);
            // the DWM shadow of a frameless window, and Windows 11 rounded corners
            let m = MARGINS { cxLeftWidth: 0, cxRightWidth: 0, cyTopHeight: 1, cyBottomHeight: 0 };
            let _ = DwmExtendFrameIntoClientArea(w, &m);
            let pref = DWMWCP_ROUND;
            let _ = DwmSetWindowAttribute(
                w,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &pref as *const _ as *const core::ffi::c_void,
                std::mem::size_of_val(&pref) as u32,
            );
            // keep the system menu and the snap behaviour of a normal window
            let style = GetWindowLongPtrW(w, GWL_STYLE);
            let want = style | (WS_SYSMENU.0 | WS_MINIMIZEBOX.0 | WS_MAXIMIZEBOX.0 | WS_THICKFRAME.0) as isize;
            if want != style {
                SetWindowLongPtrW(w, GWL_STYLE, want);
                let _ = SetWindowPos(w, None, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED);
            }
            let ok = SetWindowSubclass(w, Some(subclass), 0xAF01, 0).as_bool();
            INSTALLED.store(ok, Ordering::Relaxed);
            ok
        }
    }

    pub fn frame_installed() -> bool {
        INSTALLED.load(Ordering::Relaxed)
    }

    pub fn set_hit_regions(r: HitRegions) {
        *REGIONS.lock().unwrap_or_else(|e| e.into_inner()) = r;
    }

    /// (hovered, pressed) of the maximize button: the button lives in the non-client area for Windows, so
    /// egui does not see the mouse over it.
    pub fn max_button_state() -> (bool, bool) {
        (MAX_HOT.load(Ordering::Relaxed), MAX_PRESSED.load(Ordering::Relaxed))
    }

    /// True once after the maximize button was clicked (the title bar toggles the window).
    pub fn take_max_click() -> bool {
        MAX_CLICKED.swap(false, Ordering::Relaxed)
    }

    fn inside(r: [i32; 4], x: i32, y: i32) -> bool {
        x >= r[0] && x < r[2] && y >= r[1] && y < r[3]
    }

    unsafe extern "system" fn subclass(w: HWND, msg: u32, wp: WPARAM, lp: LPARAM, _id: usize, _data: usize) -> LRESULT {
        unsafe {
            match msg {
                WM_NCCALCSIZE if wp.0 != 0 => {
                    // the whole window is client area (no system frame); a maximized window must not spill
                    // over the monitor edges
                    let params = &mut *(lp.0 as *mut NCCALCSIZE_PARAMS);
                    if IsZoomed(w).as_bool() {
                        let frame_x = GetSystemMetrics(SM_CXSIZEFRAME) + GetSystemMetrics(SM_CXPADDEDBORDER);
                        let frame_y = GetSystemMetrics(SM_CYSIZEFRAME) + GetSystemMetrics(SM_CXPADDEDBORDER);
                        params.rgrc[0].left += frame_x;
                        params.rgrc[0].right -= frame_x;
                        params.rgrc[0].top += frame_y;
                        params.rgrc[0].bottom -= frame_y;
                    }
                    return LRESULT(0);
                }
                WM_NCHITTEST => {
                    let sx = (lp.0 & 0xFFFF) as i16 as i32;
                    let sy = ((lp.0 >> 16) & 0xFFFF) as i16 as i32;
                    let mut p = POINT { x: sx, y: sy };
                    let _ = ScreenToClient(w, &mut p);
                    let mut rc = RECT::default();
                    let _ = GetClientRect(w, &mut rc);
                    let r = *REGIONS.lock().unwrap_or_else(|e| e.into_inner());
                    let b = if IsZoomed(w).as_bool() { 0 } else { r.border };
                    if b > 0 {
                        let (l, t, rr, bb) = (p.x < b, p.y < b, p.x >= rc.right - b, p.y >= rc.bottom - b);
                        let hit = match (l, t, rr, bb) {
                            (true, true, _, _) => Some(HTTOPLEFT),
                            (_, true, true, _) => Some(HTTOPRIGHT),
                            (true, _, _, true) => Some(HTBOTTOMLEFT),
                            (_, _, true, true) => Some(HTBOTTOMRIGHT),
                            (true, _, _, _) => Some(HTLEFT),
                            (_, _, true, _) => Some(HTRIGHT),
                            (_, true, _, _) => Some(HTTOP),
                            (_, _, _, true) => Some(HTBOTTOM),
                            _ => None,
                        };
                        if let Some(h) = hit {
                            return LRESULT(h as isize);
                        }
                    }
                    if inside(r.max_button, p.x, p.y) {
                        return LRESULT(HTMAXBUTTON as isize);
                    }
                    if inside(r.caption, p.x, p.y) {
                        return LRESULT(HTCAPTION as isize);
                    }
                    if MAX_HOT.swap(false, Ordering::Relaxed) {
                        request_repaint(w);
                    }
                    return LRESULT(HTCLIENT as isize);
                }
                WM_NCMOUSEMOVE => {
                    let hot = wp.0 as u32 == HTMAXBUTTON;
                    if MAX_HOT.swap(hot, Ordering::Relaxed) != hot {
                        request_repaint(w);
                    }
                    if !hot {
                        MAX_PRESSED.store(false, Ordering::Relaxed);
                    }
                    if hot {
                        // be told when the pointer leaves the non-client area
                        let mut tme = windows::Win32::UI::Input::KeyboardAndMouse::TRACKMOUSEEVENT {
                            cbSize: std::mem::size_of::<windows::Win32::UI::Input::KeyboardAndMouse::TRACKMOUSEEVENT>() as u32,
                            dwFlags: windows::Win32::UI::Input::KeyboardAndMouse::TME_LEAVE
                                | windows::Win32::UI::Input::KeyboardAndMouse::TME_NONCLIENT,
                            hwndTrack: w,
                            dwHoverTime: 0,
                        };
                        let _ = windows::Win32::UI::Input::KeyboardAndMouse::TrackMouseEvent(&mut tme);
                        return LRESULT(0);
                    }
                }
                WM_NCMOUSELEAVE => {
                    MAX_PRESSED.store(false, Ordering::Relaxed);
                    if MAX_HOT.swap(false, Ordering::Relaxed) {
                        request_repaint(w);
                    }
                }
                WM_NCLBUTTONDOWN if wp.0 as u32 == HTMAXBUTTON => {
                    MAX_PRESSED.store(true, Ordering::Relaxed);
                    request_repaint(w);
                    return LRESULT(0);
                }
                WM_NCLBUTTONDBLCLK if wp.0 as u32 == HTMAXBUTTON => return LRESULT(0),
                WM_NCLBUTTONUP if wp.0 as u32 == HTMAXBUTTON => {
                    if MAX_PRESSED.swap(false, Ordering::Relaxed) {
                        MAX_CLICKED.store(true, Ordering::Relaxed);
                        let cmd = if IsZoomed(w).as_bool() { SW_RESTORE } else { SW_MAXIMIZE };
                        let _ = ShowWindow(w, cmd);
                    }
                    request_repaint(w);
                    return LRESULT(0);
                }
                WM_NCRBUTTONUP if wp.0 as u32 == HTCAPTION => {
                    let x = (lp.0 & 0xFFFF) as i16 as i32;
                    let y = ((lp.0 >> 16) & 0xFFFF) as i16 as i32;
                    show_system_menu(w.0 as isize, x, y);
                    return LRESULT(0);
                }
                _ => {}
            }
            DefSubclassProc(w, msg, wp, lp)
        }
    }

    fn request_repaint(w: HWND) {
        unsafe {
            let _ = windows::Win32::Graphics::Gdi::InvalidateRect(Some(w), None, false);
        }
    }

    /// The window's system menu at a screen position (title-bar right-click).
    pub fn show_system_menu(h: isize, x: i32, y: i32) {
        unsafe {
            let w = hwnd(h);
            let menu = GetSystemMenu(w, false);
            if menu.is_invalid() {
                return;
            }
            let zoomed = IsZoomed(w).as_bool();
            let set = |id: u32, enabled: bool| {
                let flags = if enabled { MF_ENABLED } else { MF_GRAYED };
                let _ = EnableMenuItem(menu, id, MF_BYCOMMAND | flags);
            };
            set(SC_RESTORE, zoomed);
            set(SC_MOVE, !zoomed);
            set(SC_SIZE, !zoomed);
            set(SC_MINIMIZE, true);
            set(SC_MAXIMIZE, !zoomed);
            set(SC_CLOSE, true);
            let cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, x, y, None, w, None);
            if cmd.0 != 0 {
                let _ = PostMessageW(Some(w), WM_SYSCOMMAND, WPARAM(cmd.0 as usize), LPARAM(0));
            }
        }
    }

    /// Restores the window if minimized and brings it to the front (single instance).
    pub fn bring_to_front(h: isize) {
        let h = if h != 0 { h } else { HWND_VAL.load(Ordering::Relaxed) };
        if h == 0 {
            return;
        }
        unsafe {
            let w = hwnd(h);
            if IsIconic(w).as_bool() {
                let _ = ShowWindow(w, SW_RESTORE);
            }
            let _ = SetForegroundWindow(w);
        }
    }

    /// The 1 px window border colour (Options ▸ Window border), via DWM (Windows 11).
    pub fn set_border_color(h: isize, rgb: [u8; 3]) {
        if h == 0 {
            return;
        }
        let colorref: u32 = rgb[0] as u32 | ((rgb[1] as u32) << 8) | ((rgb[2] as u32) << 16);
        unsafe {
            let _ = DwmSetWindowAttribute(
                hwnd(h),
                DWMWA_BORDER_COLOR,
                &colorref as *const u32 as *const core::ffi::c_void,
                4,
            );
        }
    }

    /// `HKCU\...\Themes\Personalize\AppsUseLightTheme`.
    pub fn apps_use_light_theme() -> Option<bool> {
        use windows::core::w;
        use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
        let mut v: u32 = 0;
        let mut size = 4u32;
        let r = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
                w!("AppsUseLightTheme"),
                RRF_RT_REG_DWORD,
                None,
                Some(&mut v as *mut u32 as *mut core::ffi::c_void),
                Some(&mut size),
            )
        };
        (r.0 == 0).then_some(v != 0)
    }
}
