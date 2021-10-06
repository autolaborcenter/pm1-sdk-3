use pm1::{pm1, PM1QuerySender, PM1};
use serial_port::{Port, SerialPort};
use std::{
    sync::Arc,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[macro_use]
extern crate lazy_static;

pub mod pm1;

pub struct PM1Threads;

#[macro_export]
macro_rules! find_pm1 {
    () => {
        pm1_sdk_3::PM1Threads::open_all()
    };
    ($($x:expr)+) => {
        pm1_sdk_3::PM1Threads::open_some(&[$(String::from($x),)*])
    };
}

impl PM1Threads {
    /// 打开一些串口
    pub fn open_some(paths: &[String]) -> Option<Box<PM1>> {
        let mut senders = Vec::<Option<Box<PM1QuerySender>>>::new();
        let mut chassis = Vec::<Option<Box<PM1>>>::new();
        for (sender, pm1) in paths.iter().filter_map(may_open) {
            senders.push(Some(Box::new(sender)));
            chassis.push(Some(Box::new(pm1)));
        }

        send_on_single_thread(senders);

        {
            let counter = Arc::new(());
            chassis
                .iter_mut()
                .map(|c| {
                    let counter = counter.clone();
                    let mut pm1 = std::mem::replace(c, None);
                    Some(thread::spawn(move || loop {
                        if let Some(_) = pm1.as_mut().unwrap().next() {
                            if Arc::strong_count(&counter) == 1 {
                                return pm1;
                            }
                        } else {
                            return None;
                        }
                    }))
                })
                .collect::<Vec<_>>()
        }
        .iter_mut()
        .find_map(|h| std::mem::replace(h, None).unwrap().join().ok().flatten())
    }

    /// 打开所有串口
    pub fn open_all() -> Option<Box<PM1>> {
        Self::open_some(Port::list().as_slice())
    }
}

fn may_open(name: &String) -> Option<(PM1QuerySender, PM1)> {
    let path: String = if cfg!(target_os = "windows") {
        name.rmatch_indices("COM")
            .next()
            .map(|m| &name.as_str()[m.0..name.len() - 1])
            .unwrap()
            .into()
    } else {
        name.clone()
    };

    match Port::open(path.as_str(), 115200, 200) {
        Ok(port) => Some(pm1(port)),
        Err(e) => {
            eprintln!("failed to open {}: {}", path, e);
            None
        }
    }
}

struct Timer(Instant);

impl Timer {
    fn wait_per(&mut self, period: Duration) {
        let now = Instant::now();
        while self.0 <= now {
            self.0 += period;
        }
        thread::sleep(self.0 - now);
    }
}

fn send_on_single_thread(mut senders: Vec<Option<Box<PM1QuerySender>>>) -> JoinHandle<()> {
    const PERIOD: Duration = Duration::from_millis(40);
    thread::spawn(move || {
        let mut timer = Timer(Instant::now());
        loop {
            // 等待仅剩一个
            let count = senders
                .iter_mut()
                .filter_map(|o| {
                    if let Some(ref mut s) = o {
                        if s.send() {
                            Some(())
                        } else {
                            *o = None;
                            None
                        }
                    } else {
                        None
                    }
                })
                .count();
            match count {
                0 => return,
                1 => break,
                _ => timer.wait_per(PERIOD),
            }
        }
        let mut sender = std::mem::replace(&mut senders[0], None).unwrap();
        while sender.send() {
            timer.wait_per(PERIOD);
        }
    })
}
