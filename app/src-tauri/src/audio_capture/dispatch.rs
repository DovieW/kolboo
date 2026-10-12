//! Nonblocking callback delivery. Queue loss is a failed recording, never a
//! successful transcript with missing audio. Permits include the capture epoch
//! so draining a Hot Mic tail cannot append it to the next recording.
use super::*;
use std::sync::atomic::AtomicUsize;

#[derive(Debug, Default)]
pub(super) struct CaptureFence {
    generation: AtomicU64,
    state: AtomicU64,
    pending: AtomicUsize,
    callbacks: AtomicUsize,
    failed: AtomicU64,
}

impl CaptureFence {
    pub(super) fn begin(&self) -> u64 {
        let epoch = self.generation.fetch_add(2, Ordering::SeqCst) + 1;
        self.state.store(epoch, Ordering::SeqCst);
        epoch
    }
    pub(super) fn stop(&self) {
        let _ = self
            .state
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |epoch| {
                Some(epoch + (epoch & 1))
            });
    }
    pub(super) fn resume(&self, epoch: u64) {
        self.state.store(epoch, Ordering::SeqCst);
    }
    pub(super) fn is_current(&self, epoch: u64) -> bool {
        epoch & 1 != 0 && self.state.load(Ordering::SeqCst) == epoch
    }
    pub(super) fn fail(&self, epoch: u64) {
        if epoch & 1 != 0 {
            self.failed.fetch_max(epoch, Ordering::SeqCst);
        }
    }
    pub(super) fn failed(&self, epoch: u64) -> bool {
        epoch != 0 && self.failed.load(Ordering::SeqCst) == epoch
    }
    pub(super) fn drain(&self, timeout: std::time::Duration) -> bool {
        let start = std::time::Instant::now();
        while self.pending.load(Ordering::SeqCst) != 0 || self.callbacks.load(Ordering::SeqCst) != 0
        {
            if start.elapsed() >= timeout {
                return false;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        true
    }
}

#[derive(Debug)]
struct CountPermit<'a>(&'a AtomicUsize);
impl<'a> CountPermit<'a> {
    fn new(count: &'a AtomicUsize) -> Self {
        count.fetch_add(1, Ordering::SeqCst);
        Self(count)
    }
}
impl Drop for CountPermit<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

#[derive(Debug)]
struct RecordingPermit(Arc<CaptureFence>);
impl Drop for RecordingPermit {
    fn drop(&mut self) {
        self.0.pending.fetch_sub(1, Ordering::SeqCst);
    }
}

#[derive(Debug)]
pub(super) struct CapturedChunk {
    pub(super) samples: Vec<f32>,
    pub(super) epoch: u64,
    _permit: Option<RecordingPermit>,
}

#[derive(Clone)]
pub(super) struct CaptureDispatch {
    pub(super) queue: Arc<ArrayQueue<CapturedChunk>>,
    pub(super) pool: Arc<ArrayQueue<Vec<f32>>>,
    pub(super) fence: Arc<CaptureFence>,
}

impl CaptureDispatch {
    pub(super) fn enqueue<T: Copy>(&self, data: &[T], convert: fn(T) -> f32) {
        let _callback = CountPermit::new(&self.fence.callbacks);
        let epoch = self.fence.state.load(Ordering::SeqCst);
        let permit = if epoch & 1 != 0 {
            self.fence.pending.fetch_add(1, Ordering::SeqCst);
            Some(RecordingPermit(self.fence.clone()))
        } else {
            None
        };
        let Some(mut samples) = self.pool.pop() else {
            self.fence.fail(epoch);
            return;
        };
        samples.clear();
        samples.extend(data.iter().copied().map(convert));
        let chunk = CapturedChunk {
            samples,
            epoch,
            _permit: permit,
        };
        if let Err(mut chunk) = self.queue.push(chunk) {
            self.fence.fail(epoch);
            chunk.samples.clear();
            let _ = self.pool.push(std::mem::take(&mut chunk.samples));
        }
    }
}

pub(super) fn append_recorded_chunk(
    buffer: &mut AudioBuffer,
    chunk: &CapturedChunk,
    fence: &CaptureFence,
) -> bool {
    if chunk.epoch & 1 == 0 || chunk.epoch != buffer.recording_epoch {
        return false;
    }
    if fence.failed(chunk.epoch) {
        buffer.fail_live_input();
    }
    buffer.append(&chunk.samples);
    true
}

pub(super) fn normalize_i16(value: i16) -> f32 {
    ((value as f32) / i16::MAX as f32).clamp(-1.0, 1.0)
}
pub(super) fn normalize_u16(value: u16) -> f32 {
    (((value as f32) / u16::MAX as f32) * 2.0 - 1.0).clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dispatch(pool_size: usize) -> CaptureDispatch {
        let pool = Arc::new(ArrayQueue::new(pool_size.max(1)));
        for _ in 0..pool_size {
            pool.push(Vec::with_capacity(8)).unwrap();
        }
        CaptureDispatch {
            pool,
            queue: Arc::new(ArrayQueue::new(1)),
            fence: Arc::new(CaptureFence::default()),
        }
    }
    #[test]
    fn full_queues_and_empty_pools_fail_only_the_owned_recording_without_reordering() {
        for empty_pool in [false, true] {
            let d = dispatch(if empty_pool { 0 } else { 2 });
            let epoch = d.fence.begin();
            d.enqueue(&[0.1], std::convert::identity);
            d.enqueue(&[0.2], std::convert::identity);
            assert!(d.fence.failed(epoch));
            if !empty_pool {
                let chunk = d.queue.pop().unwrap();
                assert_eq!(chunk.samples, [0.1]);
                assert!(!d.fence.drain(std::time::Duration::ZERO));
                drop(chunk);
            }
            assert!(d.fence.drain(std::time::Duration::ZERO));
            d.fence.stop();
            let next = d.fence.begin();
            assert!(!d.fence.failed(next));
            d.fence.fail(epoch);
            assert!(!d.fence.failed(next));
            d.fence.fail(next);
            d.fence.fail(epoch);
            assert!(d.fence.failed(next));
        }
        let d = dispatch(0);
        d.enqueue(&[1.0], std::convert::identity);
        assert!(!d.fence.failed(0));
    }
    #[test]
    fn buffered_audio_stays_with_its_epoch_even_after_stop_pause_and_restart() {
        let d = dispatch(3);
        let epoch = d.fence.begin();
        let mut buffer = AudioBuffer::new(16000, 1, 1.0);
        buffer.recording_epoch = epoch;
        d.enqueue(&[i16::MIN, i16::MAX], normalize_i16);
        d.fence.stop();
        let chunk = d.queue.pop().unwrap();
        assert!(append_recorded_chunk(&mut buffer, &chunk, &d.fence));
        assert_eq!(buffer.snapshot(), [-1.0, 1.0]);
        d.fence.resume(epoch);
        d.enqueue(&[0u16, u16::MAX], normalize_u16);
        let next = d.queue.pop().unwrap();
        assert_eq!(next.samples, [-1.0, 1.0]);
        d.fence.stop();
        buffer.reset_for_recording(1.0);
        buffer.recording_epoch = d.fence.begin();
        assert!(!append_recorded_chunk(&mut buffer, &chunk, &d.fence));
        assert!(buffer.snapshot().is_empty());
        d.fence.stop();
        d.enqueue(&[1.0], std::convert::identity);
        let idle = d.queue.pop().unwrap();
        assert!(!append_recorded_chunk(&mut buffer, &idle, &d.fence));
        d.fence.fail(buffer.recording_epoch);
        // A failed active epoch can still retain its partial audio for recovery.
        buffer.recording_epoch = next.epoch;
        d.fence.fail(next.epoch);
        assert!(append_recorded_chunk(&mut buffer, &next, &d.fence));
    }
}
