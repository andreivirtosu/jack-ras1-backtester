#[derive(Debug, Clone)]
pub struct Bar {
    pub timestamp: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u32,
    pub is_base_bar: bool,
}

#[derive(Debug, Clone)]
pub struct Trade {
    pub timestamp: String,
    pub price: f64,
    pub size: u32,
}

pub enum MarketEvent {
    Trade(Trade),
    Bar(Bar),
}
