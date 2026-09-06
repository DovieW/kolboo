//! Append-only recovery audio. The capture worker writes here, never the audio callback.
//! Files contain a fixed header followed by little-endian float samples. A crash can
//! leave a partial final frame; readers truncate that frame rather than guessing.
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;

const MAGIC: &[u8; 8] = b"KOLPCM01";
const MAX_SECONDS: u64 = 4 * 60 * 60;
const MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug)]
pub struct Journal {
    file: File,
    rate: u32,
    channels: u16,
    bytes: u64,
    unsynced: u64,
    failed: bool,
}

impl Journal {
    pub fn create(path: &Path, rate: u32, channels: u16) -> io::Result<Self> {
        if rate == 0 || rate > 192_000 || channels == 0 || channels > 2 {
            return Err(io::Error::other("Unsupported recovery audio format"));
        }
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path)?;
        file.write_all(MAGIC)?;
        file.write_all(&rate.to_le_bytes())?;
        file.write_all(&channels.to_le_bytes())?;
        file.sync_all()?;
        Ok(Self {
            file,
            rate,
            channels,
            bytes: 0,
            unsynced: 0,
            failed: false,
        })
    }

    pub fn append(&mut self, samples: &[f32], rate: u32, channels: u16) -> io::Result<()> {
        if self.failed || rate != self.rate || channels != self.channels {
            self.failed = true;
            return Err(io::Error::other(
                "Recovery audio format changed or storage failed",
            ));
        }
        let count = samples.len() as u64 * 4;
        if self.bytes.saturating_add(count)
            > (MAX_SECONDS * self.rate as u64 * self.channels as u64 * 4).min(MAX_BYTES)
        {
            self.failed = true;
            return Err(io::Error::other("Recovery recording size limit reached"));
        }
        let bytes: Vec<u8> = samples
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        let result = self.file.write_all(&bytes).and_then(|()| {
            self.bytes += count;
            self.unsynced += count;
            if self.unsynced >= self.rate as u64 * self.channels as u64 * 4 {
                self.file.sync_data()?;
                self.unsynced = 0;
            }
            Ok(())
        });
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    pub fn finish(&self) -> io::Result<()> {
        if self.failed {
            return Err(io::Error::other(
                "Recovery storage failed; retained audio may be incomplete",
            ));
        }
        self.file.sync_all()
    }
}

/// Append-only progress is synced after each persisted transcription. A partial
/// trailing cursor left by a crash is ignored, just like a partial audio frame.
pub fn checkpoint(path: &Path, frame: u64) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path.with_extension("progress"))?;
    let length = file.metadata()?.len();
    file.set_len(length / 8 * 8)?;
    file.write_all(&frame.to_le_bytes())?;
    file.sync_all()
}

pub fn progress(path: &Path) -> io::Result<u64> {
    use std::io::{Seek, SeekFrom};
    let mut file = match File::open(path.with_extension("progress")) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e),
    };
    let length = file.metadata()?.len() / 8 * 8;
    if length == 0 {
        return Ok(0);
    }
    file.seek(SeekFrom::Start(length - 8))?;
    let mut bytes = [0; 8];
    file.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

pub fn read_chunk(path: &Path, start_frame: u64, frames: u32) -> io::Result<(u32, u16, Vec<f32>)> {
    use std::io::{Seek, SeekFrom};
    let mut file = File::open(path)?;
    let mut header = [0u8; 14];
    file.read_exact(&mut header)?;
    let rate = u32::from_le_bytes(header[8..12].try_into().unwrap());
    let channels = u16::from_le_bytes(header[12..14].try_into().unwrap());
    if &header[..8] != MAGIC
        || rate == 0
        || rate > 192_000
        || !(1..=2).contains(&channels)
        || frames > rate * 60
    {
        return Err(io::Error::other(
            "Invalid recovery audio header or chunk size",
        ));
    }
    let offset = start_frame
        .checked_mul(channels as u64 * 4)
        .and_then(|x| x.checked_add(14))
        .ok_or_else(|| io::Error::other("Invalid recovery offset"))?;
    file.seek(SeekFrom::Start(offset))?;
    let mut bytes = Vec::new();
    file.take(frames as u64 * channels as u64 * 4)
        .read_to_end(&mut bytes)?;
    bytes.truncate(bytes.len() / (channels as usize * 4) * channels as usize * 4);
    let samples = bytes
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    Ok((rate, channels, samples))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn progress_survives_partial_checkpoint_and_resumes() {
        let path =
            std::env::temp_dir().join(format!("kolboo-progress-{}.pcm", uuid::Uuid::new_v4()));
        assert_eq!(progress(&path).unwrap(), 0);
        checkpoint(&path, 480_000).unwrap();
        OpenOptions::new()
            .append(true)
            .open(path.with_extension("progress"))
            .unwrap()
            .write_all(&[1, 2])
            .unwrap();
        assert_eq!(progress(&path).unwrap(), 480_000);
        checkpoint(&path, 960_000).unwrap();
        assert_eq!(progress(&path).unwrap(), 960_000);
        std::fs::remove_file(path.with_extension("progress")).unwrap();
    }

    #[test]
    fn duration_limit_is_independent_of_sample_rate() {
        let path = std::env::temp_dir().join(format!("kolboo-limit-{}.pcm", uuid::Uuid::new_v4()));
        let mut journal = Journal::create(&path, 16000, 1).unwrap();
        journal.bytes = MAX_SECONDS * 16000 * 4;
        assert!(journal.append(&[0.0], 16000, 1).is_err());
        drop(journal);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn preserves_samples_and_ignores_a_partial_crash_frame() {
        let path =
            std::env::temp_dir().join(format!("kolboo-journal-{}.pcm", uuid::Uuid::new_v4()));
        let mut journal = Journal::create(&path, 16000, 1).unwrap();
        journal.append(&[0.25, -0.5], 16000, 1).unwrap();
        journal.finish().unwrap();
        drop(journal);
        OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(&[1, 2])
            .unwrap();
        assert_eq!(read_chunk(&path, 0, 100).unwrap().2, vec![0.25, -0.5]);
        assert!(Journal::create(&path, 16000, 1).is_err());
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn format_change_fails_closed() {
        let path =
            std::env::temp_dir().join(format!("kolboo-journal-{}.pcm", uuid::Uuid::new_v4()));
        let mut journal = Journal::create(&path, 16000, 1).unwrap();
        assert!(journal.append(&[0.0], 48000, 1).is_err());
        assert!(journal.finish().is_err());
        drop(journal);
        std::fs::remove_file(path).unwrap();
    }
}
