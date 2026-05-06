#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct WavInfo {
    pub(crate) sample_rate: u32,
    pub(crate) channels: u16,
    pub(crate) bits_per_sample: u16,
    pub(crate) data_bytes: u32,
}

impl WavInfo {
    pub(crate) fn duration_secs_f64(&self) -> Option<f64> {
        if self.sample_rate == 0 {
            return None;
        }
        let bytes_per_sample = (self.bits_per_sample as u32).checked_div(8)?;
        let bytes_per_frame = bytes_per_sample.checked_mul(self.channels as u32)?;
        if bytes_per_frame == 0 {
            return None;
        }
        let frames = (self.data_bytes as f64) / (bytes_per_frame as f64);
        Some(frames / (self.sample_rate as f64))
    }
}

fn read_u16_le(bytes: &[u8], offset: usize) -> Option<u16> {
    let b = bytes.get(offset..offset + 2)?;
    Some(u16::from_le_bytes([b[0], b[1]]))
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Option<u32> {
    let b = bytes.get(offset..offset + 4)?;
    Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Best-effort parsing of a RIFF/WAVE header.
///
/// Returns None if the input does not look like a WAV file or required chunks are missing.
pub(crate) fn parse_wav_info(bytes: &[u8]) -> Option<WavInfo> {
    if bytes.len() < 44 {
        return None;
    }
    if &bytes[0..4] != b"RIFF" {
        return None;
    }
    if &bytes[8..12] != b"WAVE" {
        return None;
    }

    let mut offset = 12usize;
    let mut sample_rate: Option<u32> = None;
    let mut channels: Option<u16> = None;
    let mut bits_per_sample: Option<u16> = None;
    let mut data_bytes: Option<u32> = None;

    while offset + 8 <= bytes.len() {
        let chunk_id = bytes.get(offset..offset + 4)?;
        let chunk_size = read_u32_le(bytes, offset + 4)? as usize;
        let chunk_data_offset = offset + 8;
        let chunk_end = chunk_data_offset.saturating_add(chunk_size);
        if chunk_end > bytes.len() {
            break;
        }

        match chunk_id {
            b"fmt " if chunk_size >= 16 => {
                // fmt chunk requires at least 16 bytes for PCM.
                channels = read_u16_le(bytes, chunk_data_offset + 2);
                sample_rate = read_u32_le(bytes, chunk_data_offset + 4);
                bits_per_sample = read_u16_le(bytes, chunk_data_offset + 14);
            }
            b"data" => {
                data_bytes = Some(chunk_size as u32);
            }
            _ => {}
        }

        // Chunks are word-aligned.
        offset = chunk_end + (chunk_size % 2);

        if sample_rate.is_some()
            && channels.is_some()
            && bits_per_sample.is_some()
            && data_bytes.is_some()
        {
            break;
        }
    }

    Some(WavInfo {
        sample_rate: sample_rate?,
        channels: channels?,
        bits_per_sample: bits_per_sample?,
        data_bytes: data_bytes?,
    })
}

#[cfg(test)]
mod tests {
    use super::{parse_wav_info, WavInfo};

    #[test]
    fn parses_basic_wav_header() {
        let mut buf = Vec::new();
        {
            let spec = hound::WavSpec {
                channels: 1,
                sample_rate: 16_000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            };
            let mut writer =
                hound::WavWriter::new(std::io::Cursor::new(&mut buf), spec).expect("writer");
            for _ in 0..16_000 {
                writer.write_sample::<i16>(0).expect("sample");
            }
            writer.finalize().expect("finalize");
        }

        let info = parse_wav_info(&buf).expect("wav info");
        assert_eq!(
            info,
            WavInfo {
                sample_rate: 16_000,
                channels: 1,
                bits_per_sample: 16,
                data_bytes: 16_000 * 2,
            }
        );
        let dur = info.duration_secs_f64().expect("duration");
        assert!((dur - 1.0).abs() < 0.01);
    }

    #[test]
    fn rejects_non_wav() {
        assert!(parse_wav_info(b"not a wav").is_none());
    }
}
