use pm1_sdk::find_pm1;

fn main() {
    if let Some(chassis) = find_pm1!() {
        for event in chassis {
            println!("{:?}", event);
        }
    }
}
