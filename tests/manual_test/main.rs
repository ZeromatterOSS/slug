extern crate hello_rust_lib;

fn main() {
    let greeting = hello_rust_lib::greet("Kuro");
    println!("{}", greeting);
}
