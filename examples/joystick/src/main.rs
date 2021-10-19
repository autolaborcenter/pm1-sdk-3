use joystick_win::JoyStick;
use pm1_sdk::{
    driver::{Driver, SupersivorEventForSingle, SupervisorForSingle},
    model::Physical,
    PM1,
};

use std::{
    sync::{Arc, Mutex},
    thread,
    time::Instant,
};

fn main() {
    let target = Arc::new(Mutex::new((Instant::now(), Physical::RELEASED)));
    {
        let target = target.clone();
        thread::spawn(move || {
            let mut joystick = JoyStick::default();
            loop {
                let (duration, event) = joystick.read();
                if let Some((x, y)) = event {
                    let (x, y) = map((x as f32) / 32768.0 - 1.0, 1.0 - (y as f32) / 32768.0);
                    let mut p = Physical {
                        speed: x.hypot(y),
                        rudder: x.atan2(y.abs()),
                    };
                    if y < 0.0 {
                        p.speed = -p.speed;
                    }
                    let now = Instant::now();
                    *target.lock().unwrap() = (now, p);
                    println!("{:.3} {:.3} {:?}", x, y, p);
                }
                thread::sleep(duration);
            }
        });
    }
    SupervisorForSingle::<PM1>::new().join(|e| {
        match e {
            SupersivorEventForSingle::Event(chassis, _) => chassis.send(*target.lock().unwrap()),
            _ => {}
        };
        true
    });
}

fn map(x: f32, y: f32) -> (f32, f32) {
    if x.abs() > y.abs() {
        (x.signum() * f32::min(x.abs(), (1.0 - y * y).sqrt()), y)
    } else {
        (x, y.signum() * f32::min(y.abs(), (1.0 - x * x).sqrt()))
    }
}
