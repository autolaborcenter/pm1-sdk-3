use pm1_sdk_3::pm1::*;
use serial_port::{Port, SerialPort};
use std::{
    thread,
    time::{Duration, Instant},
};

fn main() {
    let mut senders = Vec::<PM1QuerySender>::new();
    let mut chassis = Vec::<PM1>::new();
    Port::list()
        .iter()
        .filter_map(|s| {
            s.rmatch_indices("COM")
                .next()
                .map(|m| &s.as_str()[m.0..s.len() - 1])
                .map(|p| Port::open(p, 115200).ok())
                .flatten()
                .map(|p| pm1(p))
        })
        .for_each(|(sender, pm1)| {
            senders.push(sender);
            chassis.push(pm1);
        });

    let _ = thread::spawn(move || {
        let mut time = Instant::now();
        while senders.iter_mut().any(|s| s.send()) {
            let now = Instant::now();
            while time <= now {
                time += Duration::from_millis(40);
            }
            thread::sleep(time - now);
        }
    });

    for chassis in chassis {
        for message in chassis {
            println!("{:?}", message);
        }
    }
}
