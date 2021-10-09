﻿use driver::{Driver, Module};
use pm1_sdk::PM1Threads;

fn main() {
    if let Some(mut chassis) = PM1Threads::open_all(1).into_iter().next() {
        chassis.wait(|_, _, event| println!("{:?}", event));
    }
}
