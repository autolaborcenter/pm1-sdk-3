fn main() {
    if let Some(chassis) = pm1_sdk_3::PM1Threads::open_all() {
        for event in chassis {
            println!("{:?}", event);
        }
    }
}
