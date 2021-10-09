use driver::Module;
use pm1::PM1;
use serial_port::{Port, SerialPort};

pub mod pm1;

pub struct PM1Threads;

impl Module<Port, PM1> for PM1Threads {
    fn keys() -> Vec<Port> {
        Port::list()
            .iter()
            .filter_map(|name| {
                let path: String = if cfg!(target_os = "windows") {
                    name.rmatch_indices("COM")
                        .next()
                        .map(|m| &name.as_str()[m.0..name.len() - 1])
                        .unwrap()
                        .into()
                } else {
                    name.clone()
                };

                Port::open(path.as_str(), 115200, 200).ok()
            })
            .collect()
    }
}
