use crate::events::{Bar, MarketEvent};
use crate::strategy::{Signal, SignalType, Strategy};

#[derive(Debug, Clone)]
pub enum TradingMode {
    Intraday {
        bar_minutes: u32,
        base_bar_start_time: String,
    },

    Daily(u32),
}

#[derive(Debug, Clone)]
pub struct RAS1Thresholds {
    /// BASE BAR %
    pub base_bar_pct: f64,
}

#[derive(Debug)]
struct BarData {
    open: f64,
    high: f64,
    low: f64,
    close: f64,
}

#[derive(Debug, Clone)]
pub struct RAS1Config {
    pub symbol: String,
    pub trading_mode: TradingMode,
    pub dollar_amount: u32,
    pub base_bar_opp: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BarType {
    UpBar,
    DownBar,
}

#[derive(Debug)]
enum StrategyState {
    WaitingForBase,
    WaitingForInitial { base_bar_state: BaseBarState },
    Active { base_bar_state: BaseBarState },
}

#[derive(Debug, Clone)]
struct BaseBarState {
    bar: Bar,
    bar_type: BarType,
}

#[derive(Debug)]
pub struct RAS1Strategy {
    config: RAS1Config,
    state: StrategyState,
    prev_bar: Option<Bar>,
    prev_bar_type: Option<BarType>,
}

impl RAS1Strategy {
    pub fn new(config: RAS1Config) -> Self {
        Self {
            config,
            state: StrategyState::WaitingForBase,
            prev_bar: None,
            prev_bar_type: None,
        }
    }
}

impl RAS1Strategy {
    fn bar_type(&mut self, bar: &Bar) -> Option<BarType> {
        if let Some(prev_bar) = &self.prev_bar {
            let bar_type = if bar.close > prev_bar.close {
                BarType::UpBar
            } else if bar.close < prev_bar.close {
                BarType::DownBar
            } else {
                self.prev_bar_type.clone()?
            };
            Some(bar_type)
        } else {
            None
        }
    }

    fn update_prev_bar(&mut self, bar: &Bar) {
        self.prev_bar = Some(bar.clone());
        self.prev_bar_type = self.bar_type(bar);
    }

    fn emit_first_trade(&self, base_bar_type: &BarType, bar: &Bar) -> Signal {
        let signal_type = match base_bar_type {
            BarType::UpBar => SignalType::Buy,
            BarType::DownBar => SignalType::Sell,
        };
        Signal {
            signal_type,
            signal_trigger_price: bar.open,
        }
    }
}

impl Strategy for RAS1Strategy {
    fn on_event(&mut self, event: &MarketEvent) -> Option<Signal> {
        let bar = match event {
            MarketEvent::Bar(bar) => bar,
            _ => return None,
        };

        let signal = match &self.state {
            StrategyState::WaitingForBase => {
                if bar.is_base_bar {
                    let bar_type = self.bar_type(bar)?;
                    let base_bar_state = BaseBarState {
                        bar: bar.clone(),
                        bar_type,
                    };
                    self.state = StrategyState::WaitingForInitial { base_bar_state };
                }
                None
            }

            StrategyState::WaitingForInitial { base_bar_state } => {
                let sig = self.emit_first_trade(&base_bar_state.bar_type, bar);
                self.state = StrategyState::Active {
                    base_bar_state: base_bar_state.clone(),
                };
                Some(sig)
            }

            StrategyState::Active { .. } => None,
        };

        self.update_prev_bar(bar);
        signal
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::MarketEvent;
    use crate::strategy::{SignalType, Strategy};

    #[test]
    fn ras1_first_trade() {
        let mut strategy = RAS1Strategy::new(RAS1Config {
            symbol: String::from("IBM"),
            //trading_mode: TradingMode::Intraday { bar_minutes: 1, base_bar_start_time: String::from("")},
            trading_mode: TradingMode::Daily(1),
            dollar_amount: 1000,
            base_bar_opp: false,
        });

        if let TradingMode::Daily(days) = strategy.config.trading_mode {
            println!("days: {}", days);
        }

        let bars = [
            MarketEvent::Bar(Bar {
                timestamp: String::from(""),
                open: 110.0,
                high: 120.0,
                low: 100.0,
                close: 105.0,
                volume: 100_000,
                is_base_bar: false,
            }),
            MarketEvent::Bar(Bar {
                timestamp: String::from(""),
                open: 110.0,
                high: 130.0,
                low: 110.0,
                close: 115.0,
                volume: 100_000,
                is_base_bar: true,
            }),
            MarketEvent::Bar(Bar {
                timestamp: String::from(""),
                open: 120.0,
                high: 130.0,
                low: 100.0,
                close: 124.0,
                volume: 100_000,
                is_base_bar: false,
            }),
        ];

        let mut signals = vec![];
        for bar in bars {
            if let Some(s) = strategy.on_event(&bar) {
                signals.push(s);
            }
        }

        assert_eq!(signals.len(), 1);

        println!("{:?}", signals);
    }
}
