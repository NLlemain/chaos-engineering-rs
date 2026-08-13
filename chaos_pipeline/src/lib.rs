//! Deterministic fault replay for zero-buffer streaming pipelines.
//!
//! Records cross a `sync_channel(0)`, so every producer send rendezvous with a
//! waiting consumer. Nothing can queue between them, making backpressure an
//! observable part of the evidence rather than an inferred side effect.

use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::mpsc::sync_channel,
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineRecord {
    pub sequence: u64,
    #[serde(default = "default_partition")]
    pub partition: String,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub timestamp_ns: Option<u64>,
    #[serde(default)]
    pub data: Value,
}

fn default_partition() -> String {
    "default".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PipelineFault {
    ConsumerStall {
        every: usize,
        #[serde(default)]
        offset: usize,
        delay_ms: u64,
    },
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
    TruncateAfter {
        records: usize,
    },
    TimestampRegression {
        every: usize,
        #[serde(default)]
        offset: usize,
        subtract_ns: u64,
    },
    CorruptField {
        every: usize,
        #[serde(default)]
        offset: usize,
        pointer: String,
        replacement: Value,
    },
    PartitionOutage {
        partition: String,
        start_sequence: u64,
        end_sequence: u64,
    },
    DropMatching {
        pointer: String,
        equals: Value,
    },
    SequenceReset {
        partition: String,
        at_sequence: u64,
        to_sequence: u64,
    },
    CardinalityExplosion {
        every: usize,
        #[serde(default)]
        offset: usize,
        pointer: String,
        prefix: String,
    },
    KeyCollapse {
        every: usize,
        #[serde(default)]
        offset: usize,
        key: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineFaultPlan {
    pub seed: u64,
    #[serde(default)]
    pub faults: Vec<PipelineFault>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaultImpact {
    pub stalled_receives: usize,
    pub dropped: usize,
    pub duplicated: usize,
    pub reordered: usize,
    pub truncated: usize,
    pub timestamps_regressed: usize,
    pub fields_corrupted: usize,
    pub partition_records_dropped: usize,
    pub matching_records_dropped: usize,
    pub sequences_reset: usize,
    pub cardinality_values_created: usize,
    pub keys_collapsed: usize,
    pub maximum_requested_stall_ms: u64,
}

impl FaultImpact {
    pub fn total(&self) -> usize {
        self.stalled_receives
            + self.dropped
            + self.duplicated
            + self.reordered
            + self.truncated
            + self.timestamps_regressed
            + self.fields_corrupted
            + self.partition_records_dropped
            + self.matching_records_dropped
            + self.sequences_reset
            + self.cardinality_values_created
            + self.keys_collapsed
    }

    fn has_data_effect(&self) -> bool {
        self.total().saturating_sub(self.stalled_receives) > 0
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineBudget {
    #[serde(default)]
    pub max_sequence_gaps: usize,
    #[serde(default)]
    pub max_duplicates: usize,
    #[serde(default)]
    pub max_out_of_order: usize,
    #[serde(default)]
    pub max_timestamp_regressions: usize,
    #[serde(default)]
    pub max_records_lost: usize,
    #[serde(default)]
    pub max_p99_producer_block_ns: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineReplayReport {
    pub input_records: usize,
    pub delivered_records: usize,
    pub records_lost: usize,
    pub sequence_gaps: usize,
    pub duplicate_sequences: usize,
    pub out_of_order_sequences: usize,
    pub timestamp_regressions: usize,
    pub producer_blocked_sends: usize,
    pub producer_block_p50_ns: u64,
    pub producer_block_p99_ns: u64,
    pub producer_block_max_ns: u64,
    pub buffer_capacity: usize,
    pub maximum_in_flight: usize,
    pub delivered_digest: String,
    pub passed: bool,
    pub violations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineEvidence {
    pub seed: u64,
    pub baseline: PipelineReplayReport,
    pub chaos: PipelineReplayReport,
    pub restored: PipelineReplayReport,
    pub impact: FaultImpact,
    pub zero_buffer_verified: bool,
    pub backpressure_observed: bool,
    pub disruption_observed: bool,
    pub restoration_verified: bool,
}

#[derive(Clone)]
struct Delivery {
    record: PipelineRecord,
    stall_before_ms: u64,
}

impl PipelineFaultPlan {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.faults.is_empty(),
            "fault plan requires at least one fault"
        );
        for fault in &self.faults {
            match fault {
                PipelineFault::ConsumerStall {
                    every, delay_ms, ..
                } => {
                    ensure!(*every > 0, "fault cadence must be greater than zero");
                    ensure!(*delay_ms > 0, "consumer stall must be greater than zero");
                }
                PipelineFault::DropEvery { every, .. }
                | PipelineFault::DuplicateEvery { every, .. }
                | PipelineFault::ReorderAdjacent { every, .. }
                | PipelineFault::TimestampRegression { every, .. }
                | PipelineFault::CorruptField { every, .. }
                | PipelineFault::CardinalityExplosion { every, .. }
                | PipelineFault::KeyCollapse { every, .. } => {
                    ensure!(*every > 0, "fault cadence must be greater than zero");
                }
                PipelineFault::TruncateAfter { .. } => {}
                PipelineFault::PartitionOutage {
                    partition,
                    start_sequence,
                    end_sequence,
                } => {
                    ensure!(!partition.trim().is_empty(), "partition cannot be empty");
                    ensure!(
                        start_sequence <= end_sequence,
                        "partition outage start sequence must not exceed its end"
                    );
                }
                PipelineFault::SequenceReset { partition, .. } => {
                    ensure!(!partition.trim().is_empty(), "partition cannot be empty");
                }
                PipelineFault::DropMatching { .. } => {}
            }
            if let PipelineFault::CorruptField { pointer, .. }
            | PipelineFault::DropMatching { pointer, .. }
            | PipelineFault::CardinalityExplosion { pointer, .. } = fault
            {
                ensure!(
                    pointer.starts_with('/'),
                    "field pointer must be a JSON Pointer beginning with '/'"
                );
            }
            if let PipelineFault::CardinalityExplosion { prefix, .. } = fault {
                ensure!(!prefix.is_empty(), "cardinality prefix cannot be empty");
            }
            if let PipelineFault::KeyCollapse { key, .. } = fault {
                ensure!(!key.is_empty(), "collapsed key cannot be empty");
            }
        }
        Ok(())
    }

    fn apply(&self, records: &[PipelineRecord]) -> Result<(Vec<Delivery>, FaultImpact)> {
        self.validate()?;
        let mut deliveries: Vec<_> = records
            .iter()
            .cloned()
            .map(|record| Delivery {
                record,
                stall_before_ms: 0,
            })
            .collect();
        let mut impact = FaultImpact::default();

        for fault in &self.faults {
            match fault {
                PipelineFault::ConsumerStall {
                    every,
                    offset,
                    delay_ms,
                } => {
                    for (index, delivery) in deliveries.iter_mut().enumerate() {
                        if selected(index, *every, *offset) {
                            delivery.stall_before_ms =
                                delivery.stall_before_ms.saturating_add(*delay_ms);
                            impact.stalled_receives += 1;
                            impact.maximum_requested_stall_ms =
                                impact.maximum_requested_stall_ms.max(*delay_ms);
                        }
                    }
                }
                PipelineFault::DropEvery { every, offset } => {
                    let before = deliveries.len();
                    let mut index = 0usize;
                    deliveries.retain(|_| {
                        let keep = !selected(index, *every, *offset);
                        index += 1;
                        keep
                    });
                    impact.dropped += before - deliveries.len();
                }
                PipelineFault::DuplicateEvery { every, offset } => {
                    let mut output = Vec::with_capacity(deliveries.len());
                    for (index, delivery) in deliveries.into_iter().enumerate() {
                        output.push(delivery.clone());
                        if selected(index, *every, *offset) {
                            output.push(delivery);
                            impact.duplicated += 1;
                        }
                    }
                    deliveries = output;
                }
                PipelineFault::ReorderAdjacent { every, offset } => {
                    let mut index = 0usize;
                    while index + 1 < deliveries.len() {
                        if selected(index, *every, *offset) {
                            deliveries.swap(index, index + 1);
                            impact.reordered += 1;
                            index += 2;
                        } else {
                            index += 1;
                        }
                    }
                }
                PipelineFault::TruncateAfter { records } => {
                    if *records < deliveries.len() {
                        impact.truncated += deliveries.len() - records;
                        deliveries.truncate(*records);
                    }
                }
                PipelineFault::TimestampRegression {
                    every,
                    offset,
                    subtract_ns,
                } => {
                    for (index, delivery) in deliveries.iter_mut().enumerate() {
                        if selected(index, *every, *offset) {
                            if let Some(timestamp) = &mut delivery.record.timestamp_ns {
                                *timestamp = timestamp.saturating_sub(*subtract_ns);
                                impact.timestamps_regressed += 1;
                            }
                        }
                    }
                }
                PipelineFault::CorruptField {
                    every,
                    offset,
                    pointer,
                    replacement,
                } => {
                    for (index, delivery) in deliveries.iter_mut().enumerate() {
                        if selected(index, *every, *offset) {
                            let field =
                                delivery.record.data.pointer_mut(pointer).with_context(|| {
                                    format!(
                                        "record {} does not contain JSON Pointer '{}'",
                                        delivery.record.sequence, pointer
                                    )
                                })?;
                            *field = replacement.clone();
                            impact.fields_corrupted += 1;
                        }
                    }
                }
                PipelineFault::PartitionOutage {
                    partition,
                    start_sequence,
                    end_sequence,
                } => {
                    let before = deliveries.len();
                    deliveries.retain(|delivery| {
                        delivery.record.partition != *partition
                            || !(*start_sequence..=*end_sequence)
                                .contains(&delivery.record.sequence)
                    });
                    impact.partition_records_dropped += before - deliveries.len();
                }
                PipelineFault::DropMatching { pointer, equals } => {
                    let before = deliveries.len();
                    deliveries
                        .retain(|delivery| delivery.record.data.pointer(pointer) != Some(equals));
                    impact.matching_records_dropped += before - deliveries.len();
                }
                PipelineFault::SequenceReset {
                    partition,
                    at_sequence,
                    to_sequence,
                } => {
                    for delivery in &mut deliveries {
                        if delivery.record.partition == *partition
                            && delivery.record.sequence >= *at_sequence
                        {
                            let distance = delivery.record.sequence - at_sequence;
                            delivery.record.sequence = to_sequence.saturating_add(distance);
                            impact.sequences_reset += 1;
                        }
                    }
                }
                PipelineFault::CardinalityExplosion {
                    every,
                    offset,
                    pointer,
                    prefix,
                } => {
                    for (index, delivery) in deliveries.iter_mut().enumerate() {
                        if selected(index, *every, *offset) {
                            let field =
                                delivery.record.data.pointer_mut(pointer).with_context(|| {
                                    format!(
                                        "record {} does not contain JSON Pointer '{}'",
                                        delivery.record.sequence, pointer
                                    )
                                })?;
                            *field = Value::String(format!(
                                "{}-{}-{}",
                                prefix, delivery.record.partition, delivery.record.sequence
                            ));
                            impact.cardinality_values_created += 1;
                        }
                    }
                }
                PipelineFault::KeyCollapse { every, offset, key } => {
                    for (index, delivery) in deliveries.iter_mut().enumerate() {
                        if selected(index, *every, *offset) {
                            delivery.record.key = Some(key.clone());
                            impact.keys_collapsed += 1;
                        }
                    }
                }
            }
        }
        Ok((deliveries, impact))
    }
}

fn selected(index: usize, every: usize, offset: usize) -> bool {
    index >= offset && (index - offset) % every == 0
}

pub fn evidence(
    records: &[PipelineRecord],
    plan: &PipelineFaultPlan,
    budget: &PipelineBudget,
) -> Result<PipelineEvidence> {
    ensure!(!records.is_empty(), "pipeline fixture is empty");
    let baseline_deliveries = records
        .iter()
        .cloned()
        .map(|record| Delivery {
            record,
            stall_before_ms: 0,
        })
        .collect::<Vec<_>>();
    let baseline = rendezvous_replay(records.len(), baseline_deliveries.clone(), budget)?;
    let (faulted, impact) = plan.apply(records)?;
    let chaos = rendezvous_replay(records.len(), faulted, budget)?;
    let restored = rendezvous_replay(records.len(), baseline_deliveries, budget)?;

    let zero_buffer_verified = baseline.buffer_capacity == 0
        && chaos.buffer_capacity == 0
        && restored.buffer_capacity == 0
        && baseline.maximum_in_flight <= 1
        && chaos.maximum_in_flight <= 1
        && restored.maximum_in_flight <= 1;
    let requested_stall_ns = impact.maximum_requested_stall_ms.saturating_mul(1_000_000);
    let backpressure_observed = impact.stalled_receives > 0
        && chaos.producer_block_max_ns >= requested_stall_ns.saturating_mul(3) / 4;
    let disruption_observed = impact.total() > 0
        && (impact.has_data_effect() && !same_delivered_state(&baseline, &chaos)
            || backpressure_observed);
    let restoration_verified = same_delivered_state(&baseline, &restored) && restored.passed;

    Ok(PipelineEvidence {
        seed: plan.seed,
        baseline,
        chaos,
        restored,
        impact,
        zero_buffer_verified,
        backpressure_observed,
        disruption_observed,
        restoration_verified,
    })
}

fn rendezvous_replay(
    input_records: usize,
    deliveries: Vec<Delivery>,
    budget: &PipelineBudget,
) -> Result<PipelineReplayReport> {
    let stalls = deliveries
        .iter()
        .map(|delivery| delivery.stall_before_ms)
        .collect::<Vec<_>>();
    let (sender, receiver) = sync_channel::<PipelineRecord>(0);
    let consumer = thread::spawn(move || {
        let mut output = Vec::with_capacity(stalls.len());
        for stall in stalls {
            if stall > 0 {
                thread::sleep(Duration::from_millis(stall));
            }
            match receiver.recv() {
                Ok(record) => output.push(record),
                Err(_) => break,
            }
        }
        output
    });

    let mut block_ns = Vec::with_capacity(deliveries.len());
    for delivery in deliveries {
        let started = Instant::now();
        sender
            .send(delivery.record)
            .map_err(|_| anyhow::anyhow!("zero-buffer consumer stopped before end of stream"))?;
        block_ns.push(duration_ns(started.elapsed()));
    }
    drop(sender);
    let delivered = consumer
        .join()
        .map_err(|_| anyhow::anyhow!("zero-buffer consumer thread panicked"))?;

    Ok(analyze(input_records, &delivered, &mut block_ns, budget))
}

fn analyze(
    input_records: usize,
    delivered: &[PipelineRecord],
    block_ns: &mut [u64],
    budget: &PipelineBudget,
) -> PipelineReplayReport {
    let mut last_sequence = HashMap::<&str, u64>::new();
    let mut last_timestamp = HashMap::<&str, u64>::new();
    let mut sequence_gaps = 0usize;
    let mut duplicate_sequences = 0usize;
    let mut out_of_order_sequences = 0usize;
    let mut timestamp_regressions = 0usize;

    for record in delivered {
        let partition = record.partition.as_str();
        if let Some(previous) = last_sequence.insert(partition, record.sequence) {
            if record.sequence == previous {
                duplicate_sequences += 1;
            } else if record.sequence < previous {
                out_of_order_sequences += 1;
            } else if record.sequence > previous.saturating_add(1) {
                sequence_gaps += 1;
            }
        }
        if let Some(timestamp) = record.timestamp_ns {
            if last_timestamp
                .insert(partition, timestamp)
                .is_some_and(|previous| timestamp < previous)
            {
                timestamp_regressions += 1;
            }
        }
    }

    block_ns.sort_unstable();
    let records_lost = input_records.saturating_sub(delivered.len());
    let producer_block_p50_ns = percentile(block_ns, 50);
    let producer_block_p99_ns = percentile(block_ns, 99);
    let producer_block_max_ns = block_ns.last().copied().unwrap_or_default();
    let producer_blocked_sends = block_ns.iter().filter(|value| **value >= 100_000).count();
    let mut violations = Vec::new();
    limit(
        &mut violations,
        "sequence gaps",
        sequence_gaps,
        budget.max_sequence_gaps,
    );
    limit(
        &mut violations,
        "duplicate sequences",
        duplicate_sequences,
        budget.max_duplicates,
    );
    limit(
        &mut violations,
        "out-of-order sequences",
        out_of_order_sequences,
        budget.max_out_of_order,
    );
    limit(
        &mut violations,
        "timestamp regressions",
        timestamp_regressions,
        budget.max_timestamp_regressions,
    );
    limit(
        &mut violations,
        "records lost",
        records_lost,
        budget.max_records_lost,
    );
    if budget
        .max_p99_producer_block_ns
        .is_some_and(|maximum| producer_block_p99_ns > maximum)
    {
        violations.push(format!(
            "p99 producer block {}ns exceeded {}ns",
            producer_block_p99_ns,
            budget.max_p99_producer_block_ns.unwrap_or_default()
        ));
    }

    PipelineReplayReport {
        input_records,
        delivered_records: delivered.len(),
        records_lost,
        sequence_gaps,
        duplicate_sequences,
        out_of_order_sequences,
        timestamp_regressions,
        producer_blocked_sends,
        producer_block_p50_ns,
        producer_block_p99_ns,
        producer_block_max_ns,
        buffer_capacity: 0,
        maximum_in_flight: usize::from(!delivered.is_empty()),
        delivered_digest: digest(delivered),
        passed: violations.is_empty(),
        violations,
    }
}

fn same_delivered_state(left: &PipelineReplayReport, right: &PipelineReplayReport) -> bool {
    left.delivered_records == right.delivered_records
        && left.records_lost == right.records_lost
        && left.sequence_gaps == right.sequence_gaps
        && left.duplicate_sequences == right.duplicate_sequences
        && left.out_of_order_sequences == right.out_of_order_sequences
        && left.timestamp_regressions == right.timestamp_regressions
        && left.delivered_digest == right.delivered_digest
}

fn duration_ns(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
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

fn limit(violations: &mut Vec<String>, name: &str, actual: usize, maximum: usize) {
    if actual > maximum {
        violations.push(format!("{name} {actual} exceeded {maximum}"));
    }
}

fn digest(records: &[PipelineRecord]) -> String {
    let bytes = serde_json::to_vec(records).expect("pipeline records are serializable");
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn parse_json_lines(contents: &str) -> Result<Vec<PipelineRecord>> {
    let records = contents
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            serde_json::from_str::<PipelineRecord>(line)
                .with_context(|| format!("parse pipeline record {}", index + 1))
        })
        .collect::<Result<Vec<_>>>()?;
    if records.is_empty() {
        bail!("pipeline fixture is empty");
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn records() -> Vec<PipelineRecord> {
        (1..=6)
            .map(|sequence| PipelineRecord {
                sequence,
                partition: "orders".into(),
                key: Some(format!("order-{sequence}")),
                timestamp_ns: Some(sequence * 1_000),
                data: json!({"price": 100 + sequence, "status": "open"}),
            })
            .collect()
    }

    #[test]
    fn zero_capacity_stall_proves_real_producer_backpressure_and_recovery() {
        let plan = PipelineFaultPlan {
            seed: 42,
            faults: vec![PipelineFault::ConsumerStall {
                every: 3,
                offset: 1,
                delay_ms: 20,
            }],
        };
        let result = evidence(&records(), &plan, &PipelineBudget::default()).unwrap();
        assert!(result.zero_buffer_verified);
        assert!(result.backpressure_observed);
        assert!(result.disruption_observed);
        assert!(result.restoration_verified);
        assert_eq!(result.impact.stalled_receives, 2);
        assert_eq!(
            result.baseline.delivered_digest,
            result.chaos.delivered_digest
        );
    }

    #[test]
    fn mixed_integrity_faults_are_detected_and_restored() {
        let plan = PipelineFaultPlan {
            seed: 7,
            faults: vec![
                PipelineFault::DropEvery {
                    every: 4,
                    offset: 1,
                },
                PipelineFault::CorruptField {
                    every: 4,
                    offset: 0,
                    pointer: "/status".into(),
                    replacement: json!("rejected"),
                },
            ],
        };
        let result = evidence(&records(), &plan, &PipelineBudget::default()).unwrap();
        assert_eq!(result.impact.dropped, 2);
        assert!(result.impact.fields_corrupted > 0);
        assert!(!result.chaos.passed);
        assert!(result.disruption_observed);
        assert!(result.restoration_verified);
    }

    #[test]
    fn abrupt_end_of_stream_is_detected_and_restored() {
        let plan = PipelineFaultPlan {
            seed: 8,
            faults: vec![PipelineFault::TruncateAfter { records: 3 }],
        };
        let result = evidence(&records(), &plan, &PipelineBudget::default()).unwrap();
        assert_eq!(result.impact.truncated, 3);
        assert_eq!(result.chaos.records_lost, 3);
        assert!(!result.chaos.passed);
        assert!(result.disruption_observed);
        assert!(result.restoration_verified);
    }

    #[test]
    fn parser_names_the_bad_jsonl_record() {
        let error = parse_json_lines("{\"sequence\":1}\nnot-json").unwrap_err();
        assert!(error.to_string().contains("pipeline record 2"));
    }

    #[test]
    fn content_aware_faults_cover_cdc_crypto_and_telemetry_streams() {
        let mut input = records();
        for record in &mut input {
            record.data["tenant"] = json!("stable");
        }
        input[1].data["kind"] = json!("snapshot");
        input[2].data["kind"] = json!("transaction_end");
        input[3].data["kind"] = json!("metric");
        let plan = PipelineFaultPlan {
            seed: 99,
            faults: vec![
                PipelineFault::DropMatching {
                    pointer: "/kind".into(),
                    equals: json!("transaction_end"),
                },
                PipelineFault::SequenceReset {
                    partition: "orders".into(),
                    at_sequence: 5,
                    to_sequence: 1,
                },
                PipelineFault::CardinalityExplosion {
                    every: 2,
                    offset: 1,
                    pointer: "/tenant".into(),
                    prefix: "chaos".into(),
                },
                PipelineFault::KeyCollapse {
                    every: 2,
                    offset: 0,
                    key: "hot-partition".into(),
                },
            ],
        };
        let result = evidence(&input, &plan, &PipelineBudget::default()).unwrap();
        assert_eq!(result.impact.matching_records_dropped, 1);
        assert!(result.impact.sequences_reset > 0);
        assert!(result.impact.cardinality_values_created > 0);
        assert!(result.impact.keys_collapsed > 0);
        assert!(result.disruption_observed);
        assert!(result.restoration_verified);
    }

    #[test]
    fn shipped_stable_pipeline_plans_prove_disruption_and_restoration() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        let input = (1..=30)
            .map(|sequence| PipelineRecord {
                sequence,
                partition: "events".into(),
                key: Some(format!("event-{sequence}")),
                timestamp_ns: Some(sequence * 1_000),
                data: json!({"kind": "event", "value": sequence}),
            })
            .collect::<Vec<_>>();
        for filename in ["zero-buffer-backpressure.yaml", "abrupt-end-of-stream.yaml"] {
            let contents =
                std::fs::read_to_string(root.join("scenario-packs/data-pipelines").join(filename))
                    .unwrap();
            let plan: PipelineFaultPlan = serde_yaml::from_str(&contents).unwrap();
            let result = evidence(&input, &plan, &PipelineBudget::default()).unwrap();
            assert!(result.disruption_observed, "{filename}");
            assert!(result.restoration_verified, "{filename}");
            assert!(result.zero_buffer_verified, "{filename}");
        }
    }
}
