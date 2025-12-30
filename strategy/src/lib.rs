pub mod strategy;
pub mod events;

pub use strategy::hello;

pub fn run() {
    println!("Strategy running...");
}
