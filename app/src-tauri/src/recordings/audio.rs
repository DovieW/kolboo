//! An owned transcription source: short dictation in memory, saved meetings on
//! disk. Consumers can inspect metadata without reading an entire recording.
use std::{
    fs::File,
    io::{BufReader, Cursor, Read, Seek},
    path::PathBuf,
    sync::Arc,
};

pub(crate) trait AudioReader: Read + Seek + Send {}
impl<T: Read + Seek + Send> AudioReader for T {}

struct SharedBytes(Arc<Vec<u8>>);
impl AsRef<[u8]> for SharedBytes {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Clone)]
pub(crate) enum TranscriptionAudio {
    Memory(Arc<Vec<u8>>),
    File {
        path: PathBuf,
        bytes: usize,
        modified: std::time::SystemTime,
        duration: f64,
    },
}

impl From<Vec<u8>> for TranscriptionAudio {
    fn from(bytes: Vec<u8>) -> Self {
        Self::Memory(Arc::new(bytes))
    }
}

impl From<Arc<Vec<u8>>> for TranscriptionAudio {
    fn from(bytes: Arc<Vec<u8>>) -> Self {
        Self::Memory(bytes)
    }
}

impl TranscriptionAudio {
    pub fn open(path: PathBuf) -> Result<Self, String> {
        let file = File::open(&path).map_err(|_| "Saved audio could not be read")?;
        let metadata = file
            .metadata()
            .map_err(|_| "Saved audio could not be read")?;
        let reader = hound::WavReader::new(BufReader::new(file))
            .map_err(|_| "Saved audio could not be read")?;
        let bytes = metadata.len();
        let modified = metadata
            .modified()
            .map_err(|_| "Saved audio could not be verified")?;
        if reader.spec().sample_rate == 0 {
            return Err("Invalid saved audio sample rate".into());
        }
        Ok(Self::File {
            path,
            bytes: usize::try_from(bytes).map_err(|_| "Recording too large")?,
            modified,
            duration: reader.duration() as f64 / reader.spec().sample_rate as f64,
        })
    }
    pub fn len(&self) -> usize {
        match self {
            Self::Memory(bytes) => bytes.len(),
            Self::File { bytes, .. } => *bytes,
        }
    }
    pub fn duration(&self) -> Option<f64> {
        match self {
            Self::Memory(bytes) => crate::stats::wav_duration_secs(bytes),
            Self::File { duration, .. } => Some(*duration),
        }
    }
    pub fn memory(&self) -> Option<&Arc<Vec<u8>>> {
        match self {
            Self::Memory(bytes) => Some(bytes),
            Self::File { .. } => None,
        }
    }
    pub fn reader(&self) -> Result<Box<dyn AudioReader>, String> {
        match self {
            Self::Memory(bytes) => Ok(Box::new(Cursor::new(SharedBytes(bytes.clone())))),
            Self::File {
                path,
                bytes,
                modified,
                ..
            } => {
                let file = File::open(path).map_err(|_| "Saved audio could not be read")?;
                let metadata = file
                    .metadata()
                    .map_err(|_| "Saved audio could not be read")?;
                if metadata.len() != *bytes as u64
                    || metadata
                        .modified()
                        .map_err(|_| "Saved audio could not be verified")?
                        != *modified
                {
                    return Err("Saved audio changed; retry transcription".into());
                }
                Ok(Box::new(BufReader::new(file)))
            }
        }
    }
}

impl super::RecordingStore {
    pub fn transcription_audio(&self, id: &str) -> Result<TranscriptionAudio, String> {
        let path = self
            .wav_path_if_exists(id)?
            .ok_or("No saved audio is available")?
            .canonicalize()
            .map_err(|_| "Could not locate recording")?;
        let directory = self
            .dir
            .canonicalize()
            .map_err(|_| "Could not locate recordings")?;
        if path.parent() != Some(directory.as_path()) {
            return Err("Recording is outside its storage directory".into());
        }
        TranscriptionAudio::open(path)
    }
}

#[cfg(test)]
#[path = "tests/audio.rs"]
mod tests;
