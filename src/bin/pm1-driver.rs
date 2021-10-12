use driver::{Driver, SupersivorEventForSingle::*, SupervisorForSingle};
use pm1_control_model::{Odometry, Physical};
use pm1_sdk::{PM1Status, PM1};
use std::{
    sync::mpsc::*,
    thread,
    time::{Duration, Instant},
};

enum Request {
    S,
    P(Physical),
    T(Physical),
}

fn main() {
    let (sender, receiver) = sync_channel(0);

    thread::spawn(move || {
        SupervisorForSingle::<PM1>::new().join(|e| {
            match e {
                Connected(_, driver) => eprintln!("Connected: {}", driver.status()),
                ConnectFailed => {
                    eprintln!("Failed.");
                    thread::sleep(Duration::from_secs(1));
                }
                Disconnected => {
                    eprintln!("Disconnected.");
                    thread::sleep(Duration::from_secs(1));
                }
                Event(pm1, _) => match receiver.try_recv() {
                    Ok(Request::S) => {
                        let PM1Status {
                            battery_percent,
                            power_switch: _,
                            physical,
                            odometry,
                        } = pm1.status();
                        println!(
                            "S {} {} {} {}",
                            battery_percent, physical.speed, physical.rudder, odometry.s
                        );
                    }
                    Ok(Request::T(p)) => {
                        let mut pose = Odometry::ZERO;
                        let (model, mut predictor) = pm1.predict();
                        predictor.set_target(p);
                        for _ in 0..20 {
                            for _ in 0..5 {
                                match predictor.next() {
                                    Some(s) => {
                                        pose += model.physical_to_odometry(Physical {
                                            speed: s.speed * 0.04,
                                            ..s
                                        });
                                    }
                                    None => {
                                        pose += model.physical_to_odometry(Physical {
                                            speed: p.speed * 0.04,
                                            ..p
                                        });
                                    }
                                }
                            }
                            print!(
                                " {},{},{}",
                                pose.pose.translation.vector[0],
                                pose.pose.translation.vector[1],
                                pose.pose.rotation.angle()
                            );
                        }
                        println!();
                    }
                    Ok(Request::P(p)) => pm1.send((Instant::now(), p)),
                    _ => {}
                },
            };
            true
        });
    });

    let mut line = String::new();
    loop {
        line.clear();
        match std::io::stdin().read_line(&mut line) {
            Ok(_) => {
                let tokens = line.split_whitespace().collect::<Vec<_>>();
                match tokens.get(0) {
                    Some(&"S") => {
                        if tokens.len() == 1 {
                            let _ = sender.send(Request::S);
                        }
                    }
                    Some(&"T") => {
                        if tokens.len() == 3 {
                            if let Some(p) = parse(tokens[1], tokens[2]) {
                                let _ = sender.send(Request::T(p));
                            }
                        }
                    }
                    Some(&"P") => {
                        if tokens.len() == 3 {
                            if let Some(p) = parse(tokens[1], tokens[2]) {
                                let _ = sender.send(Request::P(p));
                            }
                        }
                    }
                    _ => {}
                }
            }
            Err(_) => return,
        }
    }
}

fn parse(speed: &str, rudder: &str) -> Option<Physical> {
    let speed = speed.parse();
    let rudder = rudder.parse();
    if speed.is_ok() {
        if rudder.is_ok() {
            Some(Physical {
                speed: speed.unwrap(),
                rudder: rudder.unwrap(),
            })
        } else if speed.unwrap() == 0.0 {
            Some(Physical::RELEASED)
        } else {
            None
        }
    } else {
        None
    }
}
