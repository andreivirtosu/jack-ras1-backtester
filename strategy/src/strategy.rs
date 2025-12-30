use chrono::{DateTime, Utc};
use crate::events::MarketEvent;
use serde::{Deserialize, Serialize};

pub fn hello() {
    println!("Hello from strategy lib");
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum SignalType {
    Buy,
    Sell,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Signal {
    pub signal_type: SignalType,
    // pub timestamp: DateTime<Utc>,
    pub signal_trigger_price: f64,
    pub size: u32,
    pub reason: String
}

pub trait Strategy {
    fn on_event(&mut self, event:&MarketEvent) ->Option<Signal>;
}
