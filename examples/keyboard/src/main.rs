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
            let mut speed = 0f32;
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
                                speed = f32::min(speed + 0.2, 1.0);
                                println!("speed = {:?}", speed);
                            }
                        }
                        KeyEvent {
                            code: KeyCode::Down,
                            modifiers: KeyModifiers::CONTROL,
                        } => {
                            if available() {
                                speed = f32::max(speed - 0.2, 0.0);
                                println!("speed = {:?}", speed);
                            }
                        }
                        KeyEvent {
                            code: KeyCode::Up,
                            modifiers: KeyModifiers::NONE,
                        } => {
                            handle.set_target(Physical { speed, rudder: 0.0 });
                        }
                        KeyEvent {
                            code: KeyCode::Down,
                            modifiers: KeyModifiers::NONE,
                        } => {
                            handle.set_target(Physical {
                                speed: -speed,
                                rudder: 0.0,
                            });
                        }
                        KeyEvent {
                            code: KeyCode::Left,
                            modifiers: KeyModifiers::NONE,
                        } => {
                            handle.set_target(Physical {
                                speed,
                                rudder: -FRAC_PI_2,
                            });
                        }
                        KeyEvent {
                            code: KeyCode::Right,
                            modifiers: KeyModifiers::NONE,
                        } => {
                            handle.set_target(Physical {
                                speed,
                                rudder: FRAC_PI_2,
                            });
                        }
                        _ => {
                            println!();
                            handle.set_target(Physical::ZERO);
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
