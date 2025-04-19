use apex_core::prelude::*;

fn main() {
    println!("Size of Order: {}", size_of::<Order>());
    println!("Alignment of Order: {}", align_of::<Order>());
}
