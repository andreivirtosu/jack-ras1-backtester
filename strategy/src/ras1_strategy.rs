use crate::events::{Bar, MarketEvent, Trade};
use crate::strategy::{Signal, SignalType, Strategy};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::{f64, io};

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

    /// NON BASE BAR END %
    pub non_base_bar_end_pct: Option<f64>,

    /// NON BASE BAR MIN %
    pub non_base_bar_min_pct: Option<f64>,

    /// NON BASE BAR MAX %
    pub non_base_bar_max_pct: Option<f64>,
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

    signals: Vec<Signal>, 
    highest_close_bar: Option<Bar>,
    lowest_close_bar: Option<Bar>

}

impl RAS1Strategy {
    pub fn new(config: RAS1Config) -> Self {
        Self {
            config,
            state: StrategyState::WaitingForBase,
            prev_bar: None,
            prev_bar_type: None,
            current_position: 0,
            signals: vec![],
            highest_close_bar: None,
            lowest_close_bar: None
        }
    }

    fn base_bar_pct(&self) -> Option<f64> {
        let pct = self.config.thresholds.base_bar_pct?;

        let StrategyState::Active { base_bar_state } = &self.state else {
            return None;
        };

        let close = base_bar_state.bar.close;

        if self.current_position > 0 {
            return Some ( close - (close * pct/100.0));
        }
        else if self.current_position < 0 {
            return Some( close + (close * pct/100.0));
        }
        None
    }

    fn base_bar_end_pct(&self) -> Option<f64> {
        let pct = self.config.thresholds.base_bar_end_pct?;

        let StrategyState::Active { base_bar_state } = &self.state else {
            return None;
        };

        if self.current_position > 0 {
            let low = base_bar_state.bar.low;
            return Some ( low - (low * pct/100.0));

        }
        else if self.current_position < 0 {
            let high = base_bar_state.bar.high;
            return Some ( high + (high * pct/100.0));
        }

        None
    }


    fn non_base_bar_min(&self, bar: &Bar) -> Option<f64> {
        let pct = self.config.thresholds.non_base_bar_min_pct?;

        if self.current_position > 0 {
            return Some(bar.close - (bar.close * pct/100.0));
        }
        else if self.current_position < 0{
            return Some( bar.close + (bar.close * pct/100.0));
        }

        None
    }

    fn non_base_bar_max(&self) -> Option<f64> {
        let pct = self.config.thresholds.non_base_bar_max_pct?;

        if self.current_position > 0 {
            let close = self.highest_close_bar.as_ref()?.close;
            return Some(close - (close * pct/100.0));
        }
        else if self.current_position < 0{
            let close = self.lowest_close_bar.as_ref()?.close;
            return Some( close + (close * pct/100.0));
        }

        None
    }


    fn non_base_bar_end(&self, bar: &Bar) -> Option<f64> {
        let pct = self.config.thresholds.non_base_bar_end_pct?;

        if self.current_position > 0 {
            return Some(bar.low - (bar.low * pct/100.0));
        }
        else if self.current_position < 0{
            return Some( bar.high + (bar.high * pct/100.0));
        }

        None
    }

    fn reverse_target_hit(&self, targets:Vec<f64>, bar: &Bar ) ->Option<f64> {
        if self.current_position > 0 {
            let target_price_short = targets
                .into_iter()
                .fold(f64::INFINITY, f64::min);

            println!("got target_price_short={}", target_price_short);
            if target_price_short >= bar.low  {
                return Some(target_price_short)
            }

        } else if self.current_position < 0 {

            let target_price_long = targets
                .into_iter()
                .fold(f64::MIN, f64::max);

            println!("got target_price_long={}", target_price_long);
            if target_price_long <= bar.high {
                return Some(target_price_long)
            }
        }

        None
    }

    fn should_reverse_on_initial_bar(&self, bar: &Bar) -> Option<f64> {

        let basebar =  self.base_bar_pct()?;
        let basebarend = self.base_bar_end_pct()?;

        let targets = vec![basebar, basebarend];
        println!("targets: {:?}", targets);

        self.reverse_target_hit(targets, bar)
    }

    fn should_reverse_after_initial_bar(&mut self, bar: &Bar) -> Option<f64>{

        println!("should_reverse_after_initial_bar");
        let prev_bar = self.prev_bar.as_ref()?;

        let nonbasebar_end =  self.non_base_bar_end(prev_bar)?;
        let nonbasebar_min =  self.non_base_bar_min(prev_bar)?;
        let basebar =  self.base_bar_pct()?;
        let basebarend = self.base_bar_end_pct()?;

        let targets = vec![basebar, basebarend,nonbasebar_end, nonbasebar_min];
        println!("targets scenario A: {:?}", targets);

        match self.reverse_target_hit(targets, bar) {
            Some(price) => Some(price),
            _ => {
                println!("highest_close_bar: {:?}", self.highest_close_bar.as_ref()?);
                let nonbase_bar_end = if self.current_position>0 {
                    self.non_base_bar_end(self.highest_close_bar.as_ref().unwrap())?
                } else {
                    self.non_base_bar_end(self.lowest_close_bar.as_ref().unwrap())?
                };
                let nonbasebar_max = self.non_base_bar_max()?;

                let targets = vec![nonbase_bar_end, nonbasebar_max];
                println!("targets scenario B: {:?}", targets);
                self.reverse_target_hit(targets, bar)
            }
        }
    }

    fn reverse_trade(&self, trigger_price: f64, reason: String) -> Signal {

        let mut size = (self.config.dollar_amount as f64 / trigger_price) as u32;
        size += self.current_position.unsigned_abs();

        let signal_type = if self.current_position > 0 { SignalType::Sell} else {SignalType::Buy};

        Signal {
            // timestamp: Utc::now(),
            signal_type,
            signal_trigger_price: trigger_price,
            size,
            reason,
            // reason: String::from("reverse trade"),
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

        // keep track of the bar with highest/lowest close
        if self.highest_close_bar.as_ref().is_some_and(|b| b.high <= bar.high ) {
            self.highest_close_bar = Some(bar.clone());
        }

        if self.lowest_close_bar.as_ref().is_some_and(|b| b.low >= bar.low ) {
            self.lowest_close_bar = Some(bar.clone());
        }

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

                self.highest_close_bar = Some(bar.clone());
                self.lowest_close_bar = Some(bar.clone());

                if let Some(reversal_price) = self.should_reverse_on_initial_bar(bar) {
                    return Some(self.reverse_trade(reversal_price, String::from("reverse [on initial-bar]")));
                }

                None
            },

            StrategyState::Active { .. } => {

                if self.signals.len() == 1  && let Some(reversal_price) = self.should_reverse_after_initial_bar(bar) {
                    return Some(self.reverse_trade(reversal_price, String::from("reverse [after initial-bar]")));
                }

                None
            }
        };

        self.update_prev_bar(bar);
        signal

    }
}

impl Strategy for RAS1Strategy {
    fn on_event(&mut self, event: &MarketEvent) -> Option<Signal> {
        let sig = match event {
            MarketEvent::Bar(bar) => self.handle_bar(bar),
            MarketEvent::Trade(trade) => self.handle_trade(trade),
        };

        if let Some(signal) = &sig {
            self.signals.push(signal.clone());
        }

        sig
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

        let filter_scenario = std::env::var("SCENARIO").ok();

        for file_path in yaml_files {
            println!("\nRunning scenarios from: {}", file_path.to_str().unwrap());
            let file_content = fs::read_to_string(file_path)?;
            let scenario_file: ScenarioFile =
                serde_yaml::from_str(&file_content).expect("failed to parse scenario YAML");

            for scenario in scenario_file.scenarios {

                if let Some(f) = &filter_scenario && !scenario.desc.contains(f){
                    println!("skipping scenario: {}", scenario.desc);
                    continue;
                }

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
