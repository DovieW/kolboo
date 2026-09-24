use super::*;

#[test]
fn disk_and_memory_sources_have_identical_metadata_and_seekable_bytes() {
    let mut wav = Cursor::new(Vec::new());
    let mut writer = hound::WavWriter::new(
        &mut wav,
        hound::WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for _ in 0..16000 {
        writer.write_sample(123_i16).unwrap();
    }
    writer.finalize().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("recording.wav");
    std::fs::write(&path, wav.get_ref()).unwrap();
    let disk = TranscriptionAudio::open(path.clone()).unwrap();
    for source in [
        disk.clone(),
        TranscriptionAudio::from(wav.get_ref().clone()),
        TranscriptionAudio::from(Arc::new(wav.get_ref().clone())),
    ] {
        assert_eq!(source.len(), wav.get_ref().len());
        assert_eq!(source.duration(), Some(1.0));
        let mut reader = source.reader().unwrap();
        reader.seek(std::io::SeekFrom::Start(44)).unwrap();
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, wav.get_ref()[44..]);
    }
    assert!(disk.memory().is_none());
    assert!(TranscriptionAudio::from(vec![]).memory().is_some());
    assert_eq!(TranscriptionAudio::from(vec![]).duration(), None);
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1))
        .unwrap();
    assert!(
        disk.reader().is_err(),
        "same-size replacements must not reuse stale metadata"
    );
    std::fs::write(&path, b"damaged").unwrap();
    assert!(disk.reader().is_err());
    assert!(TranscriptionAudio::open(path).is_err());
    let mut invalid = wav.into_inner();
    invalid[24..28].copy_from_slice(&0_u32.to_le_bytes());
    // Keep the redundant bytes-per-second field consistent so this tests our
    // zero-rate guard, rather than failing Hound's earlier header validation.
    invalid[28..32].copy_from_slice(&0_u32.to_le_bytes());
    let path = dir.path().join("zero-rate.wav");
    std::fs::write(&path, invalid).unwrap();
    assert!(matches!(
        TranscriptionAudio::open(path),
        Err(message) if message == "Invalid saved audio sample rate"
    ));
}

#[test]
fn store_opens_only_owned_audio_and_supports_four_hour_metadata_without_decoding() {
    let dir = tempfile::tempdir().unwrap();
    let store = super::super::RecordingStore::new(dir.path().to_owned());
    assert!(store.transcription_audio("../escape").is_err());
    assert!(store.transcription_audio("missing").is_err());
    let frames: u32 = 16000 * 60 * 60 * 4;
    let mut header = Cursor::new(Vec::new());
    hound::WavWriter::new(
        &mut header,
        hound::WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap()
    .finalize()
    .unwrap();
    let bytes = header.get_mut();
    bytes[4..8].copy_from_slice(&(frames * 2 + 36).to_le_bytes());
    bytes[40..44].copy_from_slice(&(frames * 2).to_le_bytes());
    store.save_wav("long", bytes).unwrap();
    let path = store.wav_path_if_exists("long").unwrap().unwrap();
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(frames as u64 * 2 + 44)
        .unwrap();
    let audio = store.transcription_audio("long").unwrap();
    assert_eq!(audio.duration(), Some(14400.0));
    assert_eq!(audio.len(), frames as usize * 2 + 44);
    assert!(audio.memory().is_none());
    let mut stream = audio.reader().unwrap();
    stream.seek(std::io::SeekFrom::End(-2)).unwrap();
    let mut last = [1; 2];
    stream.read_exact(&mut last).unwrap();
    assert_eq!(last, [0, 0]);
    std::fs::remove_file(&path).unwrap();
    assert!(audio.reader().is_err());
    #[cfg(unix)]
    {
        let outside = dir.path().join("unowned.wav");
        std::fs::write(&outside, bytes).unwrap();
        std::os::unix::fs::symlink(outside, &path).unwrap();
        assert!(store.transcription_audio("long").is_err());
    }
}
