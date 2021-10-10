use driver::SupervisorForSingle;
use pm1::PM1;
use serial_port::{Port, SerialPort};
use std::time::Duration;

pub mod pm1;

pub struct PM1Supervisor(Box<Option<PM1>>);

impl PM1Supervisor {
    pub fn new() -> Self {
        Self(Box::new(None))
    }
}

impl SupervisorForSingle<String, PM1> for PM1Supervisor {
    fn context<'a>(&'a mut self) -> &'a mut Box<Option<PM1>> {
        &mut self.0
    }

    fn open_timeout() -> Duration {
        const TIMEOUT: Duration = Duration::from_secs(2);
        TIMEOUT
    }

    fn keys() -> Vec<String> {
        Port::list()
            .into_iter()
            .map(|name| {
                if cfg!(target_os = "windows") {
                    name.rmatch_indices("COM")
                        .next()
                        .map(|m| &name.as_str()[m.0..name.len() - 1])
                        .unwrap()
                        .into()
                } else {
                    name
                }
            })
            .collect()
    }
}
