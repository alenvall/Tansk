//! The socket under `NetClient`: a browser WebSocket on the web. Natively
//! there is no socket yet (online play is the browser build's).

pub enum Incoming {
    Open,
    Message(Vec<u8>),
    Closed(String),
}

pub trait Transport {
    /// Sends one binary frame (queued until the socket is open).
    fn send(&mut self, bytes: Vec<u8>);
    /// Everything that arrived since the last poll, in order.
    fn poll(&mut self, into: &mut Vec<Incoming>);
}

/// Opens the platform's socket to `url`.
pub fn open(url: &str) -> Box<dyn Transport> {
    #[cfg(target_arch = "wasm32")]
    {
        Box::new(web::WebTransport::open(url))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = url;
        Box::new(Offline { told: false })
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct Offline {
    told: bool,
}

#[cfg(not(target_arch = "wasm32"))]
impl Transport for Offline {
    fn send(&mut self, _: Vec<u8>) {}
    fn poll(&mut self, into: &mut Vec<Incoming>) {
        if !std::mem::replace(&mut self.told, true) {
            into.push(Incoming::Closed("Online play needs the browser build; try Practice.".into()));
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::cell::RefCell;
    use std::rc::Rc;

    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;
    use web_sys::{BinaryType, CloseEvent, MessageEvent, WebSocket};

    use super::{Incoming, Transport};

    type Inbox = Rc<RefCell<Vec<Incoming>>>;

    pub struct WebTransport {
        ws: Option<WebSocket>,
        inbox: Inbox,
        queued: Vec<Vec<u8>>,
        _onopen: Option<Closure<dyn FnMut()>>,
        _onmessage: Option<Closure<dyn FnMut(MessageEvent)>>,
        _onclose: Option<Closure<dyn FnMut(CloseEvent)>>,
    }

    impl WebTransport {
        pub fn open(url: &str) -> Self {
            let inbox: Inbox = Rc::default();
            let mut t = WebTransport { ws: None, inbox: inbox.clone(), queued: Vec::new(), _onopen: None, _onmessage: None, _onclose: None };
            let ws = match WebSocket::new(url) {
                Ok(ws) => ws,
                Err(e) => {
                    inbox.borrow_mut().push(Incoming::Closed(format!("Can't connect: {e:?}")));
                    return t;
                }
            };
            ws.set_binary_type(BinaryType::Arraybuffer);
            let i = inbox.clone();
            let onopen = Closure::<dyn FnMut()>::new(move || i.borrow_mut().push(Incoming::Open));
            let i = inbox.clone();
            let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
                if let Ok(buf) = e.data().dyn_into::<js_sys::ArrayBuffer>() {
                    i.borrow_mut().push(Incoming::Message(js_sys::Uint8Array::new(&buf).to_vec()));
                }
            });
            let i = inbox;
            let onclose = Closure::<dyn FnMut(CloseEvent)>::new(move |e: CloseEvent| {
                let why = if e.reason().is_empty() { format!("Connection lost ({}).", e.code()) } else { e.reason() };
                i.borrow_mut().push(Incoming::Closed(why));
            });
            ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
            ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
            ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
            t.ws = Some(ws);
            t._onopen = Some(onopen);
            t._onmessage = Some(onmessage);
            t._onclose = Some(onclose);
            t
        }
    }

    impl Transport for WebTransport {
        fn send(&mut self, bytes: Vec<u8>) {
            let Some(ws) = &self.ws else { return };
            match ws.ready_state() {
                WebSocket::OPEN => {
                    let _ = ws.send_with_u8_array(&bytes);
                }
                WebSocket::CONNECTING => self.queued.push(bytes),
                _ => {}
            }
        }

        fn poll(&mut self, into: &mut Vec<Incoming>) {
            into.append(&mut self.inbox.borrow_mut());
            if let Some(ws) = &self.ws {
                if ws.ready_state() == WebSocket::OPEN {
                    for b in self.queued.drain(..) {
                        let _ = ws.send_with_u8_array(&b);
                    }
                }
            }
        }
    }

    impl Drop for WebTransport {
        fn drop(&mut self) {
            if let Some(ws) = &self.ws {
                ws.set_onopen(None);
                ws.set_onmessage(None);
                ws.set_onclose(None);
                let _ = ws.close();
            }
        }
    }

    /// `ws(s)://<this page's host>/ws/arena`.
    pub fn default_url() -> String {
        let loc = web_sys::window().map(|w| w.location());
        let host = loc.as_ref().and_then(|l| l.host().ok()).unwrap_or_else(|| "localhost:8080".into());
        let secure = loc.and_then(|l| l.protocol().ok()).is_some_and(|p| p == "https:");
        format!("{}://{host}/ws/arena", if secure { "wss" } else { "ws" })
    }
}

/// The arena socket on the server this page came from (`ws://localhost:8080/ws/arena` natively).
pub fn default_url() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        web::default_url()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        "ws://localhost:8080/ws/arena".into()
    }
}
