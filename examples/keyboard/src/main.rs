use crossterm::{
    event::{read, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use pm1_control_model::Physical;
use pm1_sdk_3::find_pm1;
use std::{
    f32::consts::FRAC_PI_2,
    thread,
    time::{Duration, Instant},
};

fn main() {
    if let Some(chassis) = find_pm1!() {
        let handle = chassis.get_handle();
        thread::spawn(move || {
            let mut target = Physical::ZERO;
            let mut last: Option<(Instant, KeyEvent)> = None;

            let _ = enable_raw_mode();

            loop {
                if let Event::Key(event) = read().unwrap() {
                    let now = Instant::now();
                    let available = || {
                        last.is_none()
                            || now > last.unwrap().0 + Duration::from_millis(500)
                            || last.unwrap().1 != event
                    };

                    match event {
                        KeyEvent {
                            code: KeyCode::Char('c'),
                            modifiers: KeyModifiers::CONTROL,
                        } => break,
                        KeyEvent {
                            code: KeyCode::Up,
                            modifiers: KeyModifiers::CONTROL,
                        } => {
                            if available() {
                                target.speed = f32::min(target.speed + 0.05, 1.0);
                                println!("target = {:?}", target);
                            }
                        }
                        KeyEvent {
                            code: KeyCode::Down,
                            modifiers: KeyModifiers::CONTROL,
                        } => {
                            if available() {
                                target.speed = f32::max(target.speed - 0.05, 0.0);
                                println!("target = {:?}", target);
                            }
                        }
                        KeyEvent {
                            code: KeyCode::Char(' '),
                            modifiers: _,
                        } => {
                            target = Physical::ZERO;
                            println!("target = {:?}", target);
                            handle.set_target(target);
                        }
                        KeyEvent {
                            code: KeyCode::Up,
                            modifiers: KeyModifiers::NONE,
                        } => {
                            handle.set_target(target);
                        }
                        KeyEvent {
                            code: KeyCode::Down,
                            modifiers: KeyModifiers::NONE,
                        } => {
                            handle.set_target(Physical {
                                speed: -target.speed,
                                ..target
                            });
                        }
                        KeyEvent {
                            code: KeyCode::Left,
                            modifiers: KeyModifiers::NONE,
                        } => {
                            target.rudder = f32::max(target.rudder - 0.1, -FRAC_PI_2);
                            handle.set_target(target);
                        }
                        KeyEvent {
                            code: KeyCode::Right,
                            modifiers: KeyModifiers::NONE,
                        } => {
                            target.rudder = f32::min(target.rudder + 0.1, FRAC_PI_2);
                            handle.set_target(target);
                        }
                        _ => {
                            target.speed = 0.0;
                            handle.set_target(target);
                        }
                    };
                    last = Some((now, event));
                } else {
                    last = None;
                }
            }

            let _ = disable_raw_mode();
            println!("Press Another Ctrl+C to exit");
        });
        for event in chassis {
            println!("{:?}", event);
        }
    }
}
