//! The web-sys DOM backend: inserts the partials, applies attribute/text
//! writes, and turns clicks and keys into [`UiAction`]s.
//!
//! Only the web-sys features the crate already enables are used (Window,
//! Document, Element, HtmlElement, CssStyleDeclaration); event fields and
//! form values are read with `js_sys::Reflect`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Document, Element, EventTarget, HtmlElement};

use super::{Chassis, Garage, HostConfig, Quality, Settings, UiAction, CONTROLS_HTML, CSS, HUD_HTML, MENUS_HTML};

#[derive(Default)]
struct Shared {
    queue: Vec<UiAction>,
    /// Cards on offer; 0 when no pick is open (or it was already made).
    branch_count: usize,
    hud_visible: bool,
    chat_open: bool,
}

type Listener = Closure<dyn FnMut(JsValue)>;
type Handler = fn(&Ctx, JsValue);

pub(super) struct Dom {
    doc: Document,
    root: HtmlElement,
    els: HashMap<String, Element>,
    shared: Rc<RefCell<Shared>>,
    listeners: Vec<(EventTarget, &'static str, Listener)>,
}

fn prop(v: &JsValue, name: &str) -> JsValue {
    js_sys::Reflect::get(v, &JsValue::from_str(name)).unwrap_or(JsValue::UNDEFINED)
}

fn prop_str(v: &JsValue, name: &str) -> String {
    prop(v, name).as_string().unwrap_or_default()
}

fn set_prop(v: &JsValue, name: &str, value: &JsValue) {
    let _ = js_sys::Reflect::set(v, &JsValue::from_str(name), value);
}

fn call0(v: &JsValue, method: &str) {
    if let Ok(f) = prop(v, method).dyn_into::<js_sys::Function>() {
        let _ = f.call0(v);
    }
}

fn target(ev: &JsValue) -> Option<Element> {
    prop(ev, "target").dyn_into::<Element>().ok()
}

/// Everything the closures need, cloneable into each of them.
#[derive(Clone)]
struct Ctx {
    doc: Document,
    root: Element,
    shared: Rc<RefCell<Shared>>,
}

impl Ctx {
    fn q(&self, sel: &str) -> Option<Element> {
        self.root.query_selector(sel).ok().flatten()
    }
    fn value(&self, sel: &str) -> String {
        self.q(sel).map(|e| prop_str(&e, "value")).unwrap_or_default()
    }
    fn checked(&self, sel: &str) -> bool {
        self.q(sel).is_some_and(|e| prop(&e, "checked").is_truthy())
    }
    fn push(&self, a: UiAction) {
        self.shared.borrow_mut().queue.push(a);
    }

    fn read_settings(&self) -> Settings {
        let pct = |k: &str| self.value(&format!("input[data-setting={k}]")).parse::<f32>().unwrap_or(100.0) / 100.0;
        let name = self.value("#t-set-name").trim().to_string();
        Settings {
            name: if name.is_empty() { Settings::default().name } else { name },
            vol_master: pct("vol_master"),
            vol_sfx: pct("vol_sfx"),
            vol_music: pct("vol_music"),
            vol_radio: pct("vol_radio"),
            vol_ambience: pct("vol_ambience"),
            mouse_sens: pct("mouse_sens"),
            quality: Quality::parse(&self.value("input[name=quality]:checked")),
        }
    }

    fn read_garage(&self) -> Garage {
        let chassis = match self.value("input[name=chassis]:checked").as_str() {
            "scout" => Chassis::Scout,
            "hauler" => Chassis::Hauler,
            _ => Chassis::Brawler,
        };
        let mut paint = self.value("input[name=paint]:checked");
        if paint.is_empty() {
            paint = self.value("#t-paint-custom");
        }
        let emblem = self.value("input[name=emblem]:checked");
        Garage { chassis, paint, emblem: if emblem.is_empty() { "wrench".into() } else { emblem } }
    }

    fn read_host(&self) -> HostConfig {
        HostConfig {
            mode: self.value("input[name=mode]:checked"),
            map: self.value("input[name=map]:checked"),
            bots: self.checked("#t-host-bots"),
            private: self.checked("#t-host-private"),
            code: self.q("#t-host-code").and_then(|e| e.text_content()).unwrap_or_default(),
        }
    }

    fn join_code(&self) {
        let code = self.value("#t-join-code").trim().to_uppercase();
        if !code.is_empty() {
            self.push(UiAction::Join { code });
        }
    }

    fn pick_branch(&self, i: usize) {
        {
            let mut s = self.shared.borrow_mut();
            if i >= s.branch_count {
                return;
            }
            s.branch_count = 0;
            s.queue.push(UiAction::BranchPicked(i as u8));
        }
        if let Some(card) = self.q(&format!(".t-branch-card[data-i=\"{i}\"]")) {
            let _ = card.set_attribute("data-picked", "1");
        }
        if let Some(b) = self.q("#t-branch") {
            let _ = b.set_attribute("data-picked", &i.to_string());
        }
    }

    fn set_chat(&self, open: bool) {
        self.shared.borrow_mut().chat_open = open;
        if let Some(c) = self.q("#t-chat") {
            let _ = c.set_attribute("data-open", if open { "1" } else { "0" });
        }
        if let Some(input) = self.q("#t-chat-input") {
            if open {
                call0(&input, "focus");
            } else {
                set_prop(&input, "value", &JsValue::from_str(""));
                call0(&input, "blur");
            }
        }
    }

    fn on_click(&self, ev: JsValue) {
        let Some(t) = target(&ev) else { return };
        if let Ok(Some(g)) = t.closest("[data-goto]") {
            if let (Some(panel), Some(title)) = (g.get_attribute("data-goto"), self.doc.get_element_by_id("t-title")) {
                let _ = title.set_attribute("data-panel", &panel);
            }
            return;
        }
        let Ok(Some(el)) = t.closest("[data-action]") else { return };
        let action = el.get_attribute("data-action").unwrap_or_default();
        let a = match action.as_str() {
            "play" => UiAction::Play,
            "practice" => UiAction::Practice,
            "host" => UiAction::Host(self.read_host()),
            "join" => return self.join_code(),
            "join-row" => UiAction::Join { code: el.get_attribute("data-code").unwrap_or_default() },
            "refresh-games" => UiAction::RefreshGames,
            "cancel" => UiAction::Cancel,
            "settings-apply" => UiAction::SettingsChanged(self.read_settings()),
            "garage-save" => UiAction::GarageChanged(self.read_garage()),
            "branch" => {
                let i = el.get_attribute("data-i").and_then(|s| s.parse().ok()).unwrap_or(usize::MAX);
                return self.pick_branch(i);
            }
            "again" => UiAction::Again,
            "leave" => UiAction::Leave,
            _ => return,
        };
        self.push(a);
    }

    fn on_input(&self, ev: JsValue) {
        let Some(t) = target(&ev) else { return };
        if prop_str(&t, "type") == "range" {
            let v = prop_str(&t, "value");
            let shown = if t.get_attribute("data-setting").as_deref() == Some("mouse_sens") {
                format!("{:.2}", v.parse::<f32>().unwrap_or(100.0) / 100.0)
            } else {
                v
            };
            if let Some(out) = t.parent_element().and_then(|p| p.query_selector("output").ok().flatten()) {
                out.set_text_content(Some(&shown));
            }
        }
        if t.id() == "t-paint-custom" {
            if let Some(r) = self.q("input[name=paint]:checked") {
                set_prop(&r, "checked", &JsValue::FALSE);
            }
        }
    }

    fn on_change(&self, ev: JsValue) {
        let Some(t) = target(&ev) else { return };
        if t.has_attribute("data-setting") {
            self.push(UiAction::SettingsChanged(self.read_settings()));
        }
        if t.get_attribute("name").as_deref() == Some("pmap") {
            self.push(UiAction::PracticeMap(prop_str(&t, "value")));
        }
    }

    fn on_key(&self, ev: JsValue) {
        let key = prop_str(&ev, "key");
        let (chat_open, hud, branch) = {
            let s = self.shared.borrow();
            (s.chat_open, s.hud_visible, s.branch_count)
        };
        if chat_open {
            match key.as_str() {
                "Enter" => {
                    let text = self.value("#t-chat-input").trim().to_string();
                    if !text.is_empty() {
                        self.push(UiAction::Chat(text));
                    }
                    self.set_chat(false);
                    call0(&ev, "preventDefault");
                }
                "Escape" => self.set_chat(false),
                _ => {}
            }
            return;
        }
        let in_input = target(&ev).is_some_and(|t| t.tag_name() == "INPUT");
        if in_input {
            if key == "Enter" && target(&ev).is_some_and(|t| t.id() == "t-join-code") {
                self.join_code();
            }
            return;
        }
        if hud {
            if branch > 0 {
                if let Some(i) = ["1", "2", "3"].iter().position(|k| *k == key) {
                    self.pick_branch(i);
                    return;
                }
            }
            if key == "Enter" {
                self.set_chat(true);
                call0(&ev, "preventDefault");
            }
        } else if key == "Escape" {
            if let Some(title) = self.doc.get_element_by_id("t-title") {
                let _ = title.set_attribute("data-panel", "none");
            }
        }
    }
}

impl Dom {
    pub fn mount() -> Self {
        let window = web_sys::window().expect("no window");
        let doc = window.document().expect("no document");
        let body = doc.body().expect("no body");

        if doc.get_element_by_id("tansk-ui-css").is_none() {
            let style = doc.create_element("style").expect("create style");
            style.set_id("tansk-ui-css");
            style.set_text_content(Some(CSS));
            let parent: Element = doc.query_selector("head").ok().flatten().unwrap_or_else(|| body.clone().into());
            let _ = parent.append_child(&style);
        }
        let root: HtmlElement = match doc.get_element_by_id("tansk-ui") {
            Some(e) => e.dyn_into().expect("#tansk-ui is not an HTML element"),
            None => {
                let e: HtmlElement = doc.create_element("div").expect("create div").dyn_into().expect("div");
                e.set_id("tansk-ui");
                let _ = body.append_child(&e);
                e
            }
        };
        if root.child_element_count() == 0 {
            root.set_inner_html(&format!("{}{HUD_HTML}", MENUS_HTML.replace("<!--CONTROLS-->", CONTROLS_HTML)));
        }

        let shared = Rc::new(RefCell::new(Shared::default()));
        let ctx = Ctx { doc: doc.clone(), root: root.clone().into(), shared: shared.clone() };
        let mut dom = Dom { doc, root: root.clone(), els: HashMap::new(), shared, listeners: Vec::new() };

        let root_t: EventTarget = root.into();
        let win_t: EventTarget = window.into();
        let handlers: [(&EventTarget, &'static str, Handler); 4] = [
            (&root_t, "click", Ctx::on_click),
            (&root_t, "input", Ctx::on_input),
            (&root_t, "change", Ctx::on_change),
            (&win_t, "keydown", Ctx::on_key),
        ];
        for (t, kind, f) in handlers {
            let c = ctx.clone();
            let cl: Listener = Closure::wrap(Box::new(move |ev: JsValue| f(&c, ev)));
            let _ = t.add_event_listener_with_callback(kind, cl.as_ref().unchecked_ref());
            dom.listeners.push((t.clone(), kind, cl));
        }
        dom
    }

    fn el(&mut self, id: &str) -> Option<Element> {
        if let Some(e) = self.els.get(id) {
            return Some(e.clone());
        }
        let e = self.doc.get_element_by_id(id)?;
        self.els.insert(id.to_string(), e.clone());
        Some(e)
    }

    pub fn drain(&mut self) -> Vec<UiAction> {
        std::mem::take(&mut self.shared.borrow_mut().queue)
    }
    pub fn text(&mut self, id: &str, text: &str) {
        if let Some(e) = self.el(id) {
            e.set_text_content(Some(text));
        }
    }
    pub fn html(&mut self, id: &str, html: &str) {
        if let Some(e) = self.el(id) {
            e.set_inner_html(html);
        }
    }
    pub fn append_html(&mut self, id: &str, html: &str) {
        if let Some(e) = self.el(id) {
            let _ = e.insert_adjacent_html("beforeend", html);
        }
    }
    pub fn remove_first_child(&mut self, id: &str) {
        if let Some(c) = self.el(id).and_then(|e| e.first_element_child()) {
            c.remove();
        }
    }
    pub fn attr(&mut self, id: &str, name: &str, value: &str) {
        if let Some(e) = self.el(id) {
            let _ = e.set_attribute(name, value);
        }
    }
    pub fn remove_attr(&mut self, id: &str, name: &str) {
        if let Some(e) = self.el(id) {
            let _ = e.remove_attribute(name);
        }
    }
    pub fn var(&mut self, id: &str, name: &str, value: &str) {
        if let Some(e) = self.el(id).and_then(|e| e.dyn_into::<HtmlElement>().ok()) {
            let _ = e.style().set_property(name, value);
        }
    }
    pub fn hidden(&mut self, id: &str, hidden: bool) {
        if let Some(e) = self.el(id) {
            let _ = if hidden { e.set_attribute("hidden", "") } else { e.remove_attribute("hidden") };
        }
    }
    /// Remove, force a style flush, set again: restarts a CSS animation.
    pub fn retrigger(&mut self, id: &str, name: &str, value: &str) {
        if let Some(e) = self.el(id) {
            let _ = e.remove_attribute(name);
            let _ = self.root.offset_width();
            let _ = e.set_attribute(name, value);
        }
    }
    pub fn set_value(&mut self, id: &str, value: &str) {
        if let Some(e) = self.el(id) {
            set_prop(&e, "value", &JsValue::from_str(value));
        }
    }
    pub fn set_range(&mut self, setting: &str, value: &str, shown: &str) {
        let Some(input) = self.root.query_selector(&format!("input[data-setting={setting}]")).ok().flatten() else {
            return;
        };
        set_prop(&input, "value", &JsValue::from_str(value));
        if let Some(out) = input.parent_element().and_then(|p| p.query_selector("output").ok().flatten()) {
            out.set_text_content(Some(shown));
        }
    }
    pub fn check(&mut self, selector: &str) {
        if let Some(e) = self.root.query_selector(selector).ok().flatten() {
            set_prop(&e, "checked", &JsValue::TRUE);
        }
    }
    pub fn set_hud_visible(&mut self, visible: bool) {
        self.shared.borrow_mut().hud_visible = visible;
    }
    pub fn set_branch(&mut self, count: usize) {
        self.shared.borrow_mut().branch_count = count;
    }
    pub fn chat_open(&self) -> bool {
        self.shared.borrow().chat_open
    }
}

impl Drop for Dom {
    fn drop(&mut self) {
        for (t, kind, cl) in &self.listeners {
            let _ = t.remove_event_listener_with_callback(kind, cl.as_ref().unchecked_ref());
        }
    }
}
