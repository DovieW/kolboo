//! Post-recording uploads and durable partial transcripts. No History entries or
//! rewriting here: callers receive one assembled transcript for the normal flow.
use super::{stt_flow::SttResult, PipelineError};
use crate::recordings::audio::{AudioReader, TranscriptionAudio};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::OpenOptions,
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};
use tokio_util::sync::CancellationToken;

pub(super) const MAX_MEETING_WAV_BYTES: usize = 16000 * 2 * 4 * 60 * 60 + 44;
const UPLOAD_FRAMES: usize = 16000 * 10 * 60; // 19.2 MB, below the managed 25 MB body limit.
const MAX_CHECKPOINT_BYTES: u64 = 32 * 1024 * 1024;
const WORK_BATCH_BYTES: usize = 256 * 1024;
type MeetingWavReader = hound::WavReader<Box<dyn AudioReader>>;

#[derive(Serialize, Deserialize)]
struct CompletedUpload {
    key: String,
    text: String,
    duration_ms: u64,
    #[serde(default)]
    segments: Vec<crate::stt::SpeakerSegment>,
}

fn storage_error(_: impl std::fmt::Display) -> PipelineError {
    PipelineError::Config(
        "Meeting progress could not be saved or read. Your audio is retained.".into(),
    )
}

pub(super) async fn transcribe<F, Fut>(
    audio: TranscriptionAudio,
    path: Option<&Path>,
    settings_key: &str,
    cancel: &CancellationToken,
    upload: F,
) -> Result<SttResult, PipelineError>
where
    F: FnMut(Vec<u8>) -> Fut,
    Fut: std::future::Future<Output = Result<SttResult, PipelineError>>,
{
    transcribe_source(audio, path, settings_key, cancel, UPLOAD_FRAMES, upload).await
}

fn prepare_audio(
    audio: TranscriptionAudio,
    cancel: &CancellationToken,
) -> Result<(MeetingWavReader, String), PipelineError> {
    if cancel.is_cancelled() {
        return Err(PipelineError::Cancelled);
    }
    if audio.len() > MAX_MEETING_WAV_BYTES {
        return Err(PipelineError::Config(
            "Unsupported meeting audio format".into(),
        ));
    }
    let mut source = audio.reader().map_err(storage_error)?;
    let mut hasher = Sha256::new();
    let mut block = vec![0; WORK_BATCH_BYTES];
    loop {
        if cancel.is_cancelled() {
            return Err(PipelineError::Cancelled);
        }
        let count = source.read(&mut block).map_err(storage_error)?;
        if count == 0 {
            break;
        }
        hasher.update(&block[..count]);
    }
    source.seek(SeekFrom::Start(0)).map_err(storage_error)?;
    let reader = hound::WavReader::new(source).map_err(storage_error)?;
    let spec = reader.spec();
    if spec.channels != 1
        || spec.sample_rate != 16000
        || spec.bits_per_sample != 16
        || spec.sample_format != hound::SampleFormat::Int
    {
        return Err(PipelineError::Config(
            "Unsupported meeting audio format".into(),
        ));
    }
    Ok((reader, format!("{:x}", hasher.finalize())))
}

fn read_block(
    mut reader: MeetingWavReader,
    start: u32,
    block_frames: usize,
    cancel: &CancellationToken,
) -> Result<(MeetingWavReader, Vec<i16>), PipelineError> {
    if cancel.is_cancelled() {
        return Err(PipelineError::Cancelled);
    }
    reader.seek(start).map_err(storage_error)?;
    let mut samples = Vec::with_capacity(block_frames.min((reader.duration() - start) as usize));
    for (index, sample) in reader.samples::<i16>().take(block_frames).enumerate() {
        if index % (WORK_BATCH_BYTES / 2) == 0 && cancel.is_cancelled() {
            return Err(PipelineError::Cancelled);
        }
        samples.push(sample.map_err(storage_error)?);
    }
    // Keep existing quiet boundaries/checkpoint keys so installed users can resume.
    if samples.len() == block_frames && start + (samples.len() as u32) < reader.duration() {
        let window = 4800;
        if samples.len() > 160000 + window {
            let begin = samples.len() - 160000;
            let boundary = (begin..samples.len() - window)
                .step_by(window)
                .min_by_key(|offset| {
                    samples[*offset..*offset + window]
                        .iter()
                        .map(|value| (*value as i64).pow(2))
                        .sum::<i64>()
                })
                .unwrap_or(samples.len());
            samples.truncate(boundary);
        }
    }
    if samples.is_empty() {
        return Err(storage_error("empty block"));
    }
    Ok((reader, samples))
}

fn encode_block(
    samples: Vec<i16>,
    spec: hound::WavSpec,
    cancel: &CancellationToken,
) -> Result<Vec<u8>, PipelineError> {
    let mut buffer = std::io::Cursor::new(Vec::new());
    let mut writer = hound::WavWriter::new(&mut buffer, spec).map_err(storage_error)?;
    for (index, sample) in samples.into_iter().enumerate() {
        if index % (WORK_BATCH_BYTES / 2) == 0 && cancel.is_cancelled() {
            return Err(PipelineError::Cancelled);
        }
        writer.write_sample(sample).map_err(storage_error)?;
    }
    writer.finalize().map_err(storage_error)?;
    Ok(buffer.into_inner())
}

async fn transcribe_source<F, Fut>(
    audio: TranscriptionAudio,
    path: Option<&Path>,
    settings_key: &str,
    cancel: &CancellationToken,
    block_frames: usize,
    mut upload: F,
) -> Result<SttResult, PipelineError>
where
    F: FnMut(Vec<u8>) -> Fut,
    Fut: std::future::Future<Output = Result<SttResult, PipelineError>>,
{
    if cancel.is_cancelled() {
        return Err(PipelineError::Cancelled);
    }
    let preparation_cancel = cancel.clone();
    let (mut reader, source_key) =
        tokio::task::spawn_blocking(move || prepare_audio(audio, &preparation_cancel))
            .await
            .map_err(storage_error)??;
    let spec = reader.spec();
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = path
        .map(|path| options.open(path))
        .transpose()
        .map_err(storage_error)?;
    let mut bytes = Vec::new();
    if let Some(file) = &mut file {
        if file.metadata().map_err(storage_error)?.len() > MAX_CHECKPOINT_BYTES {
            return Err(storage_error("progress too large"));
        }
        file.read_to_end(&mut bytes).map_err(storage_error)?;
    }
    // A crash during append may leave only the last JSON line incomplete.
    let valid_length = bytes.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
    let mut completed = HashMap::new();
    for line in bytes[..valid_length]
        .split(|b| *b == b'\n')
        .filter(|line| !line.is_empty())
    {
        let entry: CompletedUpload = serde_json::from_slice(line).map_err(storage_error)?;
        completed.insert(entry.key.clone(), entry);
    }
    if let Some(file) = &mut file {
        file.set_len(valid_length as u64).map_err(storage_error)?;
        file.seek(SeekFrom::End(0)).map_err(storage_error)?;
    }
    let mut output = SttResult {
        segments: Vec::new(),
        text: String::new(),
        duration_ms: 0,
        retry: Default::default(),
    };
    let mut start = 0;
    let mut part = 1;
    while start < reader.duration() {
        if cancel.is_cancelled() {
            return Err(PipelineError::Cancelled);
        }
        let read_cancel = cancel.clone();
        let (next_reader, samples) = tokio::task::spawn_blocking(move || {
            read_block(reader, start, block_frames, &read_cancel)
        })
        .await
        .map_err(storage_error)??;
        reader = next_reader;
        let end = start + samples.len() as u32;
        let key = format!(
            "{:x}",
            Sha256::digest(format!("v1:{source_key}:{settings_key}:{start}:{end}"))
        );
        let entry = if let Some(entry) = completed.remove(&key) {
            entry
        } else {
            let encode_cancel = cancel.clone();
            let wav =
                tokio::task::spawn_blocking(move || encode_block(samples, spec, &encode_cancel))
                    .await
                    .map_err(storage_error)??;
            if cancel.is_cancelled() {
                return Err(PipelineError::Cancelled);
            }
            let result = upload(wav).await?;
            output.retry.attempts += result.retry.attempts;
            output.retry.retries += result.retry.retries;
            output.retry.total_delay_ms += result.retry.total_delay_ms;
            output.retry.last_error = result.retry.last_error;
            let entry = CompletedUpload {
                segments: result.segments,
                key,
                text: result.text,
                duration_ms: result.duration_ms,
            };
            let mut line = serde_json::to_vec(&entry).map_err(storage_error)?;
            line.push(b'\n');
            if let Some(file) = &mut file {
                if file.metadata().map_err(storage_error)?.len() + line.len() as u64
                    > MAX_CHECKPOINT_BYTES
                {
                    return Err(storage_error("progress too large"));
                }
                file.write_all(&line).map_err(storage_error)?;
                file.sync_all().map_err(storage_error)?;
            }
            entry
        };
        if !entry.text.trim().is_empty() {
            if !output.text.is_empty() {
                output.text.push('\n');
            }
            output.text.push_str(entry.text.trim());
        }
        output.duration_ms += entry.duration_ms;
        output
            .segments
            .extend(entry.segments.into_iter().map(|mut segment| {
                segment.part = part;
                segment.start_seconds += start as f64 / 16000.0;
                segment.end_seconds += start as f64 / 16000.0;
                segment
            }));
        part += 1;
        start = end;
    }
    if cancel.is_cancelled() {
        return Err(PipelineError::Cancelled);
    }
    Ok(output)
}

#[cfg(test)]
#[path = "meeting_transcription/tests.rs"]
mod tests;
