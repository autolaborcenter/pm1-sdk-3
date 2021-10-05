use autocan::{Message, MessageBuffer};
use pm1_control_model::{model::ChassisModel, optimizer::Optimizer, Physical};
use serial_port::{Port, SerialPort};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use self::node::*;

pub mod autocan;

pub struct PM1 {
    port: Arc<Port>,
    buffer: MessageBuffer<32>,

    battery_percent: u8,
    power_switch: bool,
    state_memory: HashMap<(u8, u8), u8>,

    target: Arc<Mutex<(Instant, Physical)>>,

    control_period: Duration,
    model: ChassisModel,
    optimizer: Optimizer,
}

pub struct PM1QuerySender {
    port: Arc<Port>,

    control_period: Duration,
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
    let sender = PM1QuerySender {
        port: port.clone(),

        control_period,
        next: now,
        index: 0,
    };
    sender.send_len(5);
    (
        sender,
        PM1 {
            port,
            buffer: Default::default(),

            battery_percent: 0,
            power_switch: false,
            state_memory: HashMap::new(),

            target: Arc::new(Mutex::new((now, Physical::RELEASED))),
            control_period,
            model: Default::default(),
            optimizer: Optimizer::new(0.5, 1.2, control_period),
        },
    )
}

struct Queries([u8; 30]);

impl Queries {
    pub fn new() -> Self {
        let mut buffer = [0u8; 30];
        unsafe {
            let buffer = std::slice::from_raw_parts_mut(buffer.as_mut_ptr() as *mut Message, 5);
            buffer[0] = message(tcu::TYPE, EVERY_INDEX, tcu::CURRENT_POSITION, false);
            buffer[1] = message(ecu::TYPE, EVERY_INDEX, ecu::CURRENT_POSITION, false);
            buffer[2] = message(EVERY_TYPE, EVERY_INDEX, STATE, false);
            buffer[3] = message(vcu::TYPE, EVERY_INDEX, vcu::POWER_SWITCH, false);
            buffer[4] = message(vcu::TYPE, EVERY_INDEX, vcu::BATTERY_PERCENT, false);
            for item in buffer {
                item.write();
            }
        }
        Self(buffer)
    }
}

lazy_static! {
    static ref QUERIES: Queries = Queries::new();
}

impl PM1QuerySender {
    fn send_len(&self, len: usize) {
        if len > 0 {
            self.port.write(&QUERIES.0[..len * 6]);
        }
    }

    pub fn send(&mut self) {
        let now = Instant::now();
        let mut len = 0usize;
        while self.next < now {
            self.next += self.control_period;
            self.index += 1;
            if len == 5 {
            } else if self.index % 250 == 0 {
                len = 5; // 电池电量
            } else if len == 4 {
            } else if self.index % 10 == 0 {
                len = 4; // 状态和急停按钮
            } else if len == 2 {
            } else if self.index % 2 == 0 {
                len = 2; // 里程计
            } else {
                len = 1; // 后轮方向
            }
        }
        self.send_len(len);
    }
}

impl PM1 {
    pub fn set_target(&self, target: Physical) {
        *self.target.lock().unwrap() = (Instant::now(), target);
    }

    fn receive(&mut self, msg: Message) -> Option<PM1Status> {
        let header = unsafe { msg.header() };
        let _type = (header.node_type(), header.msg_type());

        println!("{}", msg);

        let mut reply = [0u8; 128]; // 126 = 14 * 9
        let mut cursor = 0usize;

        let mut result = match _type.1 {
            // 底盘发送了软件锁定或解锁
            // 目前不知道这有什么意义
            // 也许这个指令可以将上位机也锁定
            STOP => None,
            // 节点状态
            // 多种触发条件
            STATE => {
                self.state_memory.insert(
                    (header.node_type(), header.node_index()),
                    unsafe { msg.data() }[0],
                );
                None
            }
            // 不需要解析的跨节点协议
            0x80.. => None,
            // 其他范围，不属于可在节点间广播的协议
            _ => match _type.0 {
                // 车辆控制器
                vcu::TYPE => match _type.1 {
                    // 电池百分比
                    // 主动询问
                    vcu::BATTERY_PERCENT => {
                        let battery_percent = unsafe { msg.data() }[0];
                        self.battery_percent = battery_percent;
                        Some(PM1Status::Battery(self.battery_percent))
                    }
                    // 急停开关
                    // 主动询问
                    vcu::POWER_SWITCH => {
                        let power_switch = unsafe { msg.data() }[0];
                        self.power_switch = power_switch > 0;
                        println!("{}", power_switch);
                        None
                    }
                    // 其他，不需要解析
                    _ => None,
                },
                // 动力控制器
                ecu::TYPE => match _type.1 {
                    // 目标速度
                    // 接收到这个意味着正在使用遥控器
                    ecu::TARGET_SPEED => None,
                    // 当前位置
                    // 主动询问
                    ecu::CURRENT_POSITION => None,
                    // 其他，不需要解析
                    _ => None,
                },
                // 转向控制器
                tcu::TYPE => match _type.1 {
                    // 目标角度
                    // 接收到这个意味着正在使用遥控器
                    tcu::TARGET_POSITION => None,
                    // 当前角度
                    // 使用遥控器或主动询问
                    tcu::CURRENT_POSITION => None,
                    // 其他，不需要解析
                    _ => None,
                },
                // 其他，不需要解析
                _ => None,
            },
        };

        if cursor > 0 {
            self.port.write(&reply[..cursor]);
        }

        result
    }
}

impl Iterator for PM1 {
    type Item = PM1Status;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(msg) = self.buffer.next() {
                if let Some(status) = self.receive(msg) {
                    return Some(status);
                }
            } else {
                match self.port.read(self.buffer.as_buf()) {
                    Some(n) => self.buffer.notify_received(n),
                    // None => return None,
                    None => {}
                };
            }
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
    pub const STOP: u8 = 0xff;

    pub mod vcu {
        pub const TYPE: u8 = 0x10;
        pub const BATTERY_PERCENT: u8 = 1;
        pub const POWER_SWITCH: u8 = 7;
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
