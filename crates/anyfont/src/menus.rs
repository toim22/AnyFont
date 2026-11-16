//! The menu bar and AnyFont's own popup menus (menus, submenus, context menus), drawn like the reference
//! (UI-SPEC §2.1–§2.2): 1 px border, 2 px padding, 22 px items, 7 px separators, the 16 px icon column at
//! x 9, text at x 36, shortcuts right-aligned 24 px from the edge, `›` at the right edge − 12, the checked look
//! (icon on a 20×20 `accent.checked` box) and access keys underlined while Alt is held or the menu was opened
//! with the keyboard.

use crate::commands::Cmd;
use crate::icons::{self, Look};
use crate::theme::{self, t};
use crate::widgets;
use egui::{pos2, vec2, Align2, Context, Id, Key, Modifiers, Order, Pos2, Rect, Sense, Stroke, WidgetInfo, Role};

/// One entry of a popup menu.
#[derive(Clone, Debug)]
pub enum Item {
    Sep,
    Cmd { label: String, translate: bool, icon: Option<&'static str>, shortcut: Option<&'static str>, enabled: bool, checked: bool, cmd: Cmd },
    Sub { label: String, translate: bool, icon: Option<&'static str>, enabled: bool, items: Vec<Item> },
}

impl Item {
    /// A command with a UI caption (translated, `&` access key).
    pub fn cmd(label: &str, cmd: Cmd) -> Item {
        Item::Cmd { label: label.to_string(), translate: true, icon: None, shortcut: None, enabled: true, checked: false, cmd }
    }
    /// A command whose caption is data (a collection name, a path): never translated, no access key.
    pub fn data(label: &str, cmd: Cmd) -> Item {
        Item::Cmd { label: label.replace('&', "&&"), translate: false, icon: None, shortcut: None, enabled: true, checked: false, cmd }
    }
    pub fn sub(label: &str, items: Vec<Item>) -> Item {
        let enabled = !items.is_empty();
        Item::Sub { label: label.to_string(), translate: true, icon: None, enabled, items }
    }
    pub fn icon(mut self, name: &'static str) -> Item {
        match &mut self {
            Item::Cmd { icon, .. } | Item::Sub { icon, .. } => *icon = Some(name),
            Item::Sep => {}
        }
        self
    }
    pub fn shortcut(mut self, s: &'static str) -> Item {
        if let Item::Cmd { shortcut, .. } = &mut self {
            *shortcut = Some(s);
        }
        self
    }
    pub fn enabled(mut self, on: bool) -> Item {
        match &mut self {
            Item::Cmd { enabled, .. } | Item::Sub { enabled, .. } => *enabled = *enabled && on,
            Item::Sep => {}
        }
        self
    }
    pub fn checked(mut self, on: bool) -> Item {
        if let Item::Cmd { checked, .. } = &mut self {
            *checked = on;
        }
        self
    }
    fn is_enabled(&self) -> bool {
        match self {
            Item::Cmd { enabled, .. } | Item::Sub { enabled, .. } => *enabled,
            Item::Sep => false,
        }
    }
    fn height(&self) -> f32 {
        if matches!(self, Item::Sep) {
            SEP_H
        } else {
            ITEM_H
        }
    }
    fn plain_label(&self) -> String {
        match self {
            Item::Cmd { label, translate, .. } | Item::Sub { label, translate, .. } => {
                if *translate {
                    widgets::caption_parts(label).0
                } else {
                    label.replace("&&", "&")
                }
            }
            Item::Sep => String::new(),
        }
    }
}

const ITEM_H: f32 = 22.0;
const SEP_H: f32 = 7.0;
const PAD: f32 = 2.0;
const MIN_W: f32 = 196.0;

/// Which menu is open (the items are rebuilt from the current state every frame).
#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    /// A menu of the menu bar (index into File, Edit, Font, View, Help).
    Bar(usize),
    /// The font row context menu (= Font menu).
    Font,
    /// A Libraries row.
    Library(anyfont_core::LibraryRef),
    /// The edit menu of a text box.
    Edit(crate::commands::EditTarget),
}

#[derive(Clone, Debug)]
pub struct Open {
    pub source: Source,
    pub pos: Pos2,
    /// Open submenus: the item index at each level.
    pub path: Vec<usize>,
    /// Highlighted item per level.
    pub hot: Vec<Option<usize>>,
    /// Opened with the keyboard (access keys underlined, first item highlighted).
    pub keyboard: bool,
    pub opened_frame: u64,
}

#[derive(Default)]
pub struct MenuState {
    pub open: Option<Open>,
    /// F10 / Alt pressed alone: the menu bar has the keyboard focus (item index), no menu open.
    pub bar_focus: Option<usize>,
    alt_down_alone: bool,
}

impl MenuState {
    pub fn open(&mut self, source: Source, pos: Pos2, keyboard: bool, frame: u64) {
        if let Source::Edit(t) = source {
            crate::commands::snapshot_selection(t);
        }
        self.open = Some(Open { source, pos, path: Vec::new(), hot: vec![None], keyboard, opened_frame: frame });
        self.bar_focus = None;
    }
    pub fn close(&mut self) {
        self.open = None;
        self.bar_focus = None;
    }
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }
}

/// The captions of the menu bar.
pub const BAR: [&str; 5] = ["&File", "&Edit", "Font", "View", "Help"];

/// Lays out the menu bar boxes (first at x 4, text + 20 px wide, 3 px apart).
pub fn bar_boxes(ui: &egui::Ui, rect: Rect) -> Vec<Rect> {
    let font = theme::body(theme::UI_SIZE);
    let mut x = rect.left() + 4.0;
    BAR.iter()
        .map(|c| {
            let w = widgets::caption_width(ui, c, font.clone()).round() + 20.0;
            let r = Rect::from_min_max(pos2(x, rect.top() + 2.0), pos2(x + w, rect.bottom() - 2.0));
            x += w + 3.0;
            r
        })
        .collect()
}

/// Draws the menu bar and handles its mouse and keyboard input.
pub fn menu_bar(ui: &mut egui::Ui, rect: Rect, state: &mut MenuState, frame: u64) {
    let ctx = ui.ctx().clone();
    ui.painter().rect_filled(rect, 0.0, t().menu_bar_bg);
    let boxes = bar_boxes(ui, rect);
    let open_bar = match &state.open {
        Some(Open { source: Source::Bar(i), .. }) => Some(*i),
        _ => None,
    };
    let keyboard = state.open.as_ref().map(|o| o.keyboard).unwrap_or(false) || state.bar_focus.is_some();
    let alt = ui.input(|i| i.modifiers.alt);
    let underline = alt || keyboard;
    for (i, (b, cap)) in boxes.iter().zip(BAR.iter()).enumerate() {
        let resp = ui.interact(*b, Id::unique(("menubar", i)), Sense::click());
        let hot = open_bar == Some(i) || state.bar_focus == Some(i) || (resp.hovered() && open_bar.is_none());
        if hot {
            ui.painter().rect_filled(*b, 3.0, t().menu_bar_hot);
        }
        widgets::caption_text(ui, ui.painter(), b.center() - vec2(0.0, 1.0), Align2::CENTER_CENTER, cap, theme::body(theme::UI_SIZE), t().text, underline);
        widgets::a11y(&resp, Role::Button, true, cap);
        if resp.clicked() {
            if open_bar == Some(i) {
                state.close();
            } else {
                state.open(Source::Bar(i), pos2(b.left() - 1.0, rect.bottom()), false, frame);
            }
        } else if resp.hovered() && open_bar.is_some() && open_bar != Some(i) {
            let kb = state.open.as_ref().map(|o| o.keyboard).unwrap_or(false);
            state.open(Source::Bar(i), pos2(b.left() - 1.0, rect.bottom()), kb, frame);
        }
    }
    // keyboard: Alt+F, Alt+E, F10, Alt alone
    let (alt_f, alt_e, f10, alt_now, any_key) = ctx.input(|i| {
        (
            i.modifiers.alt && i.key_pressed(Key::F),
            i.modifiers.alt && i.key_pressed(Key::E),
            i.key_pressed(Key::F10) && !i.modifiers.shift,
            i.modifiers.alt,
            i.events.iter().any(|e| matches!(e, egui::Event::Key { pressed: true, .. } | egui::Event::Text(_) | egui::Event::PointerButton { .. })),
        )
    });
    let modal_open = ctx.memory(|m| m.top_modal_layer().is_some());
    if modal_open {
        return;
    }
    if alt_f || alt_e {
        let i = if alt_f { 0 } else { 1 };
        state.open(Source::Bar(i), pos2(boxes[i].left() - 1.0, rect.bottom()), true, frame);
        if let Some(o) = &mut state.open {
            o.hot = vec![first_enabled_hint()];
        }
        state.alt_down_alone = false;
        return;
    }
    if f10 {
        if state.bar_focus.is_some() || state.open.is_some() {
            state.close();
        } else {
            state.bar_focus = Some(0);
        }
        return;
    }
    // Alt pressed and released alone toggles the menu-bar focus
    if alt_now && !any_key {
        state.alt_down_alone = true;
    } else if alt_now && any_key {
        state.alt_down_alone = false;
    } else if !alt_now && state.alt_down_alone {
        state.alt_down_alone = false;
        if state.open.is_none() {
            state.bar_focus = if state.bar_focus.is_some() { None } else { Some(0) };
        }
    }
    if let Some(f) = state.bar_focus {
        let (left, right, down, enter, esc) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowLeft),
                i.consume_key(Modifiers::NONE, Key::ArrowRight),
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::Enter),
                i.consume_key(Modifiers::NONE, Key::Escape),
            )
        });
        if esc {
            state.bar_focus = None;
        } else if left {
            state.bar_focus = Some((f + BAR.len() - 1) % BAR.len());
        } else if right {
            state.bar_focus = Some((f + 1) % BAR.len());
        } else if down || enter {
            state.open(Source::Bar(f), pos2(boxes[f].left() - 1.0, rect.bottom()), true, frame);
            if let Some(o) = &mut state.open {
                o.hot = vec![first_enabled_hint()];
            }
        } else {
            // a letter opens the menu with that access key
            let typed: Option<String> = ctx.input(|i| {
                i.events.iter().find_map(|e| if let egui::Event::Text(t) = e { Some(t.clone()) } else { None })
            });
            if let Some(ch) = typed.and_then(|s| s.chars().next()) {
                if let Some(i) = BAR.iter().position(|c| access_char(c) == Some(ch.to_ascii_lowercase())) {
                    state.open(Source::Bar(i), pos2(boxes[i].left() - 1.0, rect.bottom()), true, frame);
                    if let Some(o) = &mut state.open {
                        o.hot = vec![first_enabled_hint()];
                    }
                }
            }
        }
    }
}

/// Marker meaning "highlight the first enabled item when the items are known".
fn first_enabled_hint() -> Option<usize> {
    Some(usize::MAX)
}

fn access_char(caption: &str) -> Option<char> {
    let (text, key) = widgets::caption_parts(caption);
    key.and_then(|k| text.chars().nth(k)).map(|c| c.to_lowercase().next().unwrap_or(c))
}

/// What the popup did this frame.
pub enum Outcome {
    None,
    Close,
    Run(Cmd),
    /// Left/right arrow on a top-level menu of the menu bar: switch to the neighbouring menu.
    Neighbour(i32),
}

fn level_items<'a>(items: &'a [Item], path: &[usize], level: usize) -> Option<&'a [Item]> {
    let mut cur = items;
    for &i in path.iter().take(level) {
        match cur.get(i) {
            Some(Item::Sub { items, .. }) => cur = items,
            _ => return None,
        }
    }
    Some(cur)
}

fn popup_width(ui: &egui::Ui, items: &[Item]) -> f32 {
    let font = theme::body(theme::UI_SIZE);
    let mut text_w: f32 = 0.0;
    let mut short_w: f32 = 0.0;
    for it in items {
        let label = it.plain_label();
        if !label.is_empty() {
            text_w = text_w.max(widgets::data_galley(ui, &label, font.clone(), t().text).size().x);
        }
        if let Item::Cmd { shortcut: Some(s), .. } = it {
            short_w = short_w.max(widgets::data_galley(ui, &crate::tr(s), font.clone(), t().text).size().x + 24.0);
        }
    }
    (text_w + short_w + 60.0).round().max(MIN_W)
}

/// Shows the open popup (all levels). `items` are the top-level items of `open.source`.
pub fn show(ctx: &Context, open: &mut Open, items: &[Item], frame: u64) -> Outcome {
    let mut outcome = Outcome::None;
    let screen = ctx.content_rect();
    let underline = open.keyboard || ctx.input(|i| i.modifiers.alt);
    // resolve the "first enabled" hint
    for (lvl, h) in open.hot.iter_mut().enumerate() {
        if *h == Some(usize::MAX) {
            *h = level_items(items, &open.path, lvl).and_then(|its| its.iter().position(|i| i.is_enabled()));
        }
    }
    let mut rects: Vec<Rect> = Vec::new();
    let mut pos = open.pos;
    let levels = open.path.len() + 1;
    let mut new_path: Option<Vec<usize>> = None;
    let mut parent_item_rect: Option<Rect> = None;
    for lvl in 0..levels {
        let Some(its) = level_items(items, &open.path, lvl) else { break };
        if its.is_empty() {
            break;
        }
        // geometry of this level
        let w = {
            let mut w = 0.0;
            egui::Area::new(Id::unique(("anyfont-menu-measure", lvl))).order(Order::Background).fixed_pos(pos2(-10000.0, -10000.0)).show(ctx, |ui| {
                w = popup_width(ui, its);
            });
            w
        };
        let h = its.iter().map(|i| i.height()).sum::<f32>() + 2.0 * PAD + 2.0;
        if let Some(pr) = parent_item_rect {
            let prev = rects[lvl - 1];
            pos = pos2(prev.right() - 3.0, pr.top() - PAD - 1.0);
            if pos.x + w > screen.right() {
                pos.x = (prev.left() - w + 3.0).max(screen.left());
            }
        }
        if pos.x + w > screen.right() {
            pos.x = (screen.right() - w).max(screen.left());
        }
        if pos.y + h > screen.bottom() {
            pos.y = (screen.bottom() - h).max(screen.top());
        }
        let rect = Rect::from_min_size(pos, vec2(w, h));
        rects.push(rect);
        if open.hot.len() <= lvl {
            open.hot.push(None);
        }
        let mut hot = open.hot[lvl];
        egui::Area::new(Id::unique(("anyfont-menu", lvl)))
            .order(Order::Foreground)
            .fixed_pos(rect.min)
            .constrain(false)
            .interactable(true)
            .show(ctx, |ui| {
                let (full, _) = ui.allocate_exact_size(rect.size(), Sense::hover());
                let p = ui.painter();
                p.rect_filled(full.translate(vec2(2.0, 3.0)), 2.0, egui::Color32::from_black_alpha(60));
                p.rect_filled(full, 0.0, t().menu_bg);
                p.rect_stroke(full, 0.0, Stroke::new(1.0, t().menu_border), egui::StrokeKind::Inside);
                let mut y = full.top() + 1.0 + PAD;
                for (i, it) in its.iter().enumerate() {
                    let ih = it.height();
                    let ir = Rect::from_min_max(pos2(full.left() + 1.0, y), pos2(full.right() - 1.0, y + ih));
                    y += ih;
                    if let Item::Sep = it {
                        let ly = ir.center().y.round() + 0.5;
                        ui.painter().hline(full.left() + 32.0..=full.right() - 2.0, ly, Stroke::new(1.0, t().menu_separator));
                        continue;
                    }
                    let enabled = it.is_enabled();
                    let resp = ui.interact(ir, Id::unique(("anyfont-menu-item", lvl, i)), if enabled { Sense::click() } else { Sense::hover() });
                    if resp.hovered() && ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO || i.pointer.any_pressed()) {
                        hot = Some(i);
                        let mut p2: Vec<usize> = open.path.iter().take(lvl).copied().collect();
                        if let Item::Sub { enabled: true, .. } = it {
                            p2.push(i);
                        }
                        new_path = Some(p2);
                    }
                    let is_hot = hot == Some(i) && enabled;
                    let is_open_sub = open.path.get(lvl) == Some(&i);
                    if is_hot || is_open_sub {
                        ui.painter().rect_filled(ir.shrink2(vec2(1.0, 0.0)), 2.0, t().menu_hot);
                    }
                    let color = if enabled { t().text } else { t().text_disabled };
                    let (label, translate, icon, checked) = match it {
                        Item::Cmd { label, translate, icon, checked, .. } => (label, *translate, *icon, *checked),
                        Item::Sub { label, translate, icon, .. } => (label, *translate, *icon, false),
                        Item::Sep => unreachable!(),
                    };
                    let icon_c = pos2(full.left() + 9.0 + 8.0, ir.center().y);
                    if checked {
                        ui.painter().rect_filled(Rect::from_center_size(icon_c, vec2(20.0, 20.0)), 3.0, t().accent_checked);
                    }
                    if let Some(name) = icon {
                        icons::paint(ui, name, Rect::from_center_size(icon_c, vec2(16.0, 16.0)), 16.0, if enabled { Look::Normal } else { Look::Disabled });
                    } else if checked {
                        // a check item without an icon: a ✓ in the checked box
                        let pts = vec![icon_c + vec2(-5.0, 0.0), icon_c + vec2(-1.5, 3.5), icon_c + vec2(5.0, -3.5)];
                        ui.painter().add(egui::Shape::line(pts, Stroke::new(1.6, t().text_strong)));
                    }
                    let tp = pos2(full.left() + 36.0, ir.center().y);
                    if translate {
                        widgets::caption_text(ui, ui.painter(), tp, Align2::LEFT_CENTER, label, theme::body(theme::UI_SIZE), color, underline);
                    } else {
                        widgets::data_text(ui, ui.painter(), tp, Align2::LEFT_CENTER, &label.replace("&&", "&"), theme::body(theme::UI_SIZE), color);
                    }
                    match it {
                        Item::Cmd { shortcut: Some(s), .. } => {
                            ui.painter().text(pos2(full.right() - 24.0, ir.center().y), Align2::RIGHT_CENTER, *s, theme::body(theme::UI_SIZE), color);
                        }
                        Item::Sub { .. } => {
                            icons::paint(ui, "chevron-right", Rect::from_center_size(pos2(full.right() - 12.0, ir.center().y), vec2(12.0, 12.0)), 12.0, if enabled { Look::Normal } else { Look::Disabled });
                        }
                        _ => {}
                    }
                    let plain = it.plain_label();
                    let (kind, chk) = match it {
                        Item::Cmd { checked, .. } => (if *checked { Role::MenuItemCheckBox } else { Role::MenuItem }, *checked),
                        _ => (Role::MenuItem, false),
                    };
                    resp.widget_info(|| {
                        if kind == Role::MenuItemCheckBox {
                            WidgetInfo::selected(kind, enabled, chk, plain.clone())
                        } else {
                            WidgetInfo::labeled(kind, enabled, plain.clone())
                        }
                    });
                    if resp.clicked() && enabled {
                        match it {
                            Item::Cmd { cmd, .. } => outcome = Outcome::Run(cmd.clone()),
                            Item::Sub { .. } => {
                                let mut p2: Vec<usize> = open.path.iter().take(lvl).copied().collect();
                                p2.push(i);
                                new_path = Some(p2);
                            }
                            Item::Sep => {}
                        }
                    }
                }
            });
        open.hot[lvl] = hot;
        if lvl + 1 < levels {
            // the parent item of the next level: find its rect
            let idx = open.path[lvl];
            let mut y = rect.top() + 1.0 + PAD;
            for it in its.iter().take(idx) {
                y += it.height();
            }
            parent_item_rect = Some(Rect::from_min_size(pos2(rect.left(), y), vec2(rect.width(), ITEM_H)));
        }
    }
    if let Some(p) = new_path {
        if p != open.path {
            open.path = p;
            open.hot.truncate(open.path.len() + 1);
            while open.hot.len() < open.path.len() + 1 {
                open.hot.push(None);
            }
        }
    }
    if matches!(outcome, Outcome::Run(_)) {
        return outcome;
    }
    // keyboard navigation
    let (up, down, left, right, enter, esc) = ctx.input_mut(|i| {
        (
            i.consume_key(Modifiers::NONE, Key::ArrowUp),
            i.consume_key(Modifiers::NONE, Key::ArrowDown),
            i.consume_key(Modifiers::NONE, Key::ArrowLeft),
            i.consume_key(Modifiers::NONE, Key::ArrowRight),
            i.consume_key(Modifiers::NONE, Key::Enter) || i.consume_key(Modifiers::NONE, Key::Space),
            i.consume_key(Modifiers::NONE, Key::Escape),
        )
    });
    let lvl = open.path.len();
    let cur_items = level_items(items, &open.path, lvl).unwrap_or(&[]);
    if up || down {
        open.keyboard = true;
        let n = cur_items.len();
        if n > 0 {
            let mut i = open.hot[lvl].unwrap_or(if down { n - 1 } else { 0 });
            for _ in 0..n {
                i = if down { (i + 1) % n } else { (i + n - 1) % n };
                if cur_items[i].is_enabled() {
                    break;
                }
            }
            open.hot[lvl] = Some(i);
        }
    }
    if right {
        open.keyboard = true;
        if let Some(i) = open.hot[lvl] {
            if let Some(Item::Sub { enabled: true, items: sub, .. }) = cur_items.get(i) {
                open.path.push(i);
                open.hot.push(sub.iter().position(|x| x.is_enabled()));
                return Outcome::None;
            }
        }
        if matches!(open.source, Source::Bar(_)) {
            return Outcome::Neighbour(1);
        }
    }
    if left {
        open.keyboard = true;
        if lvl > 0 {
            open.path.pop();
            open.hot.truncate(lvl);
            return Outcome::None;
        }
        if matches!(open.source, Source::Bar(_)) {
            return Outcome::Neighbour(-1);
        }
    }
    if enter {
        if let Some(i) = open.hot[lvl] {
            match cur_items.get(i) {
                Some(Item::Cmd { enabled: true, cmd, .. }) => return Outcome::Run(cmd.clone()),
                Some(Item::Sub { enabled: true, items: sub, .. }) => {
                    open.path.push(i);
                    open.hot.push(sub.iter().position(|x| x.is_enabled()));
                }
                _ => {}
            }
        }
    }
    if esc {
        if lvl > 0 {
            open.path.pop();
            open.hot.truncate(lvl);
        } else {
            return Outcome::Close;
        }
    }
    // access keys inside the open level
    if open.keyboard || ctx.input(|i| i.modifiers.alt) {
        let typed: Option<char> = ctx.input(|i| {
            i.events.iter().find_map(|e| if let egui::Event::Text(t) = e { t.chars().next() } else { None })
        });
        if let Some(ch) = typed {
            let ch = ch.to_lowercase().next().unwrap_or(ch);
            let hits: Vec<usize> = cur_items
                .iter()
                .enumerate()
                .filter(|(_, it)| it.is_enabled())
                .filter(|(_, it)| match it {
                    Item::Cmd { label, translate: true, .. } | Item::Sub { label, translate: true, .. } => access_char(label) == Some(ch),
                    _ => false,
                })
                .map(|(i, _)| i)
                .collect();
            if hits.len() == 1 {
                match &cur_items[hits[0]] {
                    Item::Cmd { cmd, .. } => return Outcome::Run(cmd.clone()),
                    Item::Sub { items: sub, .. } => {
                        open.path.push(hits[0]);
                        open.hot.push(sub.iter().position(|x| x.is_enabled()));
                    }
                    Item::Sep => {}
                }
            }
        }
    }
    // a click outside every level closes the menu
    let pressed = ctx.input(|i| i.pointer.any_pressed());
    if pressed && frame > open.opened_frame {
        let p = ctx.input(|i| i.pointer.interact_pos());
        if let Some(p) = p {
            if !rects.iter().any(|r| r.contains(p)) {
                return Outcome::Close;
            }
        }
    }
    outcome
}
