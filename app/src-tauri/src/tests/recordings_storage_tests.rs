use crate::recordings::RecordingStore;
use std::fs;
use std::path::PathBuf;

fn make_temp_dir(label: &str) -> PathBuf {
    let id = uuid::Uuid::new_v4().to_string();
    let dir = std::env::temp_dir().join(format!("kolboo-test-{label}-{id}"));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

#[test]
fn test_recording_store_save_load_delete() {
    let dir = make_temp_dir("recordings");
    let store = RecordingStore::new(dir.clone());

    let id = "req-1";
    let data = vec![1u8, 2, 3, 4];

    store.save_wav(id, &data).expect("save wav");
    let loaded = store.load_wav(id).expect("load wav");
    assert_eq!(loaded, data);

    let path = store.wav_path_if_exists(id).expect("path if exists");
    assert!(path.is_some());

    let deleted = store.delete_wav_if_exists(id).expect("delete wav");
    assert!(deleted);
    let deleted_again = store.delete_wav_if_exists(id).expect("delete wav again");
    assert!(!deleted_again);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_recording_store_stats_and_validation() {
    let dir = make_temp_dir("recordings-stats");
    let store = RecordingStore::new(dir.clone());

    store.save_wav("req-1", &[1, 2]).expect("save wav 1");
    store.save_wav("req-2", &[3, 4, 5]).expect("save wav 2");

    let recordings_dir = store.directory().to_path_buf();
    fs::write(recordings_dir.join("note.txt"), b"hi").expect("write note");

    let stats = store.stats().expect("stats");
    assert_eq!(stats.count, 2);
    assert!(stats.bytes >= 5);

    let invalid = store.wav_path_if_exists("../bad");
    assert!(invalid.is_err());

    let _ = fs::remove_dir_all(&dir);
}
