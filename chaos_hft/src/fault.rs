use crate::{EventKind, MarketEvent};
use rand::{rngs::StdRng, Rng, SeedableRng};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MarketFault {
    DropEvery {
        every: usize,
        #[serde(default)]
        offset: usize,
    },
    DuplicateEvery {
        every: usize,
        #[serde(default)]
        offset: usize,
    },
    ReorderAdjacent {
        every: usize,
        #[serde(default)]
        offset: usize,
    },
    ProbabilisticDrop {
        probability: f64,
    },
    TimestampSkew {
        every: usize,
        #[serde(default)]
        offset: usize,
        offset_ns: i64,
    },
    AckDelay {
        every: usize,
        #[serde(default)]
        offset: usize,
        delay_ns: u64,
    },
    CorruptQuantity {
        every: usize,
        #[serde(default)]
        offset: usize,
        quantity: u64,
    },
    VenuePartition {
        venue: String,
        start_sequence: u64,
        end_sequence: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketFaultPlan {
    pub seed: u64,
    #[serde(default)]
    pub faults: Vec<MarketFault>,
}

impl MarketFaultPlan {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.faults.is_empty(),
            "fault plan requires at least one fault"
        );
        for fault in &self.faults {
            match fault {
                MarketFault::DropEvery { every, .. }
                | MarketFault::DuplicateEvery { every, .. }
                | MarketFault::ReorderAdjacent { every, .. }
                | MarketFault::TimestampSkew { every, .. }
                | MarketFault::AckDelay { every, .. }
                | MarketFault::CorruptQuantity { every, .. } => {
                    anyhow::ensure!(*every > 0, "fault cadence must be greater than zero");
                }
                MarketFault::ProbabilisticDrop { probability } => anyhow::ensure!(
                    probability.is_finite() && (0.0..=1.0).contains(probability),
                    "drop probability must be between 0.0 and 1.0"
                ),
                MarketFault::VenuePartition {
                    venue,
                    start_sequence,
                    end_sequence,
                } => {
                    anyhow::ensure!(!venue.trim().is_empty(), "partition venue cannot be empty");
                    anyhow::ensure!(
                        start_sequence <= end_sequence,
                        "partition start sequence must not exceed its end"
                    );
                }
            }
        }
        Ok(())
    }

    pub fn apply(&self, input: &[MarketEvent]) -> anyhow::Result<(Vec<MarketEvent>, FaultImpact)> {
        self.validate()?;
        let mut rng = StdRng::seed_from_u64(self.seed);
        let mut output = input.to_vec();
        let mut impact = FaultImpact::default();

        for fault in &self.faults {
            let mut next = Vec::with_capacity(output.len());
            let mut pending = None;
            for (index, mut event) in output.into_iter().enumerate() {
                let selected = match fault {
                    MarketFault::DropEvery { every, offset }
                    | MarketFault::DuplicateEvery { every, offset }
                    | MarketFault::ReorderAdjacent { every, offset }
                    | MarketFault::TimestampSkew { every, offset, .. }
                    | MarketFault::AckDelay { every, offset, .. }
                    | MarketFault::CorruptQuantity { every, offset, .. } => {
                        index >= *offset && (index - *offset) % *every == 0
                    }
                    MarketFault::ProbabilisticDrop { probability } => {
                        *probability >= 1.0 || (*probability > 0.0 && rng.gen_bool(*probability))
                    }
                    MarketFault::VenuePartition {
                        venue,
                        start_sequence,
                        end_sequence,
                    } => {
                        event.venue == *venue
                            && (*start_sequence..=*end_sequence).contains(&event.sequence)
                    }
                };

                match fault {
                    MarketFault::DropEvery { .. }
                    | MarketFault::ProbabilisticDrop { .. }
                    | MarketFault::VenuePartition { .. }
                        if selected =>
                    {
                        impact.dropped += 1;
                    }
                    MarketFault::DuplicateEvery { .. } if selected => {
                        next.push(event.clone());
                        next.push(event);
                        impact.duplicated += 1;
                    }
                    MarketFault::ReorderAdjacent { .. } if selected && pending.is_none() => {
                        pending = Some(event);
                    }
                    MarketFault::TimestampSkew { offset_ns, .. } if selected => {
                        event.timestamp_ns = event.timestamp_ns.saturating_add_signed(*offset_ns);
                        next.push(event);
                        impact.timestamp_skewed += 1;
                    }
                    MarketFault::AckDelay { delay_ns, .. } if selected => {
                        if let EventKind::OrderAck { latency_ns, .. } = &mut event.kind {
                            *latency_ns = latency_ns.saturating_add(*delay_ns);
                            impact.acks_delayed += 1;
                        }
                        next.push(event);
                    }
                    MarketFault::CorruptQuantity { quantity, .. } if selected => {
                        if let EventKind::BookUpdate {
                            quantity: event_quantity,
                            ..
                        } = &mut event.kind
                        {
                            *event_quantity = *quantity;
                            impact.quantities_corrupted += 1;
                        }
                        next.push(event);
                    }
                    _ => {
                        next.push(event);
                        if let Some(held) = pending.take() {
                            next.push(held);
                            impact.reordered += 1;
                        }
                    }
                }
            }
            if let Some(held) = pending {
                next.push(held);
            }
            output = next;
        }

        Ok((output, impact))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaultImpact {
    pub dropped: usize,
    pub duplicated: usize,
    pub reordered: usize,
    pub timestamp_skewed: usize,
    pub acks_delayed: usize,
    pub quantities_corrupted: usize,
}

impl FaultImpact {
    pub fn total(self) -> usize {
        self.dropped
            + self.duplicated
            + self.reordered
            + self.timestamp_skewed
            + self.acks_delayed
            + self.quantities_corrupted
    }
}
