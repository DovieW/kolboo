use super::*;
use crate::audio_capture::journal::Journal;

#[test]
fn startup_removes_only_recognized_interrupted_preparation_files() {
    let dir = tempfile::tempdir().unwrap();
    let recordings = dir.path().join("recordings");
    std::fs::create_dir(&recordings).unwrap();
    let abandoned = recordings.join(".kolboo-normalize-aB0123.tmp");
    std::fs::write(&abandoned, b"incomplete normalized audio").unwrap();
    let preserved = [
        "meeting.wav",
        "meeting.pcm",
        ".tmpABCDEF",
        ".kolboo-normalize-not-owned.tmp",
    ];
    for name in preserved {
        std::fs::write(recordings.join(name), b"keep").unwrap();
    }
    // Windows filesystems commonly fold case, so this must not alias `abandoned`.
    let directory = recordings.join(".kolboo-normalize-Cd4567.tmp");
    std::fs::create_dir(&directory).unwrap();
    let store = RecordingStore::new(dir.path().to_owned());
    assert!(!abandoned.exists());
    for name in preserved {
        assert!(recordings.join(name).exists(), "removed {name}");
    }
    assert!(directory.is_dir());
    assert_eq!(store.stats().unwrap().count, 1);
    remove_interrupted_preparations(&dir.path().join("missing"), &crate::fs::RealFs);
}

#[test]
fn preparation_reuses_completed_audio_and_rebuilds_changed_or_incomplete_files() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("meeting.pcm");
    let mut journal = Journal::create(&source, 16000, 1).unwrap();
    journal.append(&vec![0.25; 16000], 16000, 1).unwrap();
    journal.finish().unwrap();
    let store = RecordingStore::new(dir.path().to_owned());
    let cancel = CancellationToken::new();
    let options = RecordingPreferences::default();
    store
        .prepare_journal("meeting-final", &source, &options, &cancel)
        .unwrap();
    let wav = store.path_for_id("meeting-final");
    let stamp = Stamp::read(&wav).unwrap();
    let original = std::fs::read(&wav).unwrap();
    store.waveform("meeting-final").unwrap();
    store
        .prepare_journal("meeting-final", &source, &options, &cancel)
        .unwrap();
    assert_eq!(
        Stamp::read(&wav).unwrap(),
        stamp,
        "retry rewrote normalized audio"
    );
    assert!(store.dir.join("meeting-final.waveform.json").exists());
    std::fs::write(&wav, b"incomplete").unwrap();
    store
        .prepare_journal("meeting-final", &source, &options, &cancel)
        .unwrap();
    assert_eq!(std::fs::read(&wav).unwrap(), original);
    journal.append(&vec![0.5; 16000], 16000, 1).unwrap();
    journal.finish().unwrap();
    store
        .prepare_journal("meeting-final", &source, &options, &cancel)
        .unwrap();
    assert_eq!(hound::WavReader::open(&wav).unwrap().duration(), 32000);
    let marker = store.dir.join("meeting-final.prepared.json");
    std::fs::write(&marker, b"partial json").unwrap();
    store
        .prepare_journal("meeting-final", &source, &options, &cancel)
        .unwrap();
    assert!(serde_json::from_slice::<Prepared>(&std::fs::read(&marker).unwrap()).is_ok());
    assert!(source.exists());
    assert!(store.delete_wav_if_exists("meeting-final").unwrap());
    assert!(!marker.exists());
    assert!(source.exists());
}

#[test]
fn invalid_or_cancelled_preparation_preserves_source_and_existing_audio() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("meeting.pcm");
    std::fs::write(&source, b"broken").unwrap();
    let store = RecordingStore::new(dir.path().to_owned());
    let cancel = CancellationToken::new();
    store.save_wav("meeting-final", b"original").unwrap();
    assert!(store
        .prepare_journal("../escape", &source, &Default::default(), &cancel)
        .is_err());
    assert!(store
        .prepare_journal("meeting-final", &source, &Default::default(), &cancel)
        .is_err());
    cancel.cancel();
    assert!(store
        .prepare_journal("meeting-final", &source, &Default::default(), &cancel)
        .is_err());
    assert_eq!(std::fs::read(&source).unwrap(), b"broken");
    assert_eq!(store.load_wav("meeting-final").unwrap(), b"original");
    assert!(!store.dir.join("meeting-final.prepared.json").exists());
    assert!(std::fs::read_dir(&store.dir).unwrap().all(|entry| !entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".kolboo-normalize-")));
}

#[test]
fn failed_install_keeps_source_and_existing_destination() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("meeting.pcm");
    let mut journal = Journal::create(&source, 16000, 1).unwrap();
    journal.append(&[0.2; 32], 16000, 1).unwrap();
    journal.finish().unwrap();
    let store = RecordingStore::new(dir.path().to_owned());
    let destination = store.path_for_id("meeting-final");
    std::fs::create_dir(&destination).unwrap();
    assert!(store
        .prepare_journal(
            "meeting-final",
            &source,
            &Default::default(),
            &CancellationToken::new()
        )
        .is_err());
    assert!(source.exists());
    assert!(destination.is_dir());
    assert!(!store.dir.join("meeting-final.prepared.json").exists());
    assert_eq!(
        std::fs::read_dir(&store.dir).unwrap().count(),
        2,
        "failed publish leaked a temporary audio file"
    );
}

#[test]
fn cancellation_or_source_change_wins_over_completed_preparation() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.pcm");
    std::fs::write(&source, b"original source").unwrap();
    let store = RecordingStore::new(dir.path().to_owned());
    store
        .save_wav("example", b"previous completed audio")
        .unwrap();
    for cancel_first in [true, false] {
        let stamp = Stamp::read(&source).unwrap();
        let mut temporary = tempfile::NamedTempFile::new_in(&store.dir).unwrap();
        temporary.write_all(b"newly prepared audio").unwrap();
        let temporary_path = temporary.path().to_owned();
        let token = CancellationToken::new();
        if cancel_first {
            token.cancel();
        } else {
            std::fs::write(&source, b"changed source").unwrap();
        }
        assert!(store
            .commit_prepared_audio("example", &source, stamp, temporary, &token)
            .is_err());
        assert_eq!(
            store.load_wav("example").unwrap(),
            b"previous completed audio"
        );
        assert!(!temporary_path.exists());
        assert!(source.exists());
    }
}
