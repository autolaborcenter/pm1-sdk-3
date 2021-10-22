use gilrs::{Axis, Button, EventType::*, Gamepad, Gilrs};
use pm1_sdk::{
    driver::{SupersivorEventForSingle::*, SupervisorForSingle},
    model::Physical,
    PM1,
};
use std::{
    cmp,
    f32::consts::{FRAC_PI_2, FRAC_PI_6},
};

fn main() {
    let mut gilrs = Gilrs::new().unwrap();
    let mut active_gamepad = None;
    let mut gear = 1;

    SupervisorForSingle::<PM1>::new().join(|e| {
        match e {
            Event(driver, _) => {
                while let Some(e) = gilrs.next_event() {
                    active_gamepad = Some(e.id);
                    match e.event {
                        ButtonReleased(Button::Start, _) => gear = cmp::min(5, gear + 1),
                        ButtonReleased(Button::Select, _) => gear = cmp::max(1, gear - 1),
                        _ => {}
                    }
                }
                let mut target = match active_gamepad {
                    Some(id) => map(&gilrs.gamepad(id)),
                    None => Physical::RELEASED,
                };
                target.speed *= gear as f32 / 5.0;
                println!("{:?}", target);
                driver.drive(target);
            }
            _ => {}
        }
        true
    });
    loop {}
}

fn map(gamepad: &Gamepad) -> Physical {
    let buttons = (
        gamepad.is_pressed(Button::North),
        gamepad.is_pressed(Button::South),
        gamepad.is_pressed(Button::West),
        gamepad.is_pressed(Button::East),
        gamepad.is_pressed(Button::DPadUp),
        gamepad.is_pressed(Button::DPadDown),
        gamepad.is_pressed(Button::DPadLeft),
        gamepad.is_pressed(Button::DPadRight),
    );
    match buttons {
        (true, _, true, false, _, _, _, _) => button(1, 1),
        (true, _, false, true, _, _, _, _) => button(1, -1),
        (true, _, _, _, _, _, _, _) => button(1, 0),
        (_, true, true, false, _, _, _, _) => button(-1, 1),
        (_, true, false, true, _, _, _, _) => button(-1, -1),
        (_, true, _, _, _, _, _, _) => button(-1, 0),
        (_, _, true, false, _, _, _, _) => button(0, 1),
        (_, _, false, true, _, _, _, _) => button(0, -1),
        (_, _, true, true, _, _, _, _) => button(0, 0),
        (_, _, _, _, true, _, true, false) => button(1, 1),
        (_, _, _, _, true, _, false, true) => button(1, -1),
        (_, _, _, _, true, _, _, _) => button(1, 0),
        (_, _, _, _, _, true, true, false) => button(-1, 1),
        (_, _, _, _, _, true, false, true) => button(-1, -1),
        (_, _, _, _, _, true, _, _) => button(-1, 0),
        (_, _, _, _, _, _, true, false) => button(0, 1),
        (_, _, _, _, _, _, false, true) => button(0, -1),
        (_, _, _, _, _, _, true, true) => button(0, 0),
        _ => {
            let lx = gamepad.value(Axis::LeftStickY);
            let ly = gamepad.value(Axis::LeftStickY);
            let rx = gamepad.value(Axis::RightStickX);
            let ry = gamepad.value(Axis::RightStickY);
            let x = if lx.abs() > rx.abs() { lx } else { rx };
            let y = if ly.abs() > ry.abs() { ly } else { ry };
            let speed = f32::max(x.abs(), y.abs());
            if speed < 0.01 {
                Physical::RELEASED
            } else if y >= 0.0 {
                Physical {
                    speed,
                    rudder: stick(x, y),
                }
            } else {
                Physical {
                    speed: -speed,
                    rudder: stick(x, -y),
                }
            }
        }
    }
}

fn stick(x: f32, y: f32) -> f32 {
    let rad = x.atan2(y);
    let map = rad.signum() * FRAC_PI_2;
    (rad / map).powi(2) * map
}

fn button(y: i8, x: i8) -> Physical {
    match (y, x) {
        (0, 0) => Physical::ZERO,
        (1, 0) => Physical {
            speed: 1.0,
            rudder: 0.0,
        },
        (-1, 0) => Physical {
            speed: -1.0,
            rudder: 0.0,
        },
        (0, 1) => Physical {
            speed: 1.0,
            rudder: -FRAC_PI_2,
        },
        (0, -1) => Physical {
            speed: 1.0,
            rudder: FRAC_PI_2,
        },
        (1, 1) => Physical {
            speed: 1.0,
            rudder: -FRAC_PI_6,
        },
        (1, -1) => Physical {
            speed: 1.0,
            rudder: FRAC_PI_6,
        },
        (-1, 1) => Physical {
            speed: -1.0,
            rudder: -FRAC_PI_6,
        },
        (-1, -1) => Physical {
            speed: -1.0,
            rudder: FRAC_PI_6,
        },
        _ => panic!("Impossible!"),
    }
}
