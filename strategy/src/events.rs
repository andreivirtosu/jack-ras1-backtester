#[derive(Debug, PartialEq)]
pub enum MarketEvent {
    Tick {
        timestamp: String,
        price: f64,
        size: u32,
    },
    Bar {
        timestamp: String,
        open: f64,
        high: f64,
        low: f64,
        close: f64,
        volume: u32,
    },
}
