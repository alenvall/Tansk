//! The WebSocket to the server: bincode frames both ways. Incoming events
//! wait in a queue until the frame loop takes them.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use std::marker::PhantomData;

use js_sys::{ArrayBuffer, Uint8Array};
use serde::de::DeserializeOwned;
use serde::Serialize;
use tansk_sim::proto::{decode, encode};
use wasm_bindgen::prelude::*;
use web_sys::{BinaryType, CloseEvent, Event, MessageEvent, WebSocket};

pub enum NetEvent<S> {
    Open,
    Msg(S),
    Closed,
}

/// A socket sending `C` and receiving `S` (the 2D game's or the 3D game's messages).
pub struct Net<C, S> {
    ws: WebSocket,
    inbox: Rc<RefCell<VecDeque<NetEvent<S>>>>,
    _types: PhantomData<fn(C)>,
    _on_open: Closure<dyn FnMut(Event)>,
    _on_msg: Closure<dyn FnMut(MessageEvent)>,
    _on_close: Closure<dyn FnMut(CloseEvent)>,
}

impl<C: Serialize, S: DeserializeOwned + 'static> Net<C, S> {
    /// Connects to the WebSocket at `path` on this page's host (`/ws` or `/ws3d`).
    pub fn connect(path: &str) -> Result<Self, JsValue> {
        let loc = web_sys::window().unwrap().location();
        let scheme = if loc.protocol()? == "https:" { "wss" } else { "ws" };
        let url = format!("{scheme}://{}{path}", loc.host()?);
        let ws = WebSocket::new(&url)?;
        ws.set_binary_type(BinaryType::Arraybuffer);
        let inbox: Rc<RefCell<VecDeque<NetEvent<S>>>> = Rc::default();

        let q = inbox.clone();
        let on_open = Closure::<dyn FnMut(Event)>::new(move |_| q.borrow_mut().push_back(NetEvent::Open));
        let q = inbox.clone();
        let on_msg = Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
            if let Ok(buf) = e.data().dyn_into::<ArrayBuffer>() {
                let bytes = Uint8Array::new(&buf).to_vec();
                if let Some(msg) = decode::<S>(&bytes) {
                    q.borrow_mut().push_back(NetEvent::Msg(msg));
                }
            }
        });
        let q = inbox.clone();
        let on_close = Closure::<dyn FnMut(CloseEvent)>::new(move |_| q.borrow_mut().push_back(NetEvent::Closed));
        ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));
        ws.set_onmessage(Some(on_msg.as_ref().unchecked_ref()));
        ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));
        Ok(Net { ws, inbox, _types: PhantomData, _on_open: on_open, _on_msg: on_msg, _on_close: on_close })
    }

    pub fn send(&self, msg: &C) {
        if self.ws.ready_state() == WebSocket::OPEN {
            let _ = self.ws.send_with_u8_array(&encode(msg));
        }
    }

    pub fn take(&self) -> Vec<NetEvent<S>> {
        self.inbox.borrow_mut().drain(..).collect()
    }
}

impl<C, S> Drop for Net<C, S> {
    fn drop(&mut self) {
        self.ws.set_onopen(None);
        self.ws.set_onmessage(None);
        self.ws.set_onclose(None);
        let _ = self.ws.close();
    }
}
