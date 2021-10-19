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
                    let mut p = Physical {
                        speed: f32::max(x.abs(), y.abs()),
                        rudder: x.atan2(y.abs()),
                    };
                    if y < 0.0 {
                        p.speed = -p.speed;
                    }
                    let now = Instant::now();
                    *target.lock().unwrap() = (now, p);
                    println!("{} {} {:?}", x, y, p);
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
