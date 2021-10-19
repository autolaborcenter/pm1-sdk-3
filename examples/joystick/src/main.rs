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
    f32::consts::FRAC_PI_2,
    sync::atomic::{AtomicU32, Ordering},
    time::Instant,
};

fn main() {
    let target = Arc::new(Mutex::new((Instant::now(), Physical::RELEASED)));
    let level = Arc::new(AtomicU32::new(0.3f32.to_bits()));
    joystick(target.clone());
    keyboard(level.clone());
    SupervisorForSingle::<PM1>::new().join(|e| {
        match e {
            SupersivorEventForSingle::Event(chassis, _) => {
                let command = task::block_on(async {
                    let (time, mut target) = *target.lock().await;
                    target.speed *= f32::from_bits(level.load(Ordering::Relaxed));
                    (time, target)
                });
                chassis.send(command);
            }
            _ => {}
        };
        true
    });
}

fn joystick(target: Arc<Mutex<(Instant, Physical)>>) -> task::JoinHandle<()> {
    task::spawn(async move {
        let mut joystick = JoyStick::default();
        loop {
            let (duration, event) = joystick.read();
            if let Some((x, y)) = event {
                fn map(x: f32, y: f32) -> f32 {
                    let rad = x.atan2(y);
                    let map = rad.signum() * FRAC_PI_2;
                    (rad / map).powi(2) * map
                }

                let speed = f32::max(x.abs(), y.abs());
                let command = (
                    Instant::now(),
                    if speed < 0.01 {
                        Physical::RELEASED
                    } else if y >= 0.0 {
                        Physical {
                            speed,
                            rudder: map(x, y),
                        }
                    } else {
                        Physical {
                            speed: -speed,
                            rudder: map(x, -y),
                        }
                    },
                );
                *target.lock().await = command;
            }
            task::sleep(duration).await;
        }
    })
}

fn keyboard(level: Arc<AtomicU32>) -> task::JoinHandle<()> {
    task::spawn(async move {
        let mut line = String::new();
        loop {
            line.clear();
            if let Ok(_) = io::stdin().read_line(&mut line).await {
                if let Ok(k) = line.trim().parse::<f32>() {
                    println!("k = {}", k);
                    if 0.0 <= k && k <= 1.0 {
                        level.store(k.to_bits(), Ordering::Relaxed);
                    }
                }
            }
        }
    })
}
