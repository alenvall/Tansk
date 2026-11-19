//! Native builds have no DOM: every operation is a no-op, so the game can
//! drive the UI the same way on both targets.

use super::UiAction;

pub(super) struct Dom;

impl Dom {
    pub fn mount() -> Self {
        Dom
    }
    pub fn drain(&mut self) -> Vec<UiAction> {
        Vec::new()
    }
    pub fn text(&mut self, _id: &str, _text: &str) {}
    pub fn html(&mut self, _id: &str, _html: &str) {}
    pub fn append_html(&mut self, _id: &str, _html: &str) {}
    pub fn remove_first_child(&mut self, _id: &str) {}
    pub fn attr(&mut self, _id: &str, _name: &str, _value: &str) {}
    pub fn remove_attr(&mut self, _id: &str, _name: &str) {}
    pub fn var(&mut self, _id: &str, _name: &str, _value: &str) {}
    pub fn hidden(&mut self, _id: &str, _hidden: bool) {}
    pub fn retrigger(&mut self, _id: &str, _name: &str, _value: &str) {}
    pub fn flush(&mut self) {}
    pub fn set_value(&mut self, _id: &str, _value: &str) {}
    pub fn set_range(&mut self, _setting: &str, _value: &str, _shown: &str) {}
    pub fn check(&mut self, _selector: &str) {}
    pub fn set_checked(&mut self, _selector: &str, _on: bool) {}
    pub fn set_hud_visible(&mut self, _visible: bool) {}
    pub fn set_branch(&mut self, _count: usize) {}
    pub fn chat_open(&self) -> bool {
        false
    }
}
