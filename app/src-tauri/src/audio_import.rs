//! Deliberate local-file import. Decode in bounded blocks, then atomically expose
//! a complete private journal to recovery. Never upload here or modify the source.
use crate::audio_capture::journal::Journal;
use crate::audio_normalization::{downmix_interleaved_to_mono, resample_to_16khz_vad_quality};
use crate::recordings::options::RecordingPreferences;
use std::fs::{File, Metadata, OpenOptions};
use std::path::{Path, PathBuf};
use symphonia::core::{
    audio::SampleBuffer,
    codecs::{CodecParameters, DecoderOptions, CODEC_TYPE_FLAC},
    errors::Error,
    formats::FormatOptions,
    io::MediaSourceStream,
    meta::{Limit, MetadataOptions},
    probe::Hint,
};

const MAX_SOURCE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_SECONDS: u64 = 4 * 60 * 60;
const FORMATS: &[&str] = &["wav", "wave", "mp3", "flac", "aac"];
const INVALID_AUDIO: &str =
    "This audio could not be decoded completely. Try exporting it as WAV, MP3, FLAC or ADTS AAC.";
const CLEANUP_FAILED: &str = "Some temporary import files could not be removed from local storage. Your original file is unchanged. Check local storage permissions, then try again.";
const TOO_LONG: &str =
    "This file exceeds the four-hour import limit. Your original file is unchanged.";
const UNSAFE_DECODER_ALLOCATION: &str =
    "This audio exceeds safe decoding limits. Export mono or stereo audio up to 192 kHz and try again.";

/// A failed or cancelled decode must not become a deceptively complete recording.
/// The original file is always retained; only a finished import enters recovery.
struct StagedImport {
    path: PathBuf,
    final_path: PathBuf,
}
impl StagedImport {
    fn cleanup(&self) -> Result<(), String> {
        // Do not erase mode ownership while private staging audio still exists.
        match std::fs::remove_file(&self.path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(CLEANUP_FAILED.into()),
        }
        if !self.final_path.try_exists().map_err(|_| CLEANUP_FAILED)? {
            match std::fs::remove_file(self.path.with_extension("options.json")) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err(CLEANUP_FAILED.into()),
            }
        }
        Ok(())
    }
}
impl Drop for StagedImport {
    fn drop(&mut self) {
        // Panic/unwind backup only; ordinary errors run fallible cleanup below.
        let _ = self.cleanup();
    }
}

fn open_source(source: &Path) -> Result<(File, Metadata), String> {
    let inspected = std::fs::symlink_metadata(source)
        .map_err(|_| "The selected file is no longer available")?;
    if !inspected.file_type().is_file()
        || inspected.len() == 0
        || inspected.len() > MAX_SOURCE_BYTES
    {
        return Err("Choose a regular audio file smaller than 2 GiB".into());
    }
    open_inspected_source(source, &inspected)
}

fn open_inspected_source(source: &Path, _inspected: &Metadata) -> Result<(File, Metadata), String> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // The path can change after inspection. No-follow blocks a substituted
        // symlink and nonblocking allows rejecting a substituted FIFO by fstat.
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows::Win32::Storage::FileSystem::{FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ};
        options
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0)
            .share_mode(FILE_SHARE_READ.0);
    }
    let file = options
        .open(source)
        .map_err(|_| "The selected file could not be opened safely")?;
    let opened = file
        .metadata()
        .map_err(|_| "The selected file could not be inspected")?;
    if !opened.file_type().is_file() || opened.len() == 0 || opened.len() > MAX_SOURCE_BYTES {
        return Err("Choose a regular audio file smaller than 2 GiB".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if _inspected.dev() != opened.dev() || _inspected.ino() != opened.ino() {
            return Err(
                "The selected file changed. Choose it again after it finishes saving.".into(),
            );
        }
    }
    Ok((file, opened))
}

pub(crate) fn import_file(
    source: &Path,
    directory: &Path,
    preferences: &RecordingPreferences,
    cancelled: impl Fn() -> bool,
) -> Result<String, String> {
    preferences.validate()?;
    let extension = source
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !source.is_absolute() || !FORMATS.contains(&extension.as_str()) {
        return Err("Choose a local WAV, MP3, FLAC or ADTS AAC file".into());
    }
    let (source_file, source_metadata) = open_source(source)?;
    std::fs::create_dir_all(directory).map_err(|_| "Could not prepare local audio storage")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))
            .map_err(|_| "Could not protect local audio storage")?;
    }
    let id = uuid::Uuid::new_v4().to_string();
    let staged = StagedImport {
        path: directory.join(format!("{id}.importing")),
        final_path: directory.join(format!("{id}.pcm")),
    };
    let result = (|| -> Result<String, String> {
        preferences.save_journal(&staged.path)?;
        let mut journal =
            Journal::create(&staged.path, 16000, 1).map_err(|_| "Could not save imported audio")?;
        let reader = source_file
            .try_clone()
            .map_err(|_| "The selected file could not be read")?;
        decode(reader, &extension, &cancelled, |samples| {
            journal
                .append(samples, 16000, 1)
                .map_err(|_| "Could not save imported audio".to_string())
        })?;
        let final_metadata = source_file
            .metadata()
            .map_err(|_| "The selected file could not be inspected")?;
        if final_metadata.len() != source_metadata.len()
            || final_metadata
                .modified()
                .map_err(|_| "The selected file could not be inspected")?
                != source_metadata
                    .modified()
                    .map_err(|_| "The selected file could not be inspected")?
        {
            return Err(
                "The selected file changed while importing. Retry after it finishes saving.".into(),
            );
        }
        journal
            .finish()
            .map_err(|_| "Could not finish saving imported audio")?;
        drop(journal);
        if cancelled() {
            return Err("Import cancelled. Your original file is unchanged.".into());
        }
        std::fs::rename(&staged.path, &staged.final_path)
            .map_err(|_| "Could not finish saving imported audio")?;
        Ok(id)
    })();
    // The writer was dropped before cleanup (required on Windows). Cleanup
    // failures are surfaced, never silently presented as a clean cancellation.
    if let Err(error) = result {
        return match staged.cleanup() {
            Ok(()) => Err(error),
            Err(cleanup) => Err(format!("{error} {cleanup}")),
        };
    }
    result
}

/// Called only by an exclusive recovery owner before it starts decoding. A crash can leave a
/// private staging file; it is not a recording and must never be transcribed.
pub(crate) fn clean_interrupted_imports(directory: &Path) -> Result<(), String> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err("Could not read local audio storage".into()),
    };
    for entry in entries {
        let entry = entry.map_err(|_| "Could not read local audio storage")?;
        let path = entry.path();
        if path.extension().and_then(|x| x.to_str()) != Some("importing")
            || !path
                .file_stem()
                .and_then(|x| x.to_str())
                .is_some_and(|x| uuid::Uuid::parse_str(x).is_ok())
            || !entry
                .file_type()
                .map_err(|_| "Could not inspect local audio storage")?
                .is_file()
        {
            continue;
        }
        StagedImport {
            final_path: path.with_extension("pcm"),
            path,
        }
        .cleanup()?;
    }
    Ok(())
}

fn decode(
    source: File,
    extension: &str,
    cancelled: &impl Fn() -> bool,
    mut write: impl FnMut(&[f32]) -> Result<(), String>,
) -> Result<(), String> {
    let mut hint = Hint::new();
    hint.with_extension(extension);
    let mut format = symphonia::default::get_probe()
        .format(
            &hint,
            MediaSourceStream::new(Box::new(source), Default::default()),
            &FormatOptions::default(),
            &MetadataOptions {
                limit_metadata_bytes: Limit::Maximum(4096),
                limit_visual_bytes: Limit::Maximum(0),
            },
        )
        .map_err(|_| INVALID_AUDIO)?
        .format;
    let track = format.default_track().ok_or(INVALID_AUDIO)?;
    let track_id = track.id;
    let expected_frames = track.codec_params.n_frames;
    let declared_time_base = track.codec_params.time_base;
    // Probe identifies file contents independently of the extension hint. Use
    // detected codecs for completeness rules, never a user-controlled suffix.
    // The registry's PCM descriptors include signed/unsigned, float, A/mu-law
    // and every supported endian/width variant.
    let codec = track.codec_params.codec;
    let exact_duration = codec == CODEC_TYPE_FLAC
        || symphonia::default::get_codecs()
            .get_codec(codec)
            .is_some_and(|descriptor| descriptor.short_name.starts_with("pcm_"));
    let declared_duration = expected_frames.and_then(|expected| {
        declared_time_base
            .filter(|base| base.numer > 0 && base.denom > 0)
            .map(|base| (expected as u128 * base.numer as u128, base.denom as u128))
            .or_else(|| {
                track
                    .codec_params
                    .sample_rate
                    .filter(|rate| *rate > 0)
                    .map(|rate| (expected as u128, rate as u128))
            })
    });
    // MP3/ADTS bitrate estimates remain bounded by actual decoded samples.
    if exact_duration
        && declared_duration.is_some_and(|(numer, denom)| numer > MAX_SECONDS as u128 * denom)
    {
        return Err(TOO_LONG.into());
    }
    validate_decoder_allocation(&track.codec_params)?;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|_| INVALID_AUDIO)?;
    let mut rate = None;
    let mut channels = None;
    let mut frames = 0u64;
    let mut mono = Vec::new();
    loop {
        if cancelled() {
            return Err("Import cancelled. Your original file is unchanged.".into());
        }
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(Error::IoError(error)) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
                break
            }
            Err(_) => return Err(INVALID_AUDIO.into()),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = decoder.decode(&packet).map_err(|_| INVALID_AUDIO)?;
        let spec = *decoded.spec();
        let channel_count = spec.channels.count();
        if spec.rate == 0
            || spec.rate > 192_000
            || !(1..=2).contains(&channel_count)
            || rate.is_some_and(|previous| previous != spec.rate)
            || channels.is_some_and(|previous| previous != channel_count)
            || decoded.capacity() > spec.rate as usize * 10
        {
            return Err("Use mono or stereo audio with a fixed sample rate up to 192 kHz".into());
        }
        rate = Some(spec.rate);
        channels = Some(channel_count);
        frames += decoded.frames() as u64;
        if frames > MAX_SECONDS * spec.rate as u64 {
            return Err(TOO_LONG.into());
        }
        let mut buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        buffer.copy_interleaved_ref(decoded);
        if buffer.samples().iter().any(|sample| !sample.is_finite()) {
            return Err(INVALID_AUDIO.into());
        }
        mono.extend(downmix_interleaved_to_mono(buffer.samples(), channel_count));
        let block = spec.rate as usize * 60;
        while mono.len() >= block {
            write_normalized(&mono[..block], spec.rate, &mut write)?;
            mono.drain(..block);
        }
    }
    if frames == 0 {
        return Err("The file contains no audio".into());
    }
    if let (Some((numer, denom)), Some(rate)) = (declared_duration, rate) {
        // Container timestamps can be track ticks rather than sample frames.
        // PCM/FLAC must contain the exact declared duration.
        if exact_duration && frames as u128 * denom != numer * rate as u128 {
            return Err(INVALID_AUDIO.into());
        }
    }
    if !mono.is_empty() {
        write_normalized(&mono, rate.ok_or(INVALID_AUDIO)?, &mut write)?;
    }
    Ok(())
}

fn validate_decoder_allocation(params: &CodecParameters) -> Result<(), String> {
    if params
        .sample_rate
        .is_some_and(|rate| rate == 0 || rate > 192_000)
        || params
            .channels
            .is_some_and(|channels| !(1..=2).contains(&channels.count()))
        || params
            .channel_layout
            .is_some_and(|layout| !(1..=2).contains(&layout.into_channels().count()))
        || params.max_frames_per_packet.is_some_and(|frames| {
            frames == 0 || frames > params.sample_rate.unwrap_or(192_000) as u64 * 10
        })
    {
        return Err(UNSAFE_DECODER_ALLOCATION.into());
    }
    Ok(())
}

fn write_normalized(
    samples: &[f32],
    rate: u32,
    write: &mut impl FnMut(&[f32]) -> Result<(), String>,
) -> Result<(), String> {
    let normalized = resample_to_16khz_vad_quality(samples, rate);
    let expected = (samples.len() as u64 * 16000).div_ceil(rate as u64) as usize;
    // The shared resampler retains legacy fallback behavior on failure; imports
    // must not label unresampled samples as 16 kHz and change speed/duration.
    if normalized.len().abs_diff(expected) > 1 {
        return Err("Could not normalize this audio. Try exporting a 16 kHz WAV file.".into());
    }
    write(&normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wav(path: &Path, rate: u32, channels: u16, frames: u32) {
        let mut writer = hound::WavWriter::create(
            path,
            hound::WavSpec {
                channels,
                sample_rate: rate,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for _ in 0..frames * channels as u32 {
            writer.write_sample(8192i16).unwrap();
        }
        writer.finalize().unwrap();
    }
    #[test]
    fn imports_complete_audio_with_pinned_mode_without_changing_source() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("private-name.wav");
        wav(&source, 48000, 2, 48000 * 61);
        let bytes = std::fs::read(&source).unwrap();
        let destination = dir.path().join("recovery");
        let options = RecordingPreferences {
            mode: crate::RecordingMode::Meeting,
            meeting_model: Some(crate::recordings::options::MeetingModel {
                provider: "openai".into(),
                model: "test".into(),
                use_managed: false,
            }),
        };
        let id = import_file(&source, &destination, &options, || false).unwrap();
        let path = destination.join(format!("{id}.pcm"));
        assert_eq!(std::fs::read(source).unwrap(), bytes);
        assert_eq!(std::fs::metadata(&path).unwrap().len(), 14 + 16000 * 61 * 4);
        assert_eq!(
            RecordingPreferences::load_journal(&path).unwrap().mode,
            crate::RecordingMode::Meeting
        );
        assert!(!path.with_extension("importing").exists());
        let (_, _, samples) =
            crate::audio_capture::journal::read_chunk(&path, 16000 * 60, 16000).unwrap();
        assert_eq!(samples.len(), 16000);
        assert!(samples.iter().all(|sample| sample.is_finite()));
    }
    #[test]
    fn cancellation_and_invalid_media_never_leave_a_partial_recovery() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("audio.wav");
        wav(&source, 16000, 1, 16000);
        let destination = dir.path().join("recovery");
        assert!(
            import_file(&source, &destination, &Default::default(), || true)
                .unwrap_err()
                .contains("cancelled")
        );
        assert_eq!(std::fs::read_dir(&destination).unwrap().count(), 0);
        std::fs::write(&source, b"not audio").unwrap();
        assert!(import_file(&source, &destination, &Default::default(), || false).is_err());
        assert_eq!(std::fs::read_dir(&destination).unwrap().count(), 0);
        assert_eq!(std::fs::read(source).unwrap(), b"not audio");
    }
    #[test]
    fn cleanup_only_removes_known_staging_files_and_keeps_completed_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let id = uuid::Uuid::new_v4();
        let path = dir.path().join(format!("{id}.importing"));
        std::fs::write(&path, b"partial").unwrap();
        std::fs::write(path.with_extension("options.json"), b"mode").unwrap();
        std::fs::write(dir.path().join("unrelated.importing"), b"other").unwrap();
        clean_interrupted_imports(dir.path()).unwrap();
        assert!(!path.exists());
        assert!(!path.with_extension("options.json").exists());
        assert!(dir.path().join("unrelated.importing").exists());
    }

    #[test]
    fn imports_existing_mp3_fixture_without_playback_or_network() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("sound.mp3");
        let original = include_bytes!("assets/start.mp3");
        std::fs::write(&source, original).unwrap();
        let destination = dir.path().join("recovery");
        let id = import_file(&source, &destination, &Default::default(), || false).unwrap();
        let path = destination.join(format!("{id}.pcm"));
        let (rate, channels, samples) =
            crate::audio_capture::journal::read_chunk(&path, 0, 16000).unwrap();
        assert_eq!((rate, channels), (16000, 1));
        assert!(!samples.is_empty());
        assert!(samples.iter().all(|sample| sample.is_finite()));
        assert_eq!(std::fs::read(&source).unwrap(), original);
    }

    #[test]
    fn imports_synthetic_supported_container_fixtures_completely() {
        let fixtures: &[(&str, &[u8])] = &[
            (
                "tone.flac",
                include_bytes!("assets/import-fixtures/tone.flac"),
            ),
            (
                "tone.aac",
                include_bytes!("assets/import-fixtures/tone.aac"),
            ),
        ];
        for &(name, bytes) in fixtures {
            let directory = tempfile::tempdir().unwrap();
            let source = directory.path().join(name);
            let destination = directory.path().join("recovery");
            std::fs::write(&source, bytes).unwrap();
            let id = import_file(&source, &destination, &Default::default(), || false)
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            let (_, _, samples) = crate::audio_capture::journal::read_chunk(
                &destination.join(format!("{id}.pcm")),
                0,
                16000,
            )
            .unwrap();
            assert!((1600..8000).contains(&samples.len()), "{name}");
            assert!(samples.iter().all(|sample| sample.is_finite()), "{name}");
            assert!(samples.iter().any(|sample| sample.abs() > 0.001), "{name}");
            assert_eq!(std::fs::read(&source).unwrap(), bytes, "{name}");
        }
    }

    #[test]
    fn unsupported_containers_are_rejected_even_with_a_supported_extension() {
        let fixtures: &[(&str, &[u8])] = &[
            (
                "tone.aiff",
                include_bytes!("assets/import-fixtures/tone.aiff"),
            ),
            (
                "tone.ogg",
                include_bytes!("assets/import-fixtures/tone.ogg"),
            ),
            (
                "tone-aac.m4a",
                include_bytes!("assets/import-fixtures/tone-aac.m4a"),
            ),
            (
                "tone-alac.m4a",
                include_bytes!("assets/import-fixtures/tone-alac.m4a"),
            ),
        ];
        for &(name, original) in fixtures {
            let directory = tempfile::tempdir().unwrap();
            let source = directory.path().join(name);
            std::fs::write(&source, original).unwrap();
            let destination = directory.path().join("recovery");
            assert!(
                import_file(&source, &destination, &Default::default(), || false)
                    .unwrap_err()
                    .contains("Choose a local")
            );
            assert!(!destination.exists());
            assert_eq!(std::fs::read(&source).unwrap(), original);
            // Disabled probe/codec features must also reject the contents;
            // changing the suffix cannot reach the unsafe format readers.
            let renamed = source.with_extension("mp3");
            std::fs::rename(&source, &renamed).unwrap();
            assert!(
                import_file(&renamed, &destination, &Default::default(), || false).is_err(),
                "{name}"
            );
            assert_eq!(std::fs::read_dir(destination).unwrap().count(), 0, "{name}");
            assert_eq!(std::fs::read(renamed).unwrap(), original, "{name}");
        }
    }

    #[test]
    fn failed_staging_cleanup_retains_mode_and_is_retryable() {
        let dir = tempfile::tempdir().unwrap();
        let staged = StagedImport {
            path: dir.path().join("partial.importing"),
            final_path: dir.path().join("partial.pcm"),
        };
        // A directory at the exact staging path injects a deterministic delete
        // failure without relying on permissions/root behavior.
        std::fs::create_dir(&staged.path).unwrap();
        std::fs::write(staged.path.with_extension("options.json"), b"mode").unwrap();
        assert!(staged
            .cleanup()
            .unwrap_err()
            .contains("could not be removed"));
        assert!(staged.path.with_extension("options.json").exists());
        std::fs::remove_dir(&staged.path).unwrap();
        staged.cleanup().unwrap();
        assert!(!staged.path.with_extension("options.json").exists());
    }

    #[test]
    fn staging_cleanup_never_removes_completed_audio_or_its_mode() {
        let dir = tempfile::tempdir().unwrap();
        let staged = StagedImport {
            path: dir.path().join("finished.importing"),
            final_path: dir.path().join("finished.pcm"),
        };
        std::fs::write(&staged.path, b"leftover staging").unwrap();
        std::fs::write(&staged.final_path, b"complete").unwrap();
        std::fs::write(staged.path.with_extension("options.json"), b"mode").unwrap();
        staged.cleanup().unwrap();
        assert!(!staged.path.exists());
        assert_eq!(std::fs::read(&staged.final_path).unwrap(), b"complete");
        assert_eq!(
            std::fs::read(staged.path.with_extension("options.json")).unwrap(),
            b"mode"
        );
    }

    #[test]
    fn rejects_truncated_wav_and_declared_overlong_audio_without_publishing() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("audio.wav");
        let destination = dir.path().join("recovery");
        wav(&source, 8000, 1, 10);
        let mut bytes = std::fs::read(&source).unwrap();
        bytes.truncate(bytes.len() - 2);
        std::fs::write(&source, &bytes).unwrap();
        assert!(import_file(&source, &destination, &Default::default(), || false).is_err());
        assert_eq!(std::fs::read_dir(&destination).unwrap().count(), 0);
        assert_eq!(std::fs::read(&source).unwrap(), bytes);

        wav(&source, 8000, 1, 10);
        let mut bytes = std::fs::read(&source).unwrap();
        let data_offset = bytes
            .windows(4)
            .position(|window| window == b"data")
            .unwrap()
            + 4;
        let declared_data_bytes = ((MAX_SECONDS * 8000 + 1) * 2) as u32;
        bytes[data_offset..data_offset + 4].copy_from_slice(&declared_data_bytes.to_le_bytes());
        let declared_riff_bytes = declared_data_bytes + data_offset as u32 - 4;
        bytes[4..8].copy_from_slice(&declared_riff_bytes.to_le_bytes());
        std::fs::write(&source, &bytes).unwrap();
        assert!(
            import_file(&source, &destination, &Default::default(), || false)
                .unwrap_err()
                .contains("four-hour")
        );
        assert_eq!(std::fs::read_dir(&destination).unwrap().count(), 0);
        assert_eq!(std::fs::read(&source).unwrap(), bytes);
    }

    #[test]
    fn completeness_follows_detected_audio_not_filename_extension() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("renamed.mp3");
        let destination = directory.path().join("recovery");
        // More than one decoder packet is needed to catch the old bug: a
        // truncated WAV renamed .mp3 could publish its complete packet prefix.
        wav(&source, 16000, 1, 4000);
        let mut bytes = std::fs::read(&source).unwrap();
        bytes.truncate(bytes.len() - 2);
        std::fs::write(&source, &bytes).unwrap();
        assert!(import_file(&source, &destination, &Default::default(), || false).is_err());
        assert_eq!(std::fs::read_dir(&destination).unwrap().count(), 0);
        assert_eq!(std::fs::read(&source).unwrap(), bytes);

        // A real MP3 with a .wav suffix must not inherit exact PCM frame rules.
        let renamed_mp3 = directory.path().join("renamed.wav");
        std::fs::write(&renamed_mp3, include_bytes!("assets/start.mp3")).unwrap();
        assert!(import_file(&renamed_mp3, &destination, &Default::default(), || false).is_ok());
    }

    #[test]
    fn predecoder_allocation_limits_reject_oversized_buffers_without_allocating() {
        let make_params = |frames: u64, rate: u32| {
            let mut params = CodecParameters::new();
            params
                .with_sample_rate(rate)
                .with_max_frames_per_packet(frames);
            params
        };
        assert!(validate_decoder_allocation(&make_params(4096, 16000)).is_ok());
        for params in [
            make_params(u64::MAX, 16000),
            make_params(160001, 16000),
            make_params(0, 16000),
            make_params(4096, 0),
            make_params(4096, 192001),
        ] {
            assert!(validate_decoder_allocation(&params).is_err());
        }
        let mut surround = make_params(4096, 16000);
        surround.with_channels(
            symphonia::core::audio::Channels::FRONT_LEFT
                | symphonia::core::audio::Channels::FRONT_RIGHT
                | symphonia::core::audio::Channels::FRONT_CENTRE,
        );
        assert!(validate_decoder_allocation(&surround).is_err());
    }

    #[test]
    fn source_size_limit_rejects_sparse_oversized_file_before_decoding() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("large.wav");
        File::create(&source)
            .unwrap()
            .set_len(MAX_SOURCE_BYTES + 1)
            .unwrap();
        assert!(open_source(&source).is_err());
        assert_eq!(
            std::fs::metadata(&source).unwrap().len(),
            MAX_SOURCE_BYTES + 1
        );
    }

    #[cfg(unix)]
    #[test]
    fn substituted_symlink_fifo_and_regular_file_are_rejected_after_inspection() {
        use std::os::unix::{ffi::OsStrExt, fs::symlink};
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("selected.wav");
        let original = dir.path().join("original.wav");
        wav(&source, 16000, 1, 10);
        let inspected = std::fs::symlink_metadata(&source).unwrap();
        std::fs::rename(&source, &original).unwrap();
        symlink(&original, &source).unwrap();
        assert!(open_inspected_source(&source, &inspected).is_err());
        std::fs::remove_file(&source).unwrap();

        let fifo = std::ffi::CString::new(source.as_os_str().as_bytes()).unwrap();
        // SAFETY: owned temporary path is NUL-terminated and mode is owner-only.
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        assert!(open_inspected_source(&source, &inspected).is_err());
        std::fs::remove_file(&source).unwrap();

        wav(&source, 16000, 1, 10);
        assert!(open_inspected_source(&source, &inspected).is_err());
        assert!(open_source(&original).is_ok());
    }
}
