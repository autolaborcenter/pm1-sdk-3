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
    sync::{Arc, Mutex},
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
    port: Arc<Port>,

    control_period: Duration,
    next: Instant,
    index: usize,
}

#[derive(Debug)]
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

        let mut messages = MSG;
        let mut buffer = [0u8; 30];
        for i in 0..messages.len() {
            messages[i].write();
            buffer[i * 6..][..6].copy_from_slice(&messages[i].as_slice());
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
            self.port.write(&QUERIES.0[..6 * len]);
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
    const TARGET_MEMORY_WINDOW: Duration = Duration::from_millis(200); // 超过这个窗口，则将目标状态悬空
    const PAD_CONTROL_WINDOW: Duration = Duration::from_millis(200); // 超过这个窗口，则可接管控制

    pub fn set_target(&self, target: Physical) {
        *self.target.lock().unwrap() = (Instant::now() + Self::TARGET_MEMORY_WINDOW, target);
    }

    fn receive(&mut self, time: Instant, msg: Message) -> Option<PM1Status> {
        let header = unsafe { msg.header() };
        let _type = (header.node_type(), header.msg_type());

        let mut target: Option<Physical> = None;

        let result = match _type.1 {
            // 底盘发送了软件锁定或解锁
            // 这意味着通过遥控器或急停按钮进行了操作
            STOP => {
                // 暂时抑制控制
                self.using_pad = time;
                // 清除之前设置的目标状态，如同已经超时
                *self.target.lock().unwrap() = (time, Physical::RELEASED);
                None
            }
            // 节点状态
            // 多种触发条件
            STATE => {
                self.state_memory
                    .insert((header.node_type(), header.node_index()), unsafe {
                        msg.read().read_unchecked()
                    });
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
                        if header.data_field() {
                            let battery_percent = unsafe { msg.read().read_unchecked() };
                            self.battery_percent = battery_percent;
                            Some(PM1Status::Battery(self.battery_percent))
                        } else {
                            None
                        }
                    }
                    // 急停开关
                    // 主动询问
                    vcu::POWER_SWITCH => {
                        if header.data_field() {
                            let power_switch: u8 = unsafe { msg.read().read_unchecked() };
                            self.power_switch = power_switch > 0;
                        }
                        None
                    }
                    // 其他，不需要解析
                    _ => None,
                },
                // 动力控制器
                ecu::TYPE => match _type.1 {
                    // 目标速度
                    // 接收到这个意味着正在使用遥控器
                    ecu::TARGET_SPEED => {
                        self.using_pad = time;
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
                tcu::TYPE => match _type.1 {
                    // 目标角度
                    // 接收到这个意味着正在使用遥控器
                    tcu::TARGET_POSITION => {
                        self.using_pad = time;
                        None
                    }
                    // 当前角度
                    // 使用遥控器或主动询问
                    tcu::CURRENT_POSITION => {
                        if header.data_field() {
                            {
                                // 更新状态
                                let rudder = RUDDER
                                    .pluses_to_rad(
                                        unsafe { msg.read().read_unchecked::<i16>() } as i32
                                    );
                                self.current.rudder = if rudder > FRAC_PI_2 {
                                    FRAC_PI_2
                                } else if rudder < -FRAC_PI_2 {
                                    -FRAC_PI_2
                                } else {
                                    rudder
                                };
                            }
                            // 正在使用遥控器，跳过控制
                            if time > self.using_pad + Self::PAD_CONTROL_WINDOW {
                                let (deadline, physical) = *self.target.lock().unwrap();
                                target = if time >= deadline {
                                    // 距离上次接收已经超时
                                    if self.current.speed == 0.0 {
                                        None
                                    } else {
                                        Some(Physical::RELEASED)
                                    }
                                } else {
                                    Some(physical)
                                };
                            }
                            Some(PM1Status::Status(self.current))
                        } else {
                            // VCU 询问 TCU
                            // 接收到这个意味着正在使用遥控器
                            self.using_pad = time;
                            None
                        }
                    }
                    // 其他，不需要解析
                    _ => None,
                },
                // 其他，不需要解析
                _ => None,
            },
        };

        let mut reply = [0u8; 14 * 4];
        let mut cursor = 0usize;

        if let Some(mut target) = target {
            if self.state_memory.iter().any(|(_, s)| *s == 0xff) {
                // 解锁
                let mut message = message(EVERY_TYPE, EVERY_INDEX, STOP, true);
                unsafe { message.write().write_unchecked(0xff as u8) };
                reply[..14].copy_from_slice(message.as_slice());
                cursor += 14;
            }
            // 控制
            if target.rudder.is_nan() {
                target.rudder = self.current.rudder;
            }
            target.speed = self.optimizer.optimize_speed(target, self.current);
            self.current.speed = target.speed;
            let Wheels { left: l, right: r } = self.model.physical_to_wheels(self.current);
            {
                let l = WHEEL.rad_to_pulses(l);
                let mut message = message(ecu::TYPE, 0, ecu::TARGET_SPEED, true);
                unsafe { message.write().write_unchecked(l) };
                reply[cursor..][..14].copy_from_slice(message.as_slice());
                cursor += 14;
            }
            {
                let r = WHEEL.rad_to_pulses(r);
                let mut message = message(ecu::TYPE, 1, ecu::TARGET_SPEED, true);
                unsafe { message.write().write_unchecked(r) };
                reply[cursor..][..14].copy_from_slice(message.as_slice());
                cursor += 14;
            }
            {
                let r = RUDDER.rad_to_pulses(target.rudder) as i16;
                let mut message = message(tcu::TYPE, 0, tcu::TARGET_POSITION, true);
                unsafe { message.write().write_unchecked(r) };
                reply[cursor..][..14].copy_from_slice(message.as_slice());
                cursor += 14;
            }
        }

        if cursor > 0 {
            self.port.write(&reply[..cursor]);
        }

        result
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
