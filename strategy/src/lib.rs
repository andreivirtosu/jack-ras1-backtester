pub mod events;
pub mod strategy;
pub mod threshold_strategy;

pub use events::MarketEvent;
pub use strategy::Signal;
pub use strategy::Strategy;
pub use strategy::hello;
pub use threshold_strategy::ThresholdStrategy;

pub fn run() {
    println!("Strategy running...");
}
