use crate::events::{Bar, MarketEvent, Trade};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RAS1Thresholds {
    /// BASE BAR %
    pub base_bar_pct: Option<f64>,

    /// BASE BAR END %
    pub base_bar_end_pct: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RAS1Config {
    pub symbol: String,
    pub trading_mode: TradingMode,
    pub dollar_amount: u32,
    pub base_bar_opp: bool,

    pub thresholds: RAS1Thresholds
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
    current_position: i32,
}

impl RAS1Strategy {
    pub fn new(config: RAS1Config) -> Self {
        Self {
            config,
            state: StrategyState::WaitingForBase,
            prev_bar: None,
            prev_bar_type: None,
            current_position: 0
        }
    }

    fn should_reverse_on_initial_bar(&self, bar: &Bar) -> Option<f64> {
        // println!("Checking reversal on bar: {:?}, {}, {:?}", bar, self.current_position, self.state);

        let base_bar_pct = self.config.thresholds.base_bar_pct?;
        let base_bar_end_pct = self.config.thresholds.base_bar_end_pct?;

        let StrategyState::Active { base_bar_state } = &self.state else {
            return None;
        };

        if self.current_position == 0 {
            return None;
        }

        let close = base_bar_state.bar.close;

        if self.current_position > 0 {

            let low = base_bar_state.bar.low;
            let basebar_pct_target =  close - (close * base_bar_pct/100.0);
            let basebarend_pct_target = low - (low * base_bar_end_pct/100.0);

            let target_price_reversal = f64::min(basebar_pct_target, basebarend_pct_target);

            if target_price_reversal >= bar.low  {
                println!("Both {} {} are >= then current bar low of {}", basebar_pct_target, basebarend_pct_target, bar.low);
                return Some(target_price_reversal)
            }

        } else if self.current_position < 0 {

            let high = base_bar_state.bar.high;
            let basebar_pct_target =  close + (close * base_bar_pct/100.0);
            let basebarend_pct_target = high + (high * base_bar_end_pct/100.0);

            let target_price_reversal = f64::max(basebar_pct_target, basebarend_pct_target);

            if target_price_reversal <= bar.high {
                println!("Both {} {} are <= then current bar high of {}", basebar_pct_target, basebarend_pct_target, bar.low);
                return Some(target_price_reversal)
            }
        }

        None

    }

    fn reverse_trade(&self, trigger_price: f64) -> Signal {

        let mut size = (self.config.dollar_amount as f64 / trigger_price) as u32;
        size += self.current_position.unsigned_abs();

        let signal_type = if self.current_position > 0 { SignalType::Sell} else {SignalType::Buy};

        Signal {
            // timestamp: Utc::now(),
            signal_type,
            signal_trigger_price: trigger_price,
            size,
            reason: String::from("reverse trade"),
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

    fn update_position(&mut self, sig: &Signal) {
        match sig.signal_type {
            SignalType::Buy => { self.current_position += sig.size as i32;},
            SignalType::Sell => {self.current_position -= sig.size as i32;}
        }
    }

    fn handle_trade(&mut self, trade: &Trade) -> Option<Signal> {
        match &self.state {
            StrategyState::WaitingForInitial { base_bar_state } => {
                let sig =  self.emit_first_trade(&base_bar_state.bar_type, trade.price);

                self.update_position(&sig);

                Some(sig)
            },
            _ => None
        }
    }

    fn handle_bar(&mut self, bar: &Bar) -> Option<Signal>{
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

            StrategyState::WaitingForInitial { base_bar_state} => {
                self.state = StrategyState::Active {
                    base_bar_state: base_bar_state.clone(),
                };

                if let Some(reversal_price) = self.should_reverse_on_initial_bar(bar) {
                    return Some(self.reverse_trade(reversal_price));
                }

                None
            },

            StrategyState::Active { .. } => None,
        };

        self.update_prev_bar(bar);
        signal

    }
}

impl Strategy for RAS1Strategy {
    fn on_event(&mut self, event: &MarketEvent) -> Option<Signal> {
        match event {
            MarketEvent::Bar(bar) => self.handle_bar(bar),
            MarketEvent::Trade(trade) => self.handle_trade(trade),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde::Deserialize;
    use std::fs;

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


    fn trade_at_open(bar: &Bar) -> Trade {
        Trade {
            timestamp: String::new(),
            price: bar.open,
            size: 1,
        }
    }

    #[test]
    fn test_scenarios() -> std::io::Result<()> {

        let mut yaml_files: Vec<_> = fs::read_dir("test_scenarios")?
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| matches!(path.extension().and_then(|s| s.to_str()), Some("yaml" | "yml")))
            .collect();

        yaml_files.sort();

        for file_path in yaml_files {
            println!("\nRunning scenarios from: {}", file_path.to_str().unwrap());
            let file_content = fs::read_to_string(file_path)?;
            let scenario_file: ScenarioFile =
                serde_yaml::from_str(&file_content).expect("failed to parse scenario YAML");

            for scenario in scenario_file.scenarios {
                println!("Running scenario: {}", scenario.desc);

                let mut strategy = RAS1Strategy::new(scenario.config.clone());
                let mut signals = vec![];

                for bar_array in scenario.bars {
                    let bar = bar_array.to_bar();

                    if let Some(sig) = strategy.on_event(&MarketEvent::Trade(trade_at_open(&bar))) {
                        signals.push(sig);
                    }

                    if let Some(sig) = strategy.on_event(&MarketEvent::Bar(bar)) {
                        signals.push(sig);
                    }
                }

                assert_eq!(signals, scenario.expected_signals);
            }
        }

        Ok(())
    }
}
