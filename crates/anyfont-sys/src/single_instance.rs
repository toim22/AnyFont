//! Single instance: a second `anyfont.exe` hands its file arguments to the running one and exits.
//!
//! The original registers a DDE service (topic `System`) and uses named kernel events
//! (`CrlUtils::InterProcessEvent`) so that a document opened from the shell goes to the running
//! instance (dossier `10-behavior/shell-clipboard-printing.md` §2/2b). AnyFont does the same with
//! a named mutex (who is first) and a named pipe (the message):
//!
//! - mutex `Local\AnyFont.<hash>.SingleInstance` (per session);
//! - pipe `\\.\pipe\AnyFont.<hash>` (local clients only, default DACL = the user, SYSTEM and
//!   Administrators), carrying one [`Message`] per connection as a `u32` little-endian length
//!   followed by JSON; the primary answers with a 1-byte acknowledgement, so the sender never
//!   closes before the message has been read;
//! - `<hash>` covers the user name, the session and the sandbox root, so a sandboxed test
//!   instance never talks to a real one.
//!
//! The secondary calls `AllowSetForegroundWindow(ASFW_ANY)` before sending so the primary may
//! bring its window to the front (the UI does that in its message callback).

use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// What a second instance sends to the first.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    /// Absolute paths of the font files / folders given on its command line.
    pub files: Vec<PathBuf>,
    /// Its raw switches (`/NoSplash`, ...), for information.
    pub switches: Vec<String>,
}

impl Message {
    pub fn from_command_line(cl: &crate::CommandLine) -> Message {
        Message {
            files: cl.forwardable_files(),
            switches: cl
                .switches
                .iter()
                .map(|s| match &s.value {
                    Some(v) => format!("/{}:{}", s.name, v),
                    None => format!("/{}", s.name),
                })
                .collect(),
        }
    }
}

const MAX_MESSAGE: u32 = 1024 * 1024;
const ACK: u8 = 0x06;

fn read_frame(r: &mut impl Read) -> io::Result<Vec<u8>> {
    let mut len = [0u8; 4];
    r.read_exact(&mut len)?;
    let len = u32::from_le_bytes(len);
    if len > MAX_MESSAGE {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "message too large"));
    }
    let mut buf = vec![0u8; len as usize];
    r.read_exact(&mut buf)?;
    Ok(buf)
}

/// Identifier of this user + session (+ sandbox).
pub fn instance_id(app: &str) -> String {
    let user = format!(
        "{}\\{}",
        std::env::var("USERDOMAIN").unwrap_or_default(),
        std::env::var("USERNAME").unwrap_or_default()
    )
    .to_lowercase();
    let sandbox = std::env::var(crate::SANDBOX_ENV).unwrap_or_default().to_lowercase();
    let seed = format!("{app}|{user}|{}|{sandbox}", session_id());
    format!("{app}.{:016x}", crate::util::fnv1a64(seed.as_bytes()))
}

fn session_id() -> u32 {
    #[cfg(windows)]
    {
        let mut id = 0u32;
        unsafe {
            let _ = windows::Win32::System::RemoteDesktop::ProcessIdToSessionId(std::process::id(), &mut id);
        }
        id
    }
    #[cfg(not(windows))]
    {
        0
    }
}

pub fn pipe_name(app: &str) -> String {
    format!(r"\\.\pipe\{}", instance_id(app))
}

pub fn mutex_name(app: &str) -> String {
    format!(r"Local\{}.SingleInstance", instance_id(app))
}

/// Result of [`acquire`].
pub enum Instance {
    /// This process is the first one; keep the guard alive for the lifetime of the app.
    Primary(PrimaryGuard),
    /// Another instance runs: call [`forward_to_primary`] and exit.
    Secondary,
}

/// Owns the single-instance mutex.
pub struct PrimaryGuard {
    app: String,
    #[cfg(windows)]
    mutex: windows::Win32::Foundation::HANDLE,
}

// The mutex handle is only closed on drop.
unsafe impl Send for PrimaryGuard {}
unsafe impl Sync for PrimaryGuard {}

impl Drop for PrimaryGuard {
    fn drop(&mut self) {
        #[cfg(windows)]
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.mutex);
        }
    }
}

/// Become the primary instance, or learn that one exists. `app` is a short name (`"AnyFont"`).
pub fn acquire(app: &str) -> io::Result<Instance> {
    #[cfg(windows)]
    {
        use windows::core::HSTRING;
        use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
        use windows::Win32::System::Threading::CreateMutexW;
        let h = unsafe { CreateMutexW(None, false, &HSTRING::from(mutex_name(app))) }.map_err(io::Error::other)?;
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            unsafe {
                let _ = windows::Win32::Foundation::CloseHandle(h);
            }
            return Ok(Instance::Secondary);
        }
        Ok(Instance::Primary(PrimaryGuard { app: app.to_string(), mutex: h }))
    }
    #[cfg(not(windows))]
    {
        Ok(Instance::Primary(PrimaryGuard { app: app.to_string() }))
    }
}

impl PrimaryGuard {
    /// Start the pipe server thread; `on_message` runs on that thread for every message from a
    /// later instance (forward it to the UI thread and request a repaint / focus).
    pub fn listen<F>(&self, on_message: F) -> io::Result<()>
    where
        F: Fn(Message) + Send + 'static,
    {
        #[cfg(windows)]
        {
            let name = pipe_name(&self.app);
            // Create the first instance here so errors (e.g. squatting) surface to the caller.
            let first = server::create(&name, true)?;
            std::thread::Builder::new().name("anyfont-instance-pipe".into()).spawn(move || server::run(name, first, on_message))?;
            Ok(())
        }
        #[cfg(not(windows))]
        {
            let _ = on_message;
            Ok(())
        }
    }
}

#[cfg(windows)]
mod server {
    use super::*;
    use std::os::windows::io::FromRawHandle;
    use windows::core::HSTRING;
    use windows::Win32::Foundation::{CloseHandle, ERROR_PIPE_CONNECTED, HANDLE, INVALID_HANDLE_VALUE};
    use windows::Win32::Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX};
    use windows::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE,
        PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
    };

    pub(super) struct PipeHandle(pub HANDLE);
    unsafe impl Send for PipeHandle {}

    pub(super) fn create(name: &str, first: bool) -> io::Result<PipeHandle> {
        let mut open = PIPE_ACCESS_DUPLEX;
        if first {
            open |= FILE_FLAG_FIRST_PIPE_INSTANCE;
        }
        let h = unsafe {
            CreateNamedPipeW(
                &HSTRING::from(name),
                open,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_UNLIMITED_INSTANCES,
                64,
                64 * 1024,
                0,
                None,
            )
        };
        if h == INVALID_HANDLE_VALUE || h.is_invalid() {
            return Err(io::Error::last_os_error());
        }
        Ok(PipeHandle(h))
    }

    pub(super) fn run<F: Fn(Message)>(name: String, mut next: PipeHandle, on_message: F) {
        loop {
            let connected = match unsafe { ConnectNamedPipe(next.0, None) } {
                Ok(()) => true,
                Err(e) => (e.code().0 & 0xFFFF) as u32 == ERROR_PIPE_CONNECTED.0,
            };
            let h = next.0;
            if connected {
                let mut file = unsafe { std::fs::File::from_raw_handle(h.0 as _) };
                let msg = read_frame(&mut file);
                if msg.is_ok() {
                    let _ = file.write_all(&[ACK]).and_then(|_| file.flush());
                }
                unsafe {
                    let _ = DisconnectNamedPipe(h);
                }
                drop(file); // closes the handle
                match msg.and_then(|buf| serde_json::from_slice::<Message>(&buf).map_err(io::Error::other)) {
                    Ok(m) => on_message(m),
                    Err(e) => log::warn!("single instance: ignoring a malformed message ({e})"),
                }
            } else {
                unsafe {
                    let _ = CloseHandle(h);
                }
            }
            next = match create(&name, false) {
                Ok(p) => p,
                Err(e) => {
                    log::error!("single instance: pipe server stopped: {e}");
                    return;
                }
            };
        }
    }
}

/// Send `msg` to the primary instance (retrying while it is busy, up to `timeout`).
pub fn forward_to_primary(app: &str, msg: &Message, timeout: Duration) -> io::Result<()> {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{AllowSetForegroundWindow, ASFW_ANY};
        let _ = AllowSetForegroundWindow(ASFW_ANY);
    }
    let data = serde_json::to_vec(msg).map_err(io::Error::other)?;
    if data.len() > MAX_MESSAGE as usize {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "message too large"));
    }
    let name = pipe_name(app);
    let deadline = Instant::now() + timeout;
    loop {
        match std::fs::OpenOptions::new().read(true).write(true).open(&name) {
            Ok(mut f) => {
                let mut frame = (data.len() as u32).to_le_bytes().to_vec();
                frame.extend_from_slice(&data);
                f.write_all(&frame)?;
                f.flush()?;
                let mut ack = [0u8; 1];
                f.read_exact(&mut ack)?;
                return if ack[0] == ACK { Ok(()) } else { Err(io::Error::new(io::ErrorKind::InvalidData, "unexpected reply")) };
            }
            // ERROR_PIPE_BUSY (231) or not created yet (2): retry until the deadline.
            Err(e) if matches!(e.raw_os_error(), Some(231) | Some(2)) && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn second_instance_forwards_files() {
        let app = format!("AnyFontTest{}", std::process::id());
        let first = match acquire(&app).unwrap() {
            Instance::Primary(g) => g,
            Instance::Secondary => panic!("first acquire must be primary"),
        };
        assert!(matches!(acquire(&app).unwrap(), Instance::Secondary));
        let (tx, rx) = mpsc::channel();
        first.listen(move |m| tx.send(m).unwrap()).unwrap();
        let cl = crate::CommandLine::parse([r"C:\Fonts\a.ttf", "/NoSplash"]);
        let msg = Message::from_command_line(&cl);
        forward_to_primary(&app, &msg, Duration::from_secs(5)).unwrap();
        let got = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(got.files, vec![PathBuf::from(r"C:\Fonts\a.ttf")]);
        assert_eq!(got.switches, vec!["/NoSplash".to_string()]);
        // A second message on a fresh pipe instance.
        forward_to_primary(&app, &Message { files: vec![PathBuf::from(r"C:\b.otf")], switches: vec![] }, Duration::from_secs(5)).unwrap();
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap().files, vec![PathBuf::from(r"C:\b.otf")]);
        drop(first);
        assert!(matches!(acquire(&app).unwrap(), Instance::Primary(_)));
    }

    #[test]
    fn ids_depend_on_app_name() {
        assert_ne!(instance_id("A"), instance_id("B"));
        assert!(pipe_name("AnyFont").starts_with(r"\\.\pipe\AnyFont."));
        assert!(mutex_name("AnyFont").starts_with(r"Local\AnyFont."));
    }
}
