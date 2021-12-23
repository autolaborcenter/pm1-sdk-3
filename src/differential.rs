use std::time::{Duration, Instant};

pub(crate) struct Differential {
    initialized: bool,
    memory: (i32, i32),

    last_which: u8,
    last_value: i32,
    deadline: Instant,
}

impl Differential {
    const TIMEOUT: Duration = Duration::from_millis(100);
    const L: u8 = 0;
    // const R: u8 = 1;
    const N: u8 = 2;

    #[inline]
    pub fn new() -> Self {
        Self {
            initialized: false,
            memory: (0, 0),

            last_which: Self::N,
            last_value: 0,
            deadline: Instant::now(),
        }
    }

    pub fn update(&mut self, time: Instant, which: u8, value: i32) -> Option<(i32, i32)> {
        if which > 1 || which == self.last_which {
            return None;
        }

        if self.last_which == Self::N || time > self.deadline {
            self.last_which = which;
            self.last_value = value;
            self.deadline = time + Self::TIMEOUT;
            None
        } else {
            let value = std::mem::replace(
                &mut self.memory,
                if which == Self::L {
                    (value, self.last_value)
                } else {
                    (self.last_value, value)
                },
            );
            self.last_which = Self::N;
            if self.initialized {
                Some((self.memory.0 - value.0, self.memory.1 - value.1))
            } else {
                self.initialized = true;
                None
            }
        }
    }
}
