use driver::Module;
use pm1_sdk::PM1Threads;

fn main() {
    if let Some(chassis) = PM1Threads::open_all(1).into_iter().next() {
        for event in chassis {
            println!("{:?}", event);
        }
    }
}
