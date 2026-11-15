//! A tiny HTTP/1.1 server on 127.0.0.1 (std `TcpListener`, one thread) standing in for GitHub, the issue
//! relay, the font catalog and the update downloads. Routes answer fixed responses; every request is
//! recorded (method, path, headers, body) for the test to inspect.
#![allow(dead_code)]

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Request {
    pub method: String,
    pub path: String,
    /// Header names in lower case.
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(&name.to_ascii_lowercase()).map(String::as_str)
    }
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
    pub fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).expect("JSON body")
    }
}

#[derive(Clone, Debug)]
pub struct Response {
    pub status: u16,
    pub content_type: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn new(status: u16, content_type: &str, body: impl Into<Vec<u8>>) -> Response {
        Response { status, content_type: content_type.into(), headers: Vec::new(), body: body.into() }
    }
    pub fn json(status: u16, v: &serde_json::Value) -> Response {
        Response::new(status, "application/json", serde_json::to_vec(v).unwrap())
    }
    pub fn bytes(body: impl Into<Vec<u8>>) -> Response {
        Response::new(200, "application/octet-stream", body)
    }
    pub fn header(mut self, k: &str, v: &str) -> Response {
        self.headers.push((k.into(), v.into()));
        self
    }
}

type Routes = Arc<Mutex<Vec<(String, String, Response)>>>;

pub struct Server {
    pub port: u16,
    routes: Routes,
    requests: Arc<Mutex<Vec<Request>>>,
    stop: Arc<AtomicBool>,
}

impl Server {
    /// Starts the server on a free port.
    pub fn start() -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind 127.0.0.1");
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let routes: Routes = Arc::new(Mutex::new(Vec::new()));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (r2, q2, s2) = (routes.clone(), requests.clone(), stop.clone());
        std::thread::spawn(move || {
            while !s2.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let (r3, q3) = (r2.clone(), q2.clone());
                        std::thread::spawn(move || {
                            let _ = handle(stream, &r3, &q3);
                        });
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(10)),
                    Err(_) => std::thread::sleep(Duration::from_millis(10)),
                }
            }
        });
        Server { port, routes, requests, stop }
    }

    /// `http://127.0.0.1:<port><path>`.
    pub fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{}", self.port, path)
    }

    /// Answers `method` (`"*"` = any) requests whose path (without the query) is `path` (or starts with it,
    /// when `path` ends with `*`). Later routes win.
    pub fn route(&self, method: &str, path: &str, resp: Response) {
        self.routes.lock().unwrap().push((method.to_string(), path.to_string(), resp));
    }

    pub fn requests(&self) -> Vec<Request> {
        self.requests.lock().unwrap().clone()
    }

    /// Requests to this path (without the query).
    pub fn requests_to(&self, path: &str) -> Vec<Request> {
        self.requests().into_iter().filter(|r| r.path.split('?').next() == Some(path)).collect()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

fn handle(stream: TcpStream, routes: &Routes, requests: &Arc<Mutex<Vec<Request>>>) -> std::io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").to_string();
    let mut headers = HashMap::new();
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h)? == 0 {
            break;
        }
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }
    let mut body = Vec::new();
    if let Some(n) = headers.get("content-length").and_then(|v| v.parse::<usize>().ok()) {
        body.resize(n, 0);
        reader.read_exact(&mut body)?;
    } else if headers.get("transfer-encoding").map(|v| v.eq_ignore_ascii_case("chunked")).unwrap_or(false) {
        loop {
            let mut size = String::new();
            reader.read_line(&mut size)?;
            let n = usize::from_str_radix(size.trim().split(';').next().unwrap_or("0"), 16).unwrap_or(0);
            let mut chunk = vec![0; n + 2];
            reader.read_exact(&mut chunk)?;
            if n == 0 {
                break;
            }
            body.extend_from_slice(&chunk[..n]);
        }
    }
    let req = Request { method: method.clone(), path: path.clone(), headers, body };
    requests.lock().unwrap().push(req);
    let bare = path.split('?').next().unwrap_or("").to_string();
    let resp = routes
        .lock()
        .unwrap()
        .iter()
        .rev()
        .find(|(m, p, _)| (m == "*" || m.eq_ignore_ascii_case(&method)) && (p == &bare || (p.ends_with('*') && bare.starts_with(&p[..p.len() - 1]))))
        .map(|(_, _, r)| r.clone())
        .unwrap_or_else(|| Response::new(404, "text/plain", "not found"));
    let mut out = stream;
    let reason = match resp.status {
        200 => "OK",
        201 => "Created",
        400 => "Bad Request",
        404 => "Not Found",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        _ => "Status",
    };
    let mut head = format!("HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n", resp.status, reason, resp.content_type, resp.body.len());
    for (k, v) in &resp.headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str("\r\n");
    out.write_all(head.as_bytes())?;
    out.write_all(&resp.body)?;
    out.flush()?;
    Ok(())
}
