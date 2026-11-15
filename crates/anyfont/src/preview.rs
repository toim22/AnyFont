//! Font previews rendered in the font itself, off the UI thread.
//!
//! The font list and the glyph grid ask for the images they need **this frame** (visible rows only); a small
//! pool of worker threads shapes the text with rustybuzz, draws the outlines with tiny-skia and sends back an
//! alpha mask, which the UI uploads as a texture and tints with the text colour. Textures live in an LRU
//! cache; until an image is ready the caller draws a placeholder. Variable-font instances are rendered with
//! their axis coordinates (`set_variation`).

use egui::{Context, TextureHandle};
use parking_lot::{Condvar, Mutex};
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

/// What to draw.
#[derive(Clone, Debug, PartialEq)]
pub enum Content {
    /// A line of text, shaped.
    Text(String),
    /// One glyph by id, centred in a square cell of `size` pixels.
    Glyph(u16),
}

/// One render request.
#[derive(Clone, Debug)]
pub struct Request {
    pub path: PathBuf,
    pub face_index: u32,
    /// Axis coordinates (tag, value) for variable fonts.
    pub variations: Vec<([u8; 4], f32)>,
    pub content: Content,
    /// Font size in physical pixels.
    pub px: f32,
    /// Maximum image width in physical pixels (text is cut there).
    pub max_width: u32,
}

impl Request {
    /// A stable key for the cache.
    pub fn key(&self) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.path.hash(&mut h);
        self.face_index.hash(&mut h);
        for (t, v) in &self.variations {
            t.hash(&mut h);
            ((v * 100.0).round() as i64).hash(&mut h);
        }
        match &self.content {
            Content::Text(s) => {
                0u8.hash(&mut h);
                s.hash(&mut h);
            }
            Content::Glyph(g) => {
                1u8.hash(&mut h);
                g.hash(&mut h);
            }
        }
        ((self.px * 4.0).round() as i64).hash(&mut h);
        self.max_width.hash(&mut h);
        h.finish()
    }
}

/// A rendered mask.
#[derive(Clone, Debug)]
pub struct Rendered {
    pub width: u32,
    pub height: u32,
    pub alpha: Vec<u8>,
    /// Distance from the image top to the baseline, physical pixels (text only).
    pub baseline: f32,
}

/// A cached preview texture.
#[derive(Clone)]
pub struct Preview {
    pub texture: TextureHandle,
    /// Size in physical pixels.
    pub size: [u32; 2],
    pub baseline: f32,
}

enum Slot {
    Ready(Preview),
    /// The font could not draw it (Type 1, unreadable file, no glyphs): the caller draws a fallback.
    Failed,
}

struct Shared {
    queue: Mutex<VecDeque<(u64, Request)>>,
    wake: Condvar,
    stop: std::sync::atomic::AtomicBool,
    files: Mutex<FileCache>,
    /// Keys a worker is rendering right now.
    running: Mutex<HashSet<u64>>,
}

#[derive(Default)]
struct FileCache {
    map: HashMap<PathBuf, (Arc<Vec<u8>>, u64)>,
    tick: u64,
    bytes: usize,
}

impl FileCache {
    const BUDGET: usize = 96 * 1024 * 1024;

    fn get(&mut self, path: &Path) -> Option<Arc<Vec<u8>>> {
        self.tick += 1;
        let tick = self.tick;
        if let Some((d, t)) = self.map.get_mut(path) {
            *t = tick;
            return Some(d.clone());
        }
        let raw = std::fs::read(path).ok()?;
        let data = if raw.starts_with(b"wOFF") { anyfont_core::woff::decode_woff(&raw).ok()? } else { raw };
        let data = Arc::new(data);
        self.bytes += data.len();
        self.map.insert(path.to_path_buf(), (data.clone(), tick));
        while self.bytes > Self::BUDGET && self.map.len() > 1 {
            let oldest = self.map.iter().min_by_key(|(_, (_, t))| *t).map(|(k, _)| k.clone());
            match oldest {
                Some(k) => {
                    if let Some((d, _)) = self.map.remove(&k) {
                        self.bytes -= d.len();
                    }
                }
                None => break,
            }
        }
        Some(data)
    }
}

/// The preview service of one window.
pub struct Previews {
    shared: Arc<Shared>,
    rx: Receiver<(u64, Option<Rendered>)>,
    slots: HashMap<u64, (Slot, u64)>,
    wanted: Vec<(u64, Request)>,
    wanted_keys: HashSet<u64>,
    frame: u64,
    workers: Vec<std::thread::JoinHandle<()>>,
}

const MAX_TEXTURES: usize = 900;
const UPLOADS_PER_FRAME: usize = 48;

impl Previews {
    pub fn new(ctx: &Context) -> Previews {
        let shared = Arc::new(Shared {
            queue: Mutex::new(VecDeque::new()),
            wake: Condvar::new(),
            stop: std::sync::atomic::AtomicBool::new(false),
            files: Mutex::new(FileCache::default()),
            running: Mutex::new(HashSet::new()),
        });
        let (tx, rx) = channel();
        let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(2, 6) - 1;
        let workers = (0..n.max(1))
            .map(|i| {
                let shared = shared.clone();
                let tx: Sender<(u64, Option<Rendered>)> = tx.clone();
                let ctx = ctx.clone();
                std::thread::Builder::new()
                    .name(format!("anyfont-preview-{i}"))
                    .spawn(move || worker(shared, tx, ctx))
                    .expect("preview worker")
            })
            .collect();
        Previews {
            shared,
            rx,
            slots: HashMap::new(),
            wanted: Vec::new(),
            wanted_keys: HashSet::new(),
            frame: 0,
            workers,
        }
    }

    /// Takes finished renders (a limited number per frame) and uploads them.
    pub fn begin_frame(&mut self, ctx: &Context) {
        self.frame += 1;
        let mut uploads = 0;
        while uploads < UPLOADS_PER_FRAME {
            let Ok((key, r)) = self.rx.try_recv() else { break };
            let slot = match r {
                Some(r) if r.width > 0 && r.height > 0 => {
                    let pixels = r.alpha.iter().map(|a| egui::Color32::from_white_alpha(*a)).collect();
                    let img = egui::ColorImage::new([r.width as usize, r.height as usize], pixels);
                    let texture = ctx.load_texture(format!("preview-{key:x}"), img, egui::TextureOptions::LINEAR);
                    uploads += 1;
                    Slot::Ready(Preview { texture, size: [r.width, r.height], baseline: r.baseline })
                }
                _ => Slot::Failed,
            };
            self.slots.insert(key, (slot, self.frame));
        }
        if uploads == UPLOADS_PER_FRAME || self.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
        if self.slots.len() > MAX_TEXTURES {
            let mut v: Vec<(u64, u64)> = self.slots.iter().map(|(k, (_, f))| (*k, *f)).collect();
            v.sort_by_key(|(_, f)| *f);
            let drop = self.slots.len() - MAX_TEXTURES * 3 / 4;
            for (k, f) in v.into_iter().take(drop) {
                if f + 2 < self.frame {
                    self.slots.remove(&k);
                }
            }
        }
    }

    /// The preview for a request: `Some(Ok)` when ready, `Some(Err)` when it cannot be drawn, `None` while
    /// it is being rendered (the request is queued for this frame).
    pub fn get(&mut self, req: &Request) -> Option<Result<Preview, ()>> {
        let key = req.key();
        if let Some((slot, used)) = self.slots.get_mut(&key) {
            *used = self.frame;
            return Some(match slot {
                Slot::Ready(p) => Ok(p.clone()),
                Slot::Failed => Err(()),
            });
        }
        if self.wanted_keys.insert(key) {
            self.wanted.push((key, req.clone()));
        }
        None
    }

    /// Hands this frame's wanted renders to the workers (replacing older, no longer visible requests).
    pub fn end_frame(&mut self) {
        let running = self.shared.running.lock().clone();
        let mut q = self.shared.queue.lock();
        q.clear();
        for (k, r) in self.wanted.drain(..) {
            if !running.contains(&k) {
                q.push_back((k, r));
            }
        }
        self.wanted_keys.clear();
        if !q.is_empty() {
            self.shared.wake.notify_all();
        }
    }

    /// Forgets every cached image (theme or scale change, rescans).
    pub fn clear(&mut self) {
        self.slots.clear();
    }

    /// True while renders are queued or running.
    pub fn busy(&self) -> bool {
        !self.shared.running.lock().is_empty() || !self.shared.queue.lock().is_empty()
    }
}

impl Drop for Previews {
    fn drop(&mut self) {
        self.shared.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        self.shared.wake.notify_all();
        for w in self.workers.drain(..) {
            let _ = w.join();
        }
    }
}

fn worker(shared: Arc<Shared>, tx: Sender<(u64, Option<Rendered>)>, ctx: Context) {
    loop {
        let job = {
            let mut q = shared.queue.lock();
            loop {
                if shared.stop.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
                if let Some(j) = q.pop_front() {
                    shared.running.lock().insert(j.0);
                    break j;
                }
                shared.wake.wait_for(&mut q, std::time::Duration::from_millis(500));
            }
        };
        let (key, req) = job;
        let data = shared.files.lock().get(&req.path);
        let out = data.and_then(|d| std::panic::catch_unwind(|| render(&d, &req)).ok().flatten());
        let sent = tx.send((key, out));
        shared.running.lock().remove(&key);
        if sent.is_err() {
            return;
        }
        ctx.request_repaint();
    }
}

/// Renders a request into an alpha mask.
pub fn render(data: &[u8], req: &Request) -> Option<Rendered> {
    let mut face = rustybuzz::Face::from_slice(data, req.face_index)?;
    if !req.variations.is_empty() {
        let vars: Vec<rustybuzz::Variation> = req
            .variations
            .iter()
            .map(|(t, v)| rustybuzz::Variation { tag: ttf_parser::Tag::from_bytes(t), value: *v })
            .collect();
        face.set_variations(&vars);
    }
    let upem = face.units_per_em().max(1) as f32;
    let scale = req.px / upem;
    match &req.content {
        Content::Glyph(gid) => render_glyph(&face, *gid, scale, req.px),
        Content::Text(text) => render_text(&face, text, scale, req.max_width),
    }
}

struct PathSink {
    pb: tiny_skia::PathBuilder,
    x: f32,
    y: f32,
    s: f32,
}

impl ttf_parser::OutlineBuilder for PathSink {
    fn move_to(&mut self, x: f32, y: f32) {
        self.pb.move_to(self.x + x * self.s, self.y - y * self.s);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.pb.line_to(self.x + x * self.s, self.y - y * self.s);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.pb.quad_to(self.x + x1 * self.s, self.y - y1 * self.s, self.x + x * self.s, self.y - y * self.s);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.pb.cubic_to(
            self.x + x1 * self.s,
            self.y - y1 * self.s,
            self.x + x2 * self.s,
            self.y - y2 * self.s,
            self.x + x * self.s,
            self.y - y * self.s,
        );
    }
    fn close(&mut self) {
        self.pb.close();
    }
}

fn shape(face: &rustybuzz::Face, text: &str) -> rustybuzz::GlyphBuffer {
    let mut buf = rustybuzz::UnicodeBuffer::new();
    buf.push_str(text);
    buf.guess_segment_properties();
    rustybuzz::shape(face, &[], buf)
}

fn render_text(face: &rustybuzz::Face, text: &str, scale: f32, max_width: u32) -> Option<Rendered> {
    let mut glyphs = shape(face, text);
    // symbol fonts (cmap 3,0) map their characters to U+F0xx
    if glyphs.glyph_infos().iter().all(|g| g.glyph_id == 0) {
        let mapped: String = text.chars().map(|c| if (c as u32) < 0x100 { char::from_u32(0xF000 + c as u32).unwrap_or(c) } else { c }).collect();
        let alt = shape(face, &mapped);
        if alt.glyph_infos().iter().any(|g| g.glyph_id != 0) {
            glyphs = alt;
        }
    }
    if glyphs.is_empty() || glyphs.glyph_infos().iter().all(|g| g.glyph_id == 0) {
        return None;
    }
    let hhea_asc = face.ascender() as f32;
    let hhea_desc = face.descender() as f32;
    let ascent = (hhea_asc.max(face.units_per_em() as f32 * 0.6)) * scale;
    let descent = (-hhea_desc).max(0.0) * scale;
    let pad = 2.0;
    let baseline = (ascent + pad).ceil();
    let height = (baseline + descent + pad).ceil() as u32;
    let total: f32 = glyphs.glyph_positions().iter().map(|p| p.x_advance as f32 * scale).sum();
    let width = ((total + pad * 2.0).ceil() as u32).clamp(1, max_width.max(8));
    let mut pb = PathSink { pb: tiny_skia::PathBuilder::new(), x: pad, y: baseline, s: scale };
    let mut pen = pad;
    for (info, pos) in glyphs.glyph_infos().iter().zip(glyphs.glyph_positions()) {
        if pen > width as f32 {
            break;
        }
        pb.x = pen + pos.x_offset as f32 * scale;
        pb.y = baseline - pos.y_offset as f32 * scale;
        let _ = face.outline_glyph(ttf_parser::GlyphId(info.glyph_id as u16), &mut pb);
        pen += pos.x_advance as f32 * scale;
    }
    fill(pb.pb, width, height.max(1), baseline)
}

fn render_glyph(face: &rustybuzz::Face, gid: u16, scale: f32, cell: f32) -> Option<Rendered> {
    let size = cell.ceil().max(4.0) as u32;
    let id = ttf_parser::GlyphId(gid);
    let bbox = face.glyph_bounding_box(id);
    let adv = face.glyph_hor_advance(id).unwrap_or(0) as f32 * scale;
    // centre horizontally on the advance (or the box), baseline at 3/4 of the cell's font metrics
    let asc = face.ascender() as f32 * scale;
    let desc = -(face.descender() as f32) * scale;
    let line = asc + desc;
    let y = (size as f32 - line) / 2.0 + asc;
    let x = match bbox {
        Some(b) if adv <= 0.0 || (b.x_max as f32 * scale) > adv * 1.5 => {
            (size as f32 - (b.x_max - b.x_min) as f32 * scale) / 2.0 - b.x_min as f32 * scale
        }
        _ => (size as f32 - adv) / 2.0,
    };
    let mut pb = PathSink { pb: tiny_skia::PathBuilder::new(), x, y, s: scale };
    face.outline_glyph(id, &mut pb)?;
    fill(pb.pb, size, size, y)
}

fn fill(pb: tiny_skia::PathBuilder, width: u32, height: u32, baseline: f32) -> Option<Rendered> {
    let mut pixmap = tiny_skia::Pixmap::new(width, height)?;
    if let Some(path) = pb.finish() {
        let mut paint = tiny_skia::Paint::default();
        paint.set_color_rgba8(255, 255, 255, 255);
        paint.anti_alias = true;
        pixmap.fill_path(&path, &paint, tiny_skia::FillRule::Winding, tiny_skia::Transform::identity(), None);
    }
    let alpha: Vec<u8> = pixmap.data().as_chunks::<4>().0.iter().map(|p| p[3]).collect();
    Some(Rendered { width, height, alpha, baseline })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_a_system_font_when_available() {
        let p = Path::new(r"C:\Windows\Fonts\arial.ttf");
        let Ok(data) = std::fs::read(p) else { return };
        let req = Request {
            path: p.into(),
            face_index: 0,
            variations: vec![],
            content: Content::Text("Arial".into()),
            px: 36.0,
            max_width: 2000,
        };
        let r = render(&data, &req).unwrap();
        assert!(r.width > 60 && r.height > 30);
        assert!(r.alpha.iter().filter(|a| **a > 128).count() > 100);
        let g = render(&data, &Request { content: Content::Glyph(36), px: 40.0, ..req }).unwrap();
        assert_eq!((g.width, g.height), (40, 40));
        assert!(g.alpha.iter().any(|a| *a > 0));
    }

    #[test]
    fn keys_differ_by_content_and_size() {
        let a = Request { path: "a".into(), face_index: 0, variations: vec![], content: Content::Text("x".into()), px: 10.0, max_width: 100 };
        let mut b = a.clone();
        b.px = 11.0;
        assert_ne!(a.key(), b.key());
        let mut c = a.clone();
        c.variations.push((*b"wght", 300.0));
        assert_ne!(a.key(), c.key());
    }
}
