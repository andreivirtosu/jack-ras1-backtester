use crate::events::{Bar, MarketEvent};
use crate::strategy::{Signal, SignalType, Strategy};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::io;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
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
        self.prev_bar_type = self.bar_type(bar);
        self.prev_bar = Some(bar.clone());
    }

    fn emit_first_trade(&self, base_bar_type: &BarType, bar_open: f64) -> Signal {
        let mut signal_type = match base_bar_type {
            BarType::UpBar => SignalType::Buy,
            BarType::DownBar => SignalType::Sell,
        };

        if self.config.base_bar_opp {
            signal_type = signal_type.reverse();
        }

        let size = (self.config.dollar_amount as f64 / bar_open) as u32;
        Signal {
            // timestamp: Utc::now(),
            signal_type,
            signal_trigger_price: bar_open,
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
                let sig = self.emit_first_trade(&base_bar_state.bar_type, bar.open);
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
    use pretty_assertions::assert_eq;
    use serde::Deserialize;
    use std::fs;
    use std::path::Path;

    #[derive(Debug, Deserialize)]
    struct Scenario {
        desc: String,
        config: RAS1Config,
        expected_signals: Vec<Signal>,
        bars: Vec<BarArray>,
    }

    #[derive(Debug, Deserialize)]
    struct ScenarioFile {
        scenarios: Vec<Scenario>,
    }

    #[derive(Debug, Deserialize)]
    struct BarArray {
        o: f64,
        h: f64,
        l: f64,
        c: f64,
        vol: u64,
        is_base_bar: bool,
    }

    impl BarArray {
        fn to_bar(&self) -> Bar {
            Bar {
                timestamp: "".to_string(),
                open: self.o,
                high: self.h,
                low: self.l,
                close: self.c,
                volume: self.vol,
                is_base_bar: self.is_base_bar,
            }
        }
    }

    #[test]
    fn test_ras1_scenarios() -> std::io::Result<()> {
        let file_path = Path::new("test_scenarios/first_trade.yaml");
        let file_content = fs::read_to_string(file_path)?;
        let scenario_file: ScenarioFile =
            serde_yaml::from_str(&file_content).expect("failed to parse scenario YAML");

        for scenario in scenario_file.scenarios {
            println!("Running scenario: {}", scenario.desc);

            let mut strategy = RAS1Strategy::new(scenario.config.clone());
            let mut signals = vec![];

            for bar_array in scenario.bars {
                let bar = bar_array.to_bar();
                if let Some(sig) = strategy.on_event(&MarketEvent::Bar(bar)) {
                    signals.push(sig);
                }
            }

            assert_eq!(signals, scenario.expected_signals);
        }

        Ok(())
    }
}
