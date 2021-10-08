use nalgebra::{ArrayStorage, Complex, Isometry2, SVector, Translation, Unit, Vector2};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Odometry {
    s: f32,
    a: f32,
    pose: Isometry2<f32>,
}

impl Odometry {
    pub const ZERO: Self = Self {
        s: 0.0,
        a: 0.0,
        pose: Isometry2 {
            translation: Translation {
                vector: SVector::from_array_storage(ArrayStorage([[0.0, 0.0]])),
            },
            rotation: Unit::new_unchecked(Complex { re: 1.0, im: 0.0 }),
        },
    };

    pub fn from_delta(s: f32, a: f32) -> Self {
        let theta = a;
        let a = a.abs();

        Self {
            s: s.abs(),
            a,
            pose: Isometry2::new(
                if a < f32::EPSILON {
                    Vector2::new(s, 0.0)
                } else {
                    Vector2::new(theta.sin(), 1.0 - theta.cos()) * (s / theta)
                },
                theta,
            ),
        }
    }
}

impl std::ops::Add for Odometry {
    type Output = Odometry;

    fn add(self, rhs: Self) -> Self::Output {
        Self::Output {
            s: self.s + rhs.s,
            a: self.a + rhs.a,
            pose: self.pose * rhs.pose,
        }
    }
}

pub struct Differential {
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
