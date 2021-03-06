use driver::{Driver, DriverPacemaker};
use model::{Pm1Model, Pm1Predictor, TrajectoryPredictor};
use pm1_control_model::{Motor, Optimizer, Physical, Wheels};
use serial_port::{Port, PortKey, SerialPort};
use std::{
    collections::HashMap,
    f32::consts::FRAC_PI_2,
    fmt::Display,
    sync::{Arc, Mutex, Weak},
    time::{Duration, Instant},
};

mod autocan;
mod differential;

use self::node::*;
use autocan::{Message, MessageBuffer};
use differential::Differential;

pub extern crate driver;
pub extern crate pm1_control_model as model;

pub const CONTROL_PERIOD: Duration = Duration::from_millis(40); // 控制周期
const TARGET_MEMORY_TIMEOUT: Duration = Duration::from_millis(200); // 超时则将目标改为停止
const PAD_CONTROL_TIMEOUT: Duration = Duration::from_millis(200); // 在此保护时间内不进行控制
const MESSAGE_RECEIVE_TIMEOUT: Duration = Duration::from_millis(200); // 超时认为底盘已断开，立即退出
const MESSAGE_PARSE_TIMEOUT: Duration = Duration::from_millis(250); // 超时认为底盘已断开，立即退出
const OPEN_TIMEOUT: Duration = Duration::from_secs(1);

/// 保存底盘状态的结构体。
///
/// 通过 `PM1` 的一个实例，可以完成以下三项功能：
///
/// - 接收和解析串口协议
/// - 缓存并随时读取底盘状态
/// - 控制底盘移动
pub struct PM1 {
    port: Arc<Port>,
    buffer: MessageBuffer<32>,
    last_time: Instant,

    using_pad: Instant,
    state_memory: HashMap<(u8, u8), u8>,
    status: PM1Status,
    target: (Instant, Physical),

    pub model: Pm1Model,
    differential: Differential,
    optimizer: Optimizer,
}

#[derive(Clone, Copy)]
pub struct PM1Status {
    pub battery_percent: u8,
    pub power_switch: bool,
    pub physical: Physical,
}

pub struct PM1Pacemaker {
    port: Weak<Port>,

    next: Instant,
    index: usize,
}

#[derive(Clone)]
pub struct PM1Handle(Weak<Mutex<(Instant, Physical)>>);

#[derive(Debug)]
pub enum PM1Event {
    Battery(u8),
    PowerSwitch(bool),
    Physical(Physical),
    Wheels(Wheels),
}

impl DriverPacemaker for PM1Pacemaker {
    #[inline]
    fn period() -> Duration {
        CONTROL_PERIOD
    }

    fn send(&mut self) -> bool {
        let now = Instant::now();
        let mut len = 0usize;
        while self.next < now {
            self.next += CONTROL_PERIOD;
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
    #[inline]
    pub fn status<'a>(&'a self) -> &'a PM1Status {
        &self.status
    }

    #[inline]
    pub fn set_target(&mut self, target: (Instant, Physical)) {
        self.target = (target.0 + TARGET_MEMORY_TIMEOUT, target.1);
    }

    #[inline]
    pub fn drive(&mut self, target: Physical) {
        self.set_target((Instant::now(), target))
    }

    #[inline]
    pub fn status_predictor(&self) -> Pm1Predictor {
        Pm1Predictor::new(self.optimizer, CONTROL_PERIOD)
    }

    #[inline]
    pub fn trajectory_predictor(&self) -> TrajectoryPredictor<Pm1Predictor> {
        model::TrajectoryPredictor {
            period: CONTROL_PERIOD,
            model: self.model.clone(),
            predictor: self.status_predictor(),
        }
    }
}

impl Driver for PM1 {
    type Key = PortKey;
    type Pacemaker = PM1Pacemaker;
    type Event = PM1Event;

    #[inline]
    fn keys() -> Vec<Self::Key> {
        Port::list().into_iter().map(|id| id.key).collect()
    }

    #[inline]
    fn open_timeout() -> Duration {
        OPEN_TIMEOUT
    }

    fn new(key: &Self::Key) -> Option<(Self::Pacemaker, Self)> {
        match Port::open(key, 115200, MESSAGE_RECEIVE_TIMEOUT.as_millis() as u32) {
            Ok(port) => {
                let now = Instant::now();
                let port = Arc::new(port);
                let sender = PM1Pacemaker {
                    port: Arc::downgrade(&port),

                    next: now,
                    index: 0,
                };
                sender.send_len(5);
                Some((
                    sender,
                    PM1 {
                        port,
                        buffer: Default::default(),
                        last_time: now,

                        using_pad: now,
                        state_memory: HashMap::new(),
                        status: PM1Status {
                            battery_percent: 0,
                            power_switch: false,
                            physical: Physical::RELEASED,
                        },
                        target: (now, Physical::RELEASED),

                        differential: Differential::new(),
                        model: Default::default(),
                        optimizer: Optimizer::new(0.5, 1.2, CONTROL_PERIOD),
                    },
                ))
            }
            Err(_) => None,
        }
    }

    fn join<F>(&mut self, mut f: F) -> bool
    where
        F: FnMut(&mut Self, Option<(Instant, Self::Event)>) -> bool,
    {
        let mut time = Instant::now();
        loop {
            if let Some(msg) = self.buffer.next() {
                time = self.last_time;
                // 成功从缓存中消费
                let event = self.receive(time, msg);
                if !f(self, event) {
                    // 如果回调指示不要继续阻塞，立即退出
                    return true;
                }
            } else if self.last_time > time + MESSAGE_PARSE_TIMEOUT {
                // 解析超时
                return false;
            } else {
                // 重新接收
                match self.port.read(self.buffer.as_buf()) {
                    // 成功接收
                    Some(n) => {
                        if n == 0 {
                            // 串口超时
                            return false;
                        } else {
                            // 成功接收
                            self.last_time = Instant::now();
                            self.buffer.notify_received(n);
                        }
                    }
                    // 无法接收
                    None => return false,
                };
            }
        }
    }
}

struct Queries([u8; 30]);

impl Queries {
    fn new() -> Self {
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

lazy_static::lazy_static! {
    static ref QUERIES: Queries = Queries::new();
}

impl PM1Pacemaker {
    #[inline]
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
}

impl PM1 {
    #[inline]
    fn detect_control_pad(&mut self, time: Instant) {
        self.using_pad = time;
        self.status.physical.speed = 0.0;
    }

    #[inline]
    fn update_battery_percent(&mut self, battery_percent: u8) -> Option<PM1Event> {
        if battery_percent != self.status.battery_percent {
            self.status.battery_percent = battery_percent;
            Some(PM1Event::Battery(battery_percent))
        } else {
            None
        }
    }

    #[inline]
    fn update_power_switch(&mut self, power_switch: u8) -> Option<PM1Event> {
        let power_switch = power_switch != 0;
        if power_switch != self.status.power_switch {
            self.status.power_switch = power_switch;
            Some(PM1Event::PowerSwitch(power_switch))
        } else {
            None
        }
    }

    fn update_odometry(&mut self, time: Instant, which: u8, value: i32) -> Option<PM1Event> {
        if let Some((dl, dr)) = self.differential.update(time, which, value) {
            if dl == 0 && dr == 0 {
                None
            } else {
                Some(PM1Event::Wheels(Wheels {
                    left: Motor::WHEEL.pluses_to_rad(dl),
                    right: Motor::WHEEL.pluses_to_rad(dr),
                }))
            }
        } else {
            None
        }
    }

    fn update_rudder(&mut self, time: Instant, rudder: i16) -> Option<PM1Event> {
        let rudder = Motor::RUDDER.pluses_to_rad(rudder.into());
        let mut current = self.status.physical;
        // 更新状态
        current.rudder = if rudder > FRAC_PI_2 {
            FRAC_PI_2
        } else if rudder < -FRAC_PI_2 {
            -FRAC_PI_2
        } else {
            rudder
        };
        // 正在使用遥控器，跳过控制
        let target = if time > self.using_pad + PAD_CONTROL_TIMEOUT {
            let (deadline, physical) = self.target;
            if time >= deadline {
                // 距离上次接收已经超时
                if current.speed == 0.0 {
                    None
                } else {
                    Some(Physical::RELEASED)
                }
            } else if !self.status.power_switch {
                // 急停按开关断开
                self.target = (time, Physical::RELEASED);
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
                msg[1]
                    .write()
                    .write_unchecked(Motor::WHEEL.rad_to_pulses(l));
                msg[2]
                    .write()
                    .write_unchecked(Motor::WHEEL.rad_to_pulses(r));
                msg[3]
                    .write()
                    .write_unchecked(Motor::RUDDER.rad_to_pulses(target.rudder) as i16);
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
        if current != self.status.physical {
            self.status.physical = current;
            Some(PM1Event::Physical(current))
        } else {
            None
        }
    }

    fn receive(&mut self, time: Instant, msg: Message) -> Option<(Instant, PM1Event)> {
        let header = msg.header();
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
                                .and_then(|e| Some((time, e)))
                        } else {
                            None
                        }
                    }
                    // 急停开关
                    // 主动询问
                    vcu::POWER_SWITCH => {
                        if data {
                            self.update_power_switch(unsafe { msg.read().read_unchecked() })
                                .and_then(|e| Some((time, e)))
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
                        if data {
                            self.update_odometry(time, header.node_index(), unsafe {
                                msg.read().read_unchecked()
                            })
                            .and_then(|e| Some((time, e)))
                        } else {
                            None
                        }
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
                                .and_then(|e| Some((time, e)))
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

#[inline]
const fn message(node_type: u8, node_index: u8, msg_type: u8, data_field: bool) -> Message {
    Message::new(0, data_field, 3, node_type, node_index, msg_type)
}

impl Display for PM1Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Battery: {}% | Speed: {}m/s | Rudder: {}rad{}",
            self.battery_percent,
            self.physical.speed,
            self.physical.rudder,
            if !self.power_switch {
                " | Power Switch Off"
            } else {
                ""
            }
        )
    }
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
