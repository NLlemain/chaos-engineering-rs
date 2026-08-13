use crate::{FaultImpact, MarketFaultPlan};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Bid,
    Ask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BookAction {
    Upsert,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventKind {
    BookUpdate {
        side: Side,
        price_ticks: i64,
        quantity: u64,
        action: BookAction,
    },
    Trade {
        price_ticks: i64,
        quantity: u64,
    },
    OrderAck {
        client_order_id: String,
        accepted: bool,
        latency_ns: u64,
    },
    CancelAck {
        client_order_id: String,
        latency_ns: u64,
    },
    Heartbeat,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketEvent {
    pub sequence: u64,
    pub timestamp_ns: u64,
    pub venue: String,
    pub symbol: String,
    #[serde(flatten)]
    pub kind: EventKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvariantBudget {
    #[serde(default)]
    pub max_sequence_gaps: usize,
    #[serde(default)]
    pub max_duplicates: usize,
    #[serde(default)]
    pub max_out_of_order: usize,
    #[serde(default)]
    pub max_stale_timestamps: usize,
    #[serde(default)]
    pub max_crossed_books: usize,
    #[serde(default)]
    pub max_p99_ack_latency_ns: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayReport {
    pub events: usize,
    pub sequence_gaps: usize,
    pub duplicate_sequences: usize,
    pub out_of_order_sequences: usize,
    pub stale_timestamps: usize,
    pub crossed_books: usize,
    pub rejected_orders: usize,
    pub ack_latency_p50_ns: u64,
    pub ack_latency_p99_ns: u64,
    pub final_state_digest: String,
    pub passed: bool,
    pub violations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentEvidence {
    pub seed: u64,
    pub baseline: ReplayReport,
    pub chaos: ReplayReport,
    pub restored: ReplayReport,
    pub impact: FaultImpact,
    pub disruption_observed: bool,
    pub restoration_verified: bool,
}

#[derive(Default, Serialize)]
struct Book {
    bids: BTreeMap<i64, u64>,
    asks: BTreeMap<i64, u64>,
}

pub fn replay(events: &[MarketEvent], budget: &InvariantBudget) -> ReplayReport {
    let mut last_sequence = HashMap::<(&str, &str), u64>::new();
    let mut last_timestamp = HashMap::<(&str, &str), u64>::new();
    let mut books = HashMap::<(&str, &str), Book>::new();
    let mut sequence_gaps = 0usize;
    let mut duplicate_sequences = 0usize;
    let mut out_of_order_sequences = 0usize;
    let mut stale_timestamps = 0usize;
    let mut crossed_books = 0usize;
    let mut rejected_orders = 0usize;
    let mut ack_latencies = Vec::new();

    for event in events {
        let stream = (event.venue.as_str(), event.symbol.as_str());
        if let Some(previous) = last_sequence.insert(stream, event.sequence) {
            if event.sequence == previous {
                duplicate_sequences += 1;
            } else if event.sequence < previous {
                out_of_order_sequences += 1;
            } else if event.sequence > previous.saturating_add(1) {
                sequence_gaps += 1;
            }
        }
        if let Some(previous) = last_timestamp.insert(stream, event.timestamp_ns) {
            if event.timestamp_ns < previous {
                stale_timestamps += 1;
            }
        }

        match &event.kind {
            EventKind::BookUpdate {
                side,
                price_ticks,
                quantity,
                action,
            } => {
                let book = books.entry(stream).or_default();
                let levels = match side {
                    Side::Bid => &mut book.bids,
                    Side::Ask => &mut book.asks,
                };
                match action {
                    BookAction::Delete => {
                        levels.remove(price_ticks);
                    }
                    BookAction::Upsert => {
                        levels.insert(*price_ticks, *quantity);
                    }
                }
                if book
                    .bids
                    .last_key_value()
                    .zip(book.asks.first_key_value())
                    .is_some_and(|((bid, _), (ask, _))| bid >= ask)
                {
                    crossed_books += 1;
                }
            }
            EventKind::OrderAck {
                accepted,
                latency_ns,
                ..
            } => {
                if !accepted {
                    rejected_orders += 1;
                }
                ack_latencies.push(*latency_ns);
            }
            EventKind::CancelAck { latency_ns, .. } => ack_latencies.push(*latency_ns),
            EventKind::Trade { .. } | EventKind::Heartbeat => {}
        }
    }

    ack_latencies.sort_unstable();
    let ack_latency_p50_ns = percentile(&ack_latencies, 50);
    let ack_latency_p99_ns = percentile(&ack_latencies, 99);
    let final_state_digest = state_digest(&books);
    let mut violations = Vec::new();
    budget_violation(
        &mut violations,
        "sequence gaps",
        sequence_gaps,
        budget.max_sequence_gaps,
    );
    budget_violation(
        &mut violations,
        "duplicate sequences",
        duplicate_sequences,
        budget.max_duplicates,
    );
    budget_violation(
        &mut violations,
        "out-of-order sequences",
        out_of_order_sequences,
        budget.max_out_of_order,
    );
    budget_violation(
        &mut violations,
        "stale timestamps",
        stale_timestamps,
        budget.max_stale_timestamps,
    );
    budget_violation(
        &mut violations,
        "crossed books",
        crossed_books,
        budget.max_crossed_books,
    );
    if budget
        .max_p99_ack_latency_ns
        .is_some_and(|limit| ack_latency_p99_ns > limit)
    {
        violations.push(format!(
            "p99 acknowledgement latency {}ns exceeded {}ns",
            ack_latency_p99_ns,
            budget.max_p99_ack_latency_ns.unwrap_or_default()
        ));
    }

    ReplayReport {
        events: events.len(),
        sequence_gaps,
        duplicate_sequences,
        out_of_order_sequences,
        stale_timestamps,
        crossed_books,
        rejected_orders,
        ack_latency_p50_ns,
        ack_latency_p99_ns,
        final_state_digest,
        passed: violations.is_empty(),
        violations,
    }
}

pub fn evidence(
    events: &[MarketEvent],
    plan: &MarketFaultPlan,
    budget: &InvariantBudget,
) -> anyhow::Result<ExperimentEvidence> {
    let baseline = replay(events, budget);
    let (faulted, impact) = plan.apply(events)?;
    let chaos = replay(&faulted, budget);
    let restored = replay(events, budget);
    let disruption_observed = impact.total() > 0 && chaos != baseline;
    let restoration_verified =
        baseline.final_state_digest == restored.final_state_digest && baseline == restored;
    Ok(ExperimentEvidence {
        seed: plan.seed,
        baseline,
        chaos,
        restored,
        impact,
        disruption_observed,
        restoration_verified,
    })
}

fn budget_violation(violations: &mut Vec<String>, name: &str, actual: usize, limit: usize) {
    if actual > limit {
        violations.push(format!("{} {} exceeded {}", name, actual, limit));
    }
}

fn percentile(samples: &[u64], percentile: usize) -> u64 {
    if samples.is_empty() {
        return 0;
    }
    let index = ((samples.len() * percentile).div_ceil(100))
        .saturating_sub(1)
        .min(samples.len() - 1);
    samples[index]
}

fn state_digest(books: &HashMap<(&str, &str), Book>) -> String {
    let mut streams: Vec<_> = books.iter().collect();
    streams.sort_by_key(|((venue, symbol), _)| (*venue, *symbol));
    let bytes = serde_json::to_vec(&streams).expect("book state is serializable");
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MarketFault;

    fn feed() -> Vec<MarketEvent> {
        vec![
            MarketEvent {
                sequence: 1,
                timestamp_ns: 1_000,
                venue: "XNAS".into(),
                symbol: "ACME".into(),
                kind: EventKind::BookUpdate {
                    side: Side::Bid,
                    price_ticks: 10_000,
                    quantity: 100,
                    action: BookAction::Upsert,
                },
            },
            MarketEvent {
                sequence: 2,
                timestamp_ns: 2_000,
                venue: "XNAS".into(),
                symbol: "ACME".into(),
                kind: EventKind::BookUpdate {
                    side: Side::Ask,
                    price_ticks: 10_001,
                    quantity: 80,
                    action: BookAction::Upsert,
                },
            },
            MarketEvent {
                sequence: 3,
                timestamp_ns: 3_000,
                venue: "XNAS".into(),
                symbol: "ACME".into(),
                kind: EventKind::OrderAck {
                    client_order_id: "order-1".into(),
                    accepted: true,
                    latency_ns: 25_000,
                },
            },
            MarketEvent {
                sequence: 4,
                timestamp_ns: 4_000,
                venue: "XNAS".into(),
                symbol: "ACME".into(),
                kind: EventKind::Heartbeat,
            },
        ]
    }

    #[test]
    fn deterministic_faults_prove_disruption_and_restoration() {
        let plan = MarketFaultPlan {
            seed: 42,
            faults: vec![MarketFault::DropEvery {
                every: 4,
                offset: 1,
            }],
        };
        let result = evidence(&feed(), &plan, &InvariantBudget::default()).unwrap();
        assert_eq!(result.impact.dropped, 1);
        assert!(result.disruption_observed);
        assert!(result.restoration_verified);
        assert_eq!(result.chaos.sequence_gaps, 1);
        assert!(!result.chaos.passed);
    }

    #[test]
    fn p99_latency_budget_catches_delayed_acknowledgements() {
        let plan = MarketFaultPlan {
            seed: 7,
            faults: vec![MarketFault::AckDelay {
                every: 1,
                offset: 0,
                delay_ns: 1_000_000,
            }],
        };
        let budget = InvariantBudget {
            max_p99_ack_latency_ns: Some(100_000),
            ..InvariantBudget::default()
        };
        let result = evidence(&feed(), &plan, &budget).unwrap();
        assert_eq!(result.impact.acks_delayed, 1);
        assert!(!result.chaos.passed);
        assert!(result.chaos.violations[0].contains("acknowledgement latency"));
    }
}
