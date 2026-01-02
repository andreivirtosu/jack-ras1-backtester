pub mod events;
pub mod ras1_strategy;
pub mod strategy;

pub use events::MarketEvent;
pub use strategy::Signal;
pub use strategy::Strategy;
pub use strategy::hello;

pub fn run() {
    println!("Strategy running...");
}
