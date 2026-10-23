pub fn post_fuel(label: &str) {
    println!("{label}: {:?}", cambc::fuel_remaining());
}
