use pm1_sdk_3::pm1::*;
use serial_port::{Port, SerialPort};

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

    for chassis in chassis {
        for message in chassis {}
    }
}
