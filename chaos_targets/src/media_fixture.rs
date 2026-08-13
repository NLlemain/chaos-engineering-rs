use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct HlsFixture {
    playlist: String,
    segments: BTreeMap<String, Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovedSegment {
    uri: String,
    bytes: Vec<u8>,
}

impl HlsFixture {
    pub fn new(
        playlist: impl Into<String>,
        segments: impl IntoIterator<Item = (String, Vec<u8>)>,
    ) -> Result<Self> {
        let playlist = playlist.into();
        let segments: BTreeMap<_, _> = segments.into_iter().collect();
        for uri in playlist_segment_uris(&playlist) {
            if !segments.contains_key(uri) {
                bail!("playlist segment '{}' has no packet fixture", uri);
            }
        }
        Ok(Self { playlist, segments })
    }

    pub fn playlist(&self) -> &str {
        &self.playlist
    }

    pub fn segment(&self, uri: &str) -> Option<&[u8]> {
        self.segments.get(uri).map(Vec::as_slice)
    }

    pub fn disrupt_segment(&mut self, uri: &str) -> Result<RemovedSegment> {
        let bytes = self
            .segments
            .remove(uri)
            .with_context(|| format!("segment '{}' is already missing or unknown", uri))?;
        Ok(RemovedSegment {
            uri: uri.to_string(),
            bytes,
        })
    }

    pub fn restore_segment(&mut self, removed: RemovedSegment) -> Result<()> {
        if self.segments.contains_key(&removed.uri) {
            bail!("segment '{}' is already present", removed.uri);
        }
        self.segments.insert(removed.uri, removed.bytes);
        Ok(())
    }
}

pub fn playlist_segment_uris(playlist: &str) -> impl Iterator<Item = &str> {
    playlist
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RtpCodec {
    H264,
    Vp8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RtpPacket<'a> {
    pub marker: bool,
    pub payload_type: u8,
    pub sequence: u16,
    pub timestamp: u32,
    pub ssrc: u32,
    pub payload: &'a [u8],
}

impl<'a> RtpPacket<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() < 12 {
            bail!("RTP packet is shorter than the fixed header");
        }
        if bytes[0] >> 6 != 2 {
            bail!("RTP packet version is not 2");
        }

        let csrc_count = usize::from(bytes[0] & 0x0f);
        let mut payload_offset = 12 + csrc_count * 4;
        if bytes.len() < payload_offset {
            bail!("RTP packet has a truncated CSRC list");
        }
        if bytes[0] & 0x10 != 0 {
            if bytes.len() < payload_offset + 4 {
                bail!("RTP packet has a truncated extension header");
            }
            let extension_words = usize::from(u16::from_be_bytes([
                bytes[payload_offset + 2],
                bytes[payload_offset + 3],
            ]));
            payload_offset += 4 + extension_words * 4;
            if bytes.len() < payload_offset {
                bail!("RTP packet has truncated extension data");
            }
        }

        let padding = if bytes[0] & 0x20 != 0 {
            usize::from(*bytes.last().context("RTP padding length is missing")?)
        } else {
            0
        };
        if padding > bytes.len().saturating_sub(payload_offset) {
            bail!("RTP padding exceeds payload length");
        }
        let payload_end = bytes.len() - padding;

        Ok(Self {
            marker: bytes[1] & 0x80 != 0,
            payload_type: bytes[1] & 0x7f,
            sequence: u16::from_be_bytes([bytes[2], bytes[3]]),
            timestamp: u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            ssrc: u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
            payload: &bytes[payload_offset..payload_end],
        })
    }

    pub fn is_keyframe(&self, codec: RtpCodec) -> Result<bool> {
        match codec {
            RtpCodec::H264 => h264_keyframe(self.payload),
            RtpCodec::Vp8 => vp8_keyframe(self.payload),
        }
    }
}

pub fn drop_keyframes(packets: &[Vec<u8>], codec: RtpCodec) -> Result<(Vec<Vec<u8>>, Vec<u16>)> {
    let mut kept = Vec::new();
    let mut dropped = Vec::new();
    for bytes in packets {
        let packet = RtpPacket::parse(bytes)?;
        if packet.is_keyframe(codec)? {
            dropped.push(packet.sequence);
        } else {
            kept.push(bytes.clone());
        }
    }
    Ok((kept, dropped))
}

fn h264_keyframe(payload: &[u8]) -> Result<bool> {
    let first = *payload.first().context("H.264 RTP payload is empty")?;
    match first & 0x1f {
        5 => Ok(true),
        28 => {
            let fragment = *payload.get(1).context("H.264 FU-A header is missing")?;
            Ok(fragment & 0x80 != 0 && fragment & 0x1f == 5)
        }
        _ => Ok(false),
    }
}

fn vp8_keyframe(payload: &[u8]) -> Result<bool> {
    let descriptor = *payload.first().context("VP8 RTP payload is empty")?;
    let starts_partition = descriptor & 0x10 != 0 && descriptor & 0x0f == 0;
    if !starts_partition {
        return Ok(false);
    }

    let mut offset = 1usize;
    if descriptor & 0x80 != 0 {
        let extension = *payload
            .get(offset)
            .context("VP8 extension byte is missing")?;
        offset += 1;
        if extension & 0x80 != 0 {
            let picture_id = *payload.get(offset).context("VP8 picture ID is missing")?;
            offset += if picture_id & 0x80 != 0 { 2 } else { 1 };
        }
        if extension & 0x40 != 0 {
            offset += 1;
        }
        if extension & 0x20 != 0 || extension & 0x10 != 0 {
            offset += 1;
        }
    }
    let frame_tag = *payload.get(offset).context("VP8 frame tag is missing")?;
    Ok(frame_tag & 0x01 == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_hex(value: &str) -> Vec<u8> {
        value
            .split_whitespace()
            .map(|byte| u8::from_str_radix(byte, 16).unwrap())
            .collect()
    }

    #[test]
    fn hls_segment_loss_is_observable_and_reversible() {
        let playlist = include_str!("../tests/fixtures/media/live.m3u8");
        let segment = decode_hex(include_str!("../tests/fixtures/media/segment-101.ts.hex"));
        let mut fixture = HlsFixture::new(
            playlist,
            [
                ("segment-100.ts".into(), vec![0x47; 188]),
                ("segment-101.ts".into(), segment.clone()),
                ("segment-102.ts".into(), vec![0x47; 188]),
            ],
        )
        .unwrap();

        let removed = fixture.disrupt_segment("segment-101.ts").unwrap();
        assert!(fixture.segment("segment-101.ts").is_none());
        fixture.restore_segment(removed).unwrap();
        assert_eq!(fixture.segment("segment-101.ts"), Some(segment.as_slice()));
    }

    #[test]
    fn vp8_keyframe_packets_are_dropped_by_sequence_number() {
        let keyframe = decode_hex(include_str!("../tests/fixtures/media/vp8-keyframe.rtp.hex"));
        let delta = decode_hex(include_str!("../tests/fixtures/media/vp8-delta.rtp.hex"));
        let (kept, dropped) = drop_keyframes(&[keyframe, delta.clone()], RtpCodec::Vp8).unwrap();
        assert_eq!(dropped, [100]);
        assert_eq!(kept, [delta]);
    }

    #[test]
    fn h264_idr_and_fu_a_start_are_keyframes() {
        let idr = decode_hex(include_str!("../tests/fixtures/media/h264-idr.rtp.hex"));
        let fua = decode_hex(include_str!("../tests/fixtures/media/h264-fua-idr.rtp.hex"));
        assert!(RtpPacket::parse(&idr)
            .unwrap()
            .is_keyframe(RtpCodec::H264)
            .unwrap());
        assert!(RtpPacket::parse(&fua)
            .unwrap()
            .is_keyframe(RtpCodec::H264)
            .unwrap());
    }
}
