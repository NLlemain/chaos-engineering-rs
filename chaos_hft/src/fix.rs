use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixMessage {
    pub fields: Vec<(u32, String)>,
}

impl FixMessage {
    pub fn parse(input: &str) -> anyhow::Result<Self> {
        let delimiter = if input.contains('\u{1}') {
            '\u{1}'
        } else {
            '|'
        };
        let mut fields = Vec::new();
        for field in input
            .trim_end_matches([delimiter, '\r', '\n'])
            .split(delimiter)
        {
            let (tag, value) = field
                .split_once('=')
                .ok_or_else(|| anyhow::anyhow!("FIX field '{field}' is missing '='"))?;
            fields.push((tag.parse()?, value.to_string()));
        }
        anyhow::ensure!(
            fields.first().is_some_and(|(tag, _)| *tag == 8),
            "FIX message must start with BeginString (8)"
        );
        anyhow::ensure!(
            fields.iter().any(|(tag, _)| *tag == 35),
            "FIX message is missing MsgType (35)"
        );
        Ok(Self { fields })
    }

    pub fn get(&self, tag: u32) -> Option<&str> {
        self.fields
            .iter()
            .find_map(|(candidate, value)| (*candidate == tag).then_some(value.as_str()))
    }

    pub fn set(&mut self, tag: u32, value: impl Into<String>) {
        let value = value.into();
        if let Some((_, current)) = self
            .fields
            .iter_mut()
            .find(|(candidate, _)| *candidate == tag)
        {
            *current = value;
        } else {
            let checksum = self
                .fields
                .iter()
                .position(|(candidate, _)| *candidate == 10)
                .unwrap_or(self.fields.len());
            self.fields.insert(checksum, (tag, value));
        }
    }

    pub fn sequence(&self) -> anyhow::Result<u64> {
        self.get(34)
            .ok_or_else(|| anyhow::anyhow!("FIX message is missing MsgSeqNum (34)"))?
            .parse()
            .map_err(Into::into)
    }

    pub fn encode(&self, delimiter: char) -> anyhow::Result<String> {
        let begin = self.get(8).unwrap_or("FIX.4.4");
        let body_fields: Vec<_> = self
            .fields
            .iter()
            .filter(|(tag, _)| !matches!(*tag, 8..=10))
            .collect();
        let body = body_fields
            .iter()
            .map(|(tag, value)| format!("{tag}={value}{delimiter}"))
            .collect::<String>();
        let prefix = format!("8={begin}{delimiter}9={}{delimiter}{body}", body.len());
        let checksum = prefix
            .as_bytes()
            .iter()
            .fold(0u8, |sum, byte| sum.wrapping_add(*byte));
        Ok(format!("{prefix}10={checksum:03}{delimiter}"))
    }

    pub fn checksum_valid(&self) -> anyhow::Result<bool> {
        let encoded = self.encode('\u{1}')?;
        Ok(self.get(10) == FixMessage::parse(&encoded)?.get(10))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FixFault {
    SequenceGap { at: u64, gap: u64 },
    Duplicate { sequence: u64 },
    PossDup { sequence: u64 },
    CorruptChecksum { sequence: u64 },
    RejectExecution { sequence: u64, reason: String },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixFaultImpact {
    pub sequence_gaps: usize,
    pub duplicates: usize,
    pub poss_dup_flags: usize,
    pub corrupted_checksums: usize,
    pub execution_rejects: usize,
}

impl FixFault {
    pub fn apply_all(
        input: &[FixMessage],
        faults: &[Self],
    ) -> anyhow::Result<(Vec<String>, FixFaultImpact)> {
        let mut output = Vec::new();
        let mut impact = FixFaultImpact::default();
        let mut gaps = HashMap::new();
        for fault in faults {
            if let Self::SequenceGap { at, gap } = fault {
                anyhow::ensure!(*gap > 0, "FIX sequence gap must be greater than zero");
                gaps.insert(*at, *gap);
            }
        }

        for message in input {
            let original_sequence = message.sequence()?;
            let mut current = message.clone();
            if let Some(gap) = gaps.get(&original_sequence) {
                current.set(34, original_sequence.saturating_add(*gap).to_string());
                impact.sequence_gaps += 1;
            }
            for fault in faults {
                match fault {
                    Self::PossDup { sequence } if *sequence == original_sequence => {
                        current.set(43, "Y");
                        impact.poss_dup_flags += 1;
                    }
                    Self::RejectExecution { sequence, reason }
                        if *sequence == original_sequence =>
                    {
                        current.set(35, "8");
                        current.set(39, "8");
                        current.set(58, reason);
                        impact.execution_rejects += 1;
                    }
                    _ => {}
                }
            }

            let mut encoded = current.encode('\u{1}')?;
            if faults.iter().any(|fault| {
                matches!(fault, Self::CorruptChecksum { sequence } if *sequence == original_sequence)
            }) {
                let checksum = encoded
                    .rfind("10=")
                    .ok_or_else(|| anyhow::anyhow!("encoded FIX message has no checksum"))?;
                encoded.replace_range(checksum + 3..checksum + 6, "999");
                impact.corrupted_checksums += 1;
            }
            output.push(encoded.clone());
            if faults.iter().any(|fault| {
                matches!(fault, Self::Duplicate { sequence } if *sequence == original_sequence)
            }) {
                output.push(encoded);
                impact.duplicates += 1;
            }
        }
        Ok((output, impact))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order(sequence: u64) -> FixMessage {
        FixMessage {
            fields: vec![
                (8, "FIX.4.4".into()),
                (35, "D".into()),
                (34, sequence.to_string()),
                (49, "CLIENT".into()),
                (56, "VENUE".into()),
                (11, format!("order-{sequence}")),
            ],
        }
    }

    #[test]
    fn encoder_sets_body_length_and_checksum() {
        let encoded = order(7).encode('\u{1}').unwrap();
        let parsed = FixMessage::parse(&encoded).unwrap();
        assert_eq!(parsed.sequence().unwrap(), 7);
        assert!(parsed.checksum_valid().unwrap());
        assert!(encoded.contains("9="));
    }

    #[test]
    fn fix_faults_are_protocol_visible() {
        let (messages, impact) = FixFault::apply_all(
            &[order(1), order(2)],
            &[
                FixFault::SequenceGap { at: 2, gap: 3 },
                FixFault::PossDup { sequence: 1 },
                FixFault::Duplicate { sequence: 1 },
                FixFault::CorruptChecksum { sequence: 2 },
            ],
        )
        .unwrap();
        assert_eq!(messages.len(), 3);
        assert_eq!(impact.sequence_gaps, 1);
        assert_eq!(impact.duplicates, 1);
        assert_eq!(impact.poss_dup_flags, 1);
        assert_eq!(impact.corrupted_checksums, 1);
        assert_eq!(FixMessage::parse(&messages[0]).unwrap().get(43), Some("Y"));
        assert!(!FixMessage::parse(&messages[2])
            .unwrap()
            .checksum_valid()
            .unwrap());
    }
}
