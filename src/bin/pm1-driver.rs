use driver::{SupersivorEventForSingle::*, SupervisorForSingle};
use pm1_sdk::PM1;
use std::{thread::sleep, time::Duration};

fn main() {
    SupervisorForSingle::<PM1>::new().join(|e| {
        match e {
            Connected(_, driver) => println!("Connected: {}", driver.status()),
            ConnectFailed => {
                println!("Failed.");
                sleep(Duration::from_secs(1));
            }
            Disconnected => {
                println!("Disconnected.");
                sleep(Duration::from_secs(1));
            }
            Event(_, Some((_, event))) => println!("Event: {:?}", event),
            Event(_, None) => {}
        };
        true
    });
}
