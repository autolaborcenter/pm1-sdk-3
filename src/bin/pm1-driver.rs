use driver::{SupersivorEventForSingle::*, SupervisorForSingle};
use pm1_control_model::Odometry;
use pm1_sdk::{PM1Event, PM1};
use std::{thread, time::Duration};

fn main() {
    let mut odometry = Odometry::ZERO;

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
            Event(_, Some((_, PM1Event::Odometry(delta)))) => {
                odometry += delta;
                println!("{}", odometry);
            }
            Event(_, Some((_, e))) => println!("{:?}", e),
            Event(_, None) => {}
        };
        true
    });
}
