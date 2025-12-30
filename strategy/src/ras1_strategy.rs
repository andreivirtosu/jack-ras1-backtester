use crate::events::{Bar, MarketEvent};
use crate::strategy::{Signal, SignalType, Strategy};
use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag="type", content="value")]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

        let size = (self.config.dollar_amount as f64 / bar.open) as u32;
        Signal {
            // timestamp: Utc::now(),
            signal_type,
            signal_trigger_price: bar.open,
            size,
            reason: String::from("first trade"),
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
    use serde::Deserialize;
    use std::fs;
    use pretty_assertions::assert_eq;

    #[derive(Deserialize)]
    struct Scenario {
        config: RAS1Config,
        bars: Vec<Bar>,
        expected_signals: Vec<Signal>,
    }

    #[test]
    fn test_ras1_scenario() {
        let file_content = fs::read_to_string("test_scenarios/first_trade_occurs_after_base_bar.json").unwrap();
        let scenario: Scenario = serde_json::from_str(&file_content).unwrap();

        let mut strategy = RAS1Strategy::new(scenario.config);

        let mut signals = vec![];
        for bar in scenario.bars {
            if let Some(sig) = strategy.on_event(&MarketEvent::Bar(bar)) {
                signals.push(sig);
            }
        }

        assert_eq!(signals, scenario.expected_signals);
    }
}
