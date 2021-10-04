use autocan::{Message, MessageBuffer};
use pm1_control_model::{model::ChassisModel, optimizer::Optimizer, Physical};
use serial_port::{Port, SerialPort};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub mod autocan;

pub struct PM1 {
    port: Arc<Port>,
    buffer: MessageBuffer<32>,

    target: Arc<Mutex<(Instant, Physical)>>,

    control_period: Duration,
    model: ChassisModel,
    optimizer: Optimizer,
}

pub struct PM1QuerySender {
    port: Arc<Port>,

    next: Instant,
    index: usize,
}

pub enum PM1Status {
    Battery(u8),
    Status(Physical),
    Odometry(f32, nalgebra::Isometry2<f32>),
}

pub fn pm1(port: Port) -> (PM1QuerySender, PM1) {
    let control_period = Duration::from_millis(40);
    let now = Instant::now();
    let port = Arc::new(port);
    (
        PM1QuerySender {
            port: port.clone(),
            next: now,
            index: 0,
        },
        PM1 {
            port,
            buffer: Default::default(),
            target: Arc::new(Mutex::new((now, Physical::RELEASED))),
            control_period,
            model: Default::default(),
            optimizer: Optimizer::new(0.5, 1.2, control_period),
        },
    )
}

struct Queries([u8; 24]);

impl Queries {
    pub fn new() -> Self {
        let mut buffer = [0u8; 24];
        unsafe {
            use node::*;

            let buffer = std::slice::from_raw_parts_mut(buffer.as_mut_ptr() as *mut Message, 4);
            buffer[0] = message(tcu::TYPE, EVERY_INDEX, tcu::CURRENT_POSITION, false);
            buffer[1] = message(ecu::TYPE, EVERY_INDEX, ecu::CURRENT_POSITION, false);
            buffer[2] = message(EVERY_TYPE, EVERY_INDEX, STATE, false);
            buffer[3] = message(vcu::TYPE, EVERY_INDEX, vcu::BATTERY_PERSENT, false);
        }
        Self(buffer)
    }
}

lazy_static! {
    static ref QUERIES: Queries = Queries::new();
}

impl PM1QuerySender {
    pub fn send(&mut self) {}
}

impl PM1 {
    pub fn set_target(&self, target: Physical) {
        *self.target.lock().unwrap() = (Instant::now(), target);
    }
}

impl Iterator for PM1 {
    type Item = PM1Status;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(msg) = self.buffer.next() {
                if unsafe { msg.header().node_type() < 0x20 } {
                    println!("{}", msg);
                    return Some(PM1Status::Battery(0));
                }
            }
            match self.port.read(self.buffer.as_buf()) {
                Some(n) => self.buffer.notify_received(n),
                None => return None,
            };
        }
    }
}

#[inline]
fn message(node_type: u8, node_index: u8, msg_type: u8, data_field: bool) -> Message {
    Message::new(0, data_field, 3, node_type, node_index, msg_type)
}

pub mod node {
    pub const EVERY_TYPE: u8 = 0x3f;
    pub const EVERY_INDEX: u8 = 0x0f;
    pub const STATE: u8 = 0x80;

    pub mod vcu {
        pub const TYPE: u8 = 0x10;
        pub const BATTERY_PERSENT: u8 = 1;
    }

    pub mod ecu {
        pub const TYPE: u8 = 0x11;
        pub const CURRENT_POSITION: u8 = 6;
        pub const TARGET_SPEED: u8 = 1;
    }

    pub mod tcu {
        pub const TYPE: u8 = 0x12;
        pub const CURRENT_POSITION: u8 = 3;
        pub const TARGET_POSITION: u8 = 1;
    }
}
