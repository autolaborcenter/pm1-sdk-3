use driver::{Driver, SupersivorEventForSingle::*, SupervisorForSingle};
use pm1_sdk::PM1Supervisor;
use std::{thread, time::Duration};

fn main() {
    PM1Supervisor::new().join(|e| {
        match e {
            Connected(driver) => println!("Connected: {}", driver.status()),
            ConnectFailed => {
                println!("Failed.");
                thread::sleep(Duration::from_secs(1));
            }
            Disconnected => {
                println!("Disconnected.");
                thread::sleep(Duration::from_secs(1));
            }
            Event(_, Some((_, event))) => println!("Event: {:?}", event),
            Event(_, None) => {}
        };
        true
    });
}
