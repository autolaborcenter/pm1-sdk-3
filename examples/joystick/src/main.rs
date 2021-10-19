use async_std::{
    io,
    sync::{Arc, Mutex},
    task,
};
use joystick_win::JoyStick;
use pm1_sdk::{
    driver::{Driver, SupersivorEventForSingle, SupervisorForSingle},
    model::Physical,
    PM1,
};
use std::{
    sync::atomic::{AtomicU32, Ordering},
    time::Instant,
};

fn main() {
    let target = Arc::new(Mutex::new((Instant::now(), Physical::RELEASED)));
    let level = Arc::new(AtomicU32::new(0.3f32.to_bits()));
    {
        let target = target.clone();
        task::spawn(async move {
            let mut joystick = JoyStick::default();
            loop {
                let (duration, event) = joystick.read();
                if let Some((x, y)) = event {
                    let speed = f32::max(x.abs(), y.abs());
                    let p = if speed < 0.01 {
                        Physical::RELEASED
                    } else if y >= 0.0 {
                        Physical {
                            speed,
                            rudder: x.atan2(y),
                        }
                    } else {
                        Physical {
                            speed: -speed,
                            rudder: x.atan2(-y),
                        }
                    };
                    let now = Instant::now();
                    *target.lock().await = (now, p);
                    println!("{} {} {:?}", x, y, p);
                }
                task::sleep(duration).await;
            }
        });
    }
    {
        let level = level.clone();
        task::spawn(async move {
            let mut line = String::new();
            loop {
                if let Ok(_) = io::stdin().read_line(&mut line).await {
                    if let Ok(k) = line.trim().parse::<f32>() {
                        if 0.0 <= k && k <= 1.0 {
                            level.store(k.to_bits(), Ordering::Relaxed);
                        }
                    }
                }
            }
        });
    }
    SupervisorForSingle::<PM1>::new().join(|e| {
        match e {
            SupersivorEventForSingle::Event(chassis, _) => {
                let command = task::block_on(async {
                    let level = f32::from_bits(level.load(Ordering::Relaxed));
                    let (time, mut target) = *target.lock().await;
                    target.speed *= level;
                    (time, target)
                });
                chassis.send(command);
            }
            _ => {}
        };
        true
    });
}
