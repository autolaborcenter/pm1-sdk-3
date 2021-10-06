use pm1_sdk_3::find_pm1;

fn main() {
    if let Some(chassis) = find_pm1!() {
        for event in chassis {
            println!("{:?}", event);
        }
    }
}
