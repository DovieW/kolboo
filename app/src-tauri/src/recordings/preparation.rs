//! Crash-safe normalization. A completion stamp is published only after the
//! normalized WAV is synced and atomically installed; retries reuse that file.
use super::{options::RecordingPreferences, RecordingStore};
use serde::{Deserialize, Serialize};
use std::{
    fs::Metadata,
    io::{BufWriter, Write},
    path::Path,
};
use tokio_util::sync::CancellationToken;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Stamp {
    bytes: u64,
    modified_nanos: u128,
}

impl Stamp {
    fn read(path: &Path) -> Result<Self, String> {
        let metadata: Metadata =
            std::fs::metadata(path).map_err(|_| "Could not read saved audio")?;
        Ok(Self {
            bytes: metadata.len(),
            modified_nanos: metadata
                .modified()
                .map_err(|_| "Could not verify saved audio")?
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| "Could not verify saved audio")?
                .as_nanos(),
        })
    }
}

#[derive(Serialize, Deserialize)]
struct Prepared {
    version: u8,
    source: Stamp,
    wav: Stamp,
}

impl RecordingStore {
    /// The caller owns recovery for the entire operation, including this worker.
    pub fn prepare_journal(
        &self,
        id: &str,
        source: &Path,
        options: &RecordingPreferences,
        cancel: &CancellationToken,
    ) -> Result<(), String> {
        if !Self::is_safe_request_id(id) {
            return Err("Invalid recording id".into());
        }
        if cancel.is_cancelled() {
            return Err("Transcription cancelled. Your audio is saved.".into());
        }
        let source_stamp = Stamp::read(source)?;
        let destination = self.path_for_id(id);
        let marker = self.dir.join(format!("{id}.prepared.json"));
        let reusable = self.fs.metadata(&marker).is_ok_and(|m| m.len() <= 4096)
            && self
                .fs
                .read(&marker)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Prepared>(&bytes).ok())
                .is_some_and(|prepared| {
                    prepared.version == 1
                        && prepared.source == source_stamp
                        && Stamp::read(&destination).is_ok_and(|stamp| stamp == prepared.wav)
                });
        self.save_options(id, options)?;
        if reusable {
            return Ok(());
        }

        let mut temporary = tempfile::Builder::new()
            .prefix(".kolboo-normalize-")
            .suffix(".tmp")
            .tempfile_in(&self.dir)
            .map_err(|_| "Could not prepare recording; source is retained")?;
        {
            let mut output = BufWriter::new(temporary.as_file_mut());
            crate::audio_capture::journal::final_wav_to(source, &mut output, 0, || {
                cancel.is_cancelled()
            })
            .map_err(|_| "Audio preparation failed or was cancelled; source is retained")?;
            output
                .flush()
                .map_err(|_| "Could not flush prepared recording")?;
        }
        temporary
            .as_file()
            .sync_all()
            .map_err(|_| "Could not sync prepared recording")?;
        self.commit_prepared_audio(id, source, source_stamp, temporary, cancel)
    }

    /// Publication is a separate critical section: cancellation or a changed
    /// source must win even when normalization has just finished successfully.
    fn commit_prepared_audio(
        &self,
        id: &str,
        source: &Path,
        source_stamp: Stamp,
        temporary: tempfile::NamedTempFile,
        cancel: &CancellationToken,
    ) -> Result<(), String> {
        let _guard = self
            .media_write
            .lock()
            .map_err(|_| "Recording store unavailable")?;
        if cancel.is_cancelled() || Stamp::read(source)? != source_stamp {
            return Err("Audio preparation interrupted; source is retained".into());
        }
        self.remove_waveform(id)?;
        let destination = self.path_for_id(id);
        temporary
            .persist(&destination)
            .map_err(|_| "Could not save prepared recording")?;
        let prepared = Prepared {
            version: 1,
            source: source_stamp,
            wav: Stamp::read(&destination)?,
        };
        let bytes =
            serde_json::to_vec(&prepared).map_err(|_| "Could not save preparation progress")?;
        self.fs
            .write_private(&self.dir.join(format!("{id}.prepared.json")), &bytes)
            .map_err(|_| "Could not save preparation progress")?;
        Ok(())
    }
}

/// Only called while constructing the singleton store, before any workers start.
/// Normal failures use tempfile's RAII cleanup; this removes crash leftovers,
/// never a final recording or an arbitrary file in the recordings directory.
pub(super) fn remove_interrupted_preparations(dir: &Path, fs: &dyn crate::fs::Fs) {
    let Ok(entries) = fs.read_dir(dir) else {
        return;
    };
    for path in entries {
        let owned = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_prefix(".kolboo-normalize-"))
            .and_then(|name| name.strip_suffix(".tmp"))
            .is_some_and(|name| {
                name.len() == 6 && name.bytes().all(|byte| byte.is_ascii_alphanumeric())
            });
        if owned && fs.metadata(&path).is_ok_and(|meta| meta.is_file()) {
            let _ = fs.remove_file(&path);
        }
    }
}

#[cfg(test)]
#[path = "tests/preparation.rs"]
mod tests;
