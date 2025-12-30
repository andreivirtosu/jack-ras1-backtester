// use crate::events::MarketEvent;
// use crate::strategy::{Signal, SignalType, Strategy};
// use chrono::Utc;
//
// pub struct ThresholdStrategy {
//     pub buy_above: f64,
//     pub sell_below: f64,
// }
//
// impl Strategy for ThresholdStrategy {
//     fn on_event(&mut self, event: &MarketEvent) -> Option<Signal> {
//         let price = match event {
//             MarketEvent::Tick { price, .. } => *price,
//             MarketEvent::Bar { close, .. } => *close,
//         };
//
//         if price > self.buy_above {
//             Some(Signal {
//                 signal_type: SignalType::Buy,
//                 signal_trigger_price: price,
//                 timestamp: Utc::now(),
//                 size: 1,
//                 reason: String::from("Above price condition"),
//             })
//         } else if price < self.sell_below {
//             Some(Signal {
//                 signal_type: SignalType::Sell,
//                 signal_trigger_price: price,
//                 timestamp: Utc::now(),
//                 size: 1,
//                 reason: String::from("Below price condition"),
//             })
//         } else {
//             None
//         }
//     }
// }
//
// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::events::MarketEvent;
//     use pretty_assertions::assert_eq;
//
//     fn assert_signals_match(actual: &[Signal], expected: &[Signal]) {
//         assert_eq!(
//             actual.len(),
//             expected.len(),
//             "Number of signals emitted does not match expectations"
//         );
//
//         for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
//             // Compare each field except timestamp
//             assert_eq!(
//                 a.signal_type, e.signal_type,
//                 "Signal type mismatch at index {}",
//                 i
//             );
//             assert_eq!(
//                 a.signal_trigger_price, e.signal_trigger_price,
//                 "Signal trigger price mismatch at index {}",
//                 i
//             );
//             assert_eq!(a.size, e.size, "Signal size mismatch at index {}", i);
//             assert_eq!(a.reason, e.reason, "Signal reason mismatch at index {}", i);
//         }
//     }
//
//     #[test]
//     fn test_threshold_strategy_on_bars() {
//         let mut strategy = ThresholdStrategy {
//             buy_above: 150.0,
//             sell_below: 100.0,
//         };
//
//         let events = [
//             MarketEvent::Bar {
//                 timestamp: "2025-12-30 09:30".to_string(),
//                 open: 100.0,
//                 high: 105.0,
//                 low: 101.0,
//                 close: 102.0,
//                 volume: 1000,
//                 is_base_bar: false,
//             },
//             MarketEvent::Bar {
//                 timestamp: "2025-12-30 09:31".to_string(),
//                 open: 102.0,
//                 high: 106.0,
//                 low: 131.0,
//                 close: 154.0,
//                 volume: 1200,
//                 is_base_bar: false,
//             },
//             MarketEvent::Bar {
//                 timestamp: "2025-12-30 09:32".to_string(),
//                 open: 104.0,
//                 high: 107.0,
//                 low: 98.0,
//                 close: 99.0,
//                 volume: 900,
//                 is_base_bar: false,
//             },
//         ];
//
//         let expected_signals = [
//             Signal {
//                 signal_type: SignalType::Buy,
//                 signal_trigger_price: 154.0,
//                 timestamp: Utc::now(),
//                 size: 1,
//                 reason: String::from("Above price condition"),
//             },
//             Signal {
//                 signal_type: SignalType::Sell,
//                 signal_trigger_price: 99.0,
//                 timestamp: Utc::now(),
//                 size: 1,
//                 reason: String::from("Below price condition"),
//             },
//         ];
//
//         let emitted_signals: Vec<_> = events
//             .iter()
//             .filter_map(|event| strategy.on_event(event))
//             .collect();
//
//         assert_signals_match(&emitted_signals, &expected_signals);
//     }
// }
