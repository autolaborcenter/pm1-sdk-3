use self::node::*;
use autocan::{Message, MessageBuffer};
use pm1_control_model::{
    model::ChassisModel,
    motor::{RUDDER, WHEEL},
    optimizer::Optimizer,
    Physical, Wheels,
};
use serial_port::{Port, SerialPort};
use std::{
    collections::HashMap,
    f32::consts::FRAC_PI_2,
    sync::{Arc, Mutex, Weak},
    time::{Duration, Instant},
};

pub mod autocan;

pub struct PM1 {
    port: Arc<Port>,
    buffer: MessageBuffer<32>,

    using_pad: Instant,
    battery_percent: u8,
    power_switch: bool,
    state_memory: HashMap<(u8, u8), u8>,

    target: Arc<Mutex<(Instant, Physical)>>,
    current: Physical,

    model: ChassisModel,
    optimizer: Optimizer,
}

pub struct PM1QuerySender {
    port: Weak<Port>,

    control_period: Duration,
    next: Instant,
    index: usize,
}

pub struct PM1Interface {}

#[derive(Debug)]
pub enum PM1Status {
    Battery(u8),
    PowerSwitch(bool),
    Status(Physical),
    Odometry(f32, nalgebra::Isometry2<f32>),
}

pub fn pm1(port: Port) -> (PM1QuerySender, PM1) {
    let control_period = Duration::from_millis(40);
    let now = Instant::now();
    let port = Arc::new(port);
    let sender = PM1QuerySender {
        port: Arc::downgrade(&port),

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

            using_pad: now,
            battery_percent: 0,
            power_switch: false,
            state_memory: HashMap::new(),
            current: Physical::RELEASED,

            target: Arc::new(Mutex::new((now, Physical::RELEASED))),
            model: Default::default(),
            optimizer: Optimizer::new(0.5, 1.2, control_period),
        },
    )
}

struct Queries([u8; 30]);

impl Queries {
    pub fn new() -> Self {
        const MSG: [Message; 5] = [
            message(tcu::TYPE, EVERY_INDEX, tcu::CURRENT_POSITION, false),
            message(ecu::TYPE, EVERY_INDEX, ecu::CURRENT_POSITION, false),
            message(EVERY_TYPE, EVERY_INDEX, STATE, false),
            message(vcu::TYPE, EVERY_INDEX, vcu::POWER_SWITCH, false),
            message(vcu::TYPE, EVERY_INDEX, vcu::BATTERY_PERCENT, false),
        ];

        let mut buffer = [0u8; 30];
        let mut i = 0;
        while i < MSG.len() {
            buffer[i * 6..][..6].copy_from_slice(&MSG[i].as_slice());
            i += 1;
        }
        Self(buffer)
    }
}

lazy_static! {
    static ref QUERIES: Queries = Queries::new();
}

impl PM1QuerySender {
    fn send_len(&self, len: usize) -> bool {
        if let Some(port) = self.port.upgrade() {
            if len > 0 {
                port.write(&QUERIES.0[..6 * len]);
            }
            true
        } else {
            false
        }
    }

    pub fn send(&mut self) -> bool {
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
        self.send_len(len)
    }
}

impl PM1 {
    const TARGET_MEMORY_WINDOW: Duration = Duration::from_millis(200); // 超过这个窗口，则将目标状态悬空
    const PAD_CONTROL_WINDOW: Duration = Duration::from_millis(200); // 超过这个窗口，则可接管控制

    pub fn set_target(&self, target: Physical) {
        *self.target.lock().unwrap() = (Instant::now() + Self::TARGET_MEMORY_WINDOW, target);
    }

    fn detect_control_pad(&mut self, time: Instant) {
        self.using_pad = time;
        self.current.speed = 0.0;
    }

    fn update_battery_percent(&mut self, battery_percent: u8) -> Option<PM1Status> {
        if battery_percent != self.battery_percent {
            self.battery_percent = battery_percent;
            Some(PM1Status::Battery(battery_percent))
        } else {
            None
        }
    }

    fn update_power_switch(&mut self, power_switch: u8) -> Option<PM1Status> {
        let power_switch = power_switch != 0;
        if power_switch != self.power_switch {
            self.power_switch = power_switch;
            Some(PM1Status::PowerSwitch(power_switch))
        } else {
            None
        }
    }

    fn update_rudder(&mut self, time: Instant, rudder: i16) -> Option<PM1Status> {
        let rudder = RUDDER.pluses_to_rad(rudder.into());
        let mut current = self.current;
        // 更新状态
        current.rudder = if rudder > FRAC_PI_2 {
            FRAC_PI_2
        } else if rudder < -FRAC_PI_2 {
            -FRAC_PI_2
        } else {
            rudder
        };
        // 正在使用遥控器，跳过控制
        let target = if time > self.using_pad + Self::PAD_CONTROL_WINDOW {
            let mut guard = self.target.lock().unwrap();
            let (deadline, physical) = *guard;
            if time >= deadline {
                // 距离上次接收已经超时
                if current.speed == 0.0 {
                    None
                } else {
                    Some(Physical::RELEASED)
                }
            } else if !self.power_switch {
                // 急停按开关断开
                *guard = (time, Physical::RELEASED);
                None
            } else {
                Some(physical)
            }
        } else {
            None
        };
        if let Some(mut target) = target {
            // 执行优化，更新缓存
            if target.rudder.is_nan() {
                target.rudder = current.rudder;
            }
            target.speed = self.optimizer.optimize_speed(target, current);
            current.speed = target.speed;
            let Wheels { left: l, right: r } = self.model.physical_to_wheels(current);
            // 编码
            let reply = unsafe {
                const MSG: [Message; 4] = [
                    message(EVERY_TYPE, EVERY_INDEX, STOP, true),
                    message(ecu::TYPE, 0, ecu::TARGET_SPEED, true),
                    message(ecu::TYPE, 1, ecu::TARGET_SPEED, true),
                    message(tcu::TYPE, 0, tcu::TARGET_POSITION, true),
                ];
                const LEN: usize = std::mem::size_of::<Message>();

                let mut msg = MSG;
                // 控制
                msg[1].write().write_unchecked(WHEEL.rad_to_pulses(l));
                msg[2].write().write_unchecked(WHEEL.rad_to_pulses(r));
                msg[3]
                    .write()
                    .write_unchecked(RUDDER.rad_to_pulses(target.rudder) as i16);
                // 解锁
                let msg = if self.state_memory.iter().any(|(_, s)| *s == 0xff) {
                    msg[0].write().write_unchecked(0xff as u8);
                    &msg
                } else {
                    &msg[1..]
                };
                std::slice::from_raw_parts(msg.as_ptr() as *const u8, msg.len() * LEN)
            };

            self.port.write(reply);
        }
        if current != self.current {
            self.current = current;
            Some(PM1Status::Status(current))
        } else {
            None
        }
    }

    fn receive(&mut self, time: Instant, msg: Message) -> Option<PM1Status> {
        let header = unsafe { msg.header() };
        let data = header.data_field();
        let t_node = header.node_type();
        let i_node = header.node_index();
        let t_msg = header.msg_type();

        match t_msg {
            // 底盘发送了软件锁定或解锁
            // 这意味着通过遥控器或急停按钮进行了操作
            STOP => {
                self.detect_control_pad(time);
                None
            }
            // 节点状态
            // 多种触发条件
            STATE => {
                self.state_memory
                    .insert((t_node, i_node), unsafe { msg.read().read_unchecked() });
                None
            }
            // 不需要解析的跨节点协议
            0x80.. => None,
            // 其他范围，不属于可在节点间广播的协议
            _ => match t_node {
                // 车辆控制器
                vcu::TYPE => match t_msg {
                    // 电池百分比
                    // 主动询问
                    vcu::BATTERY_PERCENT => {
                        if data {
                            self.update_battery_percent(unsafe { msg.read().read_unchecked() })
                        } else {
                            None
                        }
                    }
                    // 急停开关
                    // 主动询问
                    vcu::POWER_SWITCH => {
                        if data {
                            self.update_power_switch(unsafe { msg.read().read_unchecked() })
                        } else {
                            None
                        }
                    }
                    // 其他，不需要解析
                    _ => None,
                },
                // 动力控制器
                ecu::TYPE => match t_msg {
                    // 目标速度
                    // 接收到这个意味着正在使用遥控器
                    ecu::TARGET_SPEED => {
                        self.detect_control_pad(time);
                        // TODO 在这里也可以接收，以同步本地状态
                        None
                    }
                    // 当前位置
                    // 主动询问
                    ecu::CURRENT_POSITION => {
                        // TODO 更新里程计
                        None
                    }
                    // 其他，不需要解析
                    _ => None,
                },
                // 转向控制器
                tcu::TYPE => match t_msg {
                    // 目标角度
                    // 接收到这个意味着正在使用遥控器
                    tcu::TARGET_POSITION => {
                        self.detect_control_pad(time);
                        None
                    }
                    // 当前角度
                    // 使用遥控器或主动询问
                    tcu::CURRENT_POSITION => {
                        if data {
                            self.update_rudder(time, unsafe { msg.read().read_unchecked() })
                        } else {
                            // VCU 询问 TCU
                            self.detect_control_pad(time);
                            None
                        }
                    }
                    // 其他，不需要解析
                    _ => None,
                },
                // 其他，不需要解析
                _ => None,
            },
        }
    }
}

impl Iterator for PM1 {
    type Item = PM1Status;

    fn next(&mut self) -> Option<Self::Item> {
        let mut time = Instant::now();
        loop {
            if let Some(msg) = self.buffer.next() {
                if let Some(status) = self.receive(time, msg) {
                    return Some(status);
                }
            } else {
                match self.port.read(self.buffer.as_buf()) {
                    Some(n) => {
                        time = Instant::now();
                        self.buffer.notify_received(n);
                    }
                    // None => return None,
                    None => {}
                };
            }
        }
    }
}

#[inline]
const fn message(node_type: u8, node_index: u8, msg_type: u8, data_field: bool) -> Message {
    Message::new(0, data_field, 0, node_type, node_index, msg_type)
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
