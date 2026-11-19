//! Headless netcode checks: a scripted drive against an in-memory server
//! with simulated latency.

use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;

use arena_core::net::{decode, encode, ClientMsg, Join, ServerMsg};
use arena_core::{Input, Map, Sim, Tuning, Vec3, DT};

use super::transport::{Incoming, Transport};
use super::{NetClient, Status};

/// Both directions of a link, each delayed by `one_way` seconds.
#[derive(Default)]
struct Link {
    now: f64,
    one_way: f64,
    to_server: VecDeque<(f64, Vec<u8>)>,
    to_client: VecDeque<(f64, Incoming)>,
}

struct LinkEnd(Rc<RefCell<Link>>);

impl Transport for LinkEnd {
    fn send(&mut self, bytes: Vec<u8>) {
        let mut l = self.0.borrow_mut();
        let at = l.now + l.one_way;
        l.to_server.push_back((at, bytes));
    }
    fn poll(&mut self, into: &mut Vec<Incoming>) {
        let mut l = self.0.borrow_mut();
        while l.to_client.front().is_some_and(|(t, _)| *t <= l.now) {
            into.push(l.to_client.pop_front().unwrap().1);
        }
    }
}

/// The server's room logic in miniature: one input per tick, snapshots
/// every other tick, acks.
struct FakeServer {
    sim: Sim,
    me: Option<u8>,
    queue: VecDeque<(u32, Input)>,
    input: Input,
    ack: u32,
    /// Server pose after applying input `seq`.
    poses: BTreeMap<u32, Vec3>,
}

impl FakeServer {
    fn send(link: &mut Link, msg: &ServerMsg) {
        let at = link.now + link.one_way;
        link.to_client.push_back((at, Incoming::Message(encode(msg))));
    }

    fn tick(&mut self, link: &mut Link) {
        while link.to_server.front().is_some_and(|(t, _)| *t <= link.now) {
            let bytes = link.to_server.pop_front().unwrap().1;
            match decode::<ClientMsg>(&bytes).unwrap() {
                ClientMsg::Hello { .. } => {
                    let you = self.sim.add_player("Crew");
                    self.me = Some(you);
                    Self::send(link, &ServerMsg::Welcome { code: "TEST".into(), public: false, you, seed: 5, token: 1 });
                }
                ClientMsg::Input { seq, input } => self.queue.push_back((seq, input)),
                ClientMsg::Chat(_) => {}
            }
        }
        let Some(me) = self.me else { return };
        let fresh = self.queue.pop_front();
        if let Some((seq, input)) = fresh {
            self.ack = seq;
            self.input = input;
        }
        self.sim.set_input(me, self.input);
        self.sim.step();
        let events = self.sim.drain_events();
        if fresh.is_some() {
            self.poses.insert(self.ack, self.sim.tank_pose(me).unwrap().0);
        }
        if self.sim.tick.is_multiple_of(2) {
            Self::send(link, &ServerMsg::Tick { ack: self.ack, snapshot: self.sim.snapshot(), events });
        }
    }
}

fn script(t: f64) -> Input {
    Input {
        throttle: if t < 7.0 { 1.0 } else { -0.6 },
        steer: (t * 0.9).sin() as f32 * 0.8,
        boost: (3.0..4.0).contains(&t),
        ..Input::default()
    }
}

/// 100 ms round trip (50 ms each way), frames at 75 Hz against 60 Hz ticks:
/// the predicted pose for each input stays within 0.2 m of the server's.
#[test]
fn prediction_converges() {
    let link = Rc::new(RefCell::new(Link { one_way: 0.05, ..Link::default() }));
    link.borrow_mut().to_client.push_back((0.0, Incoming::Open));
    let l2 = link.clone();
    let mut client = NetClient::with_transport("test", "Crew", Join::Play, Box::new(move |_: &str| Box::new(LinkEnd(l2.clone()))));
    let mut server = FakeServer {
        sim: Sim::new(5, Map::the_sink(5), Tuning::default()),
        me: None,
        queue: VecDeque::new(),
        input: Input::default(),
        ack: 0,
        poses: BTreeMap::new(),
    };

    let mut predicted: BTreeMap<u32, Vec3> = BTreeMap::new();
    let mut server_clock = 0.0;
    let frame = 1.0 / 75.0;
    let mut t = 0.0;
    while t < 10.0 {
        t += frame;
        link.borrow_mut().now = t;
        while server_clock + DT as f64 <= t {
            server_clock += DT as f64;
            server.tick(&mut link.borrow_mut());
        }
        client.send_input(script(t));
        client.update(t);
        if let super::Mode::Online(o) = &client.mode {
            if let Some(p) = &o.predictor {
                predicted.entry(o.seq).or_insert(p.predicted_pose().0);
            }
        }
    }
    assert_eq!(client.state().status, Status::Playing);
    assert!(client.state().world.is_some());
    let rtt = client.state().rtt_ms;
    assert!((90.0..160.0).contains(&rtt), "rtt {rtt}");

    let mut worst = 0.0f32;
    let mut n = 0;
    for (seq, p) in &predicted {
        // The first second: the tank settles onto its suspension.
        if *seq < 60 {
            continue;
        }
        if let Some(s) = server.poses.get(seq) {
            worst = worst.max(p.distance(*s));
            n += 1;
        }
    }
    let travelled = server.poses.values().next().unwrap().distance(*server.poses.values().last().unwrap());
    eprintln!("compared {n} inputs, worst error {worst:.3} m, drove {travelled:.1} m");
    assert!(n > 300, "only {n} inputs compared");
    assert!(travelled > 20.0, "the tank barely moved ({travelled} m)");
    assert!(worst < 0.2, "prediction error {worst} m");
}

/// Practice produces the same kind of state, with no network.
#[test]
fn practice_runs() {
    let mut c = NetClient::practice(3, 3, 0.5);
    let mut t = 0.0;
    for _ in 0..120 {
        t += 1.0 / 60.0;
        c.send_input(Input { throttle: 1.0, ..Input::default() });
        c.update(t);
    }
    let s = c.state();
    assert_eq!(s.status, Status::Practice);
    assert_eq!(s.world.as_ref().unwrap().tanks.len(), 4);
    assert!(s.my_tank().is_some());
    assert_eq!(s.players.len(), 4);
}
