//! One lazy, bounded owner for notification playback. No native output remains
//! open indefinitely after a cue, and native callbacks never format/log errors.
use super::*;
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender, TrySendError},
    Arc, OnceLock,
};
use std::time::Instant;

type CueResult = Result<(), &'static str>;
type CueSource = Result<Box<dyn Source + Send>, &'static str>;
const CAPACITY: usize = 8;
const POLL: Duration = Duration::from_millis(20);
const TAIL_PAD: Duration = Duration::from_millis(250);
const IDLE_GRACE: Duration = Duration::from_secs(1);
const PLAYBACK_LIMIT: Duration = Duration::from_secs(5);
const CALLER_LIMIT: Duration = Duration::from_secs(7);
const OUTPUT_FAILED: &str = "notification output failed";
const OUTPUT_STALLED: &str = "notification output stopped completing playback";
const REQUEST_EXPIRED: &str = "notification cue expired before playback";
static REQUESTS: OnceLock<Result<SyncSender<Request>, &'static str>> = OnceLock::new();
type WorkerSpawn = fn(Receiver<Request>) -> std::io::Result<()>;

struct Request {
    sound: SoundType,
    cue: AudioCue,
    expires_at: Instant,
    completion: Option<SyncSender<CueResult>>,
}

impl Request {
    fn finish(self, result: CueResult) {
        finish(self.completion, result);
    }
}

fn finish(completion: Option<SyncSender<CueResult>>, result: CueResult) {
    if let Some(completion) = completion {
        let _ = completion.try_send(result);
    }
}

fn submit(sender: &SyncSender<Request>, request: Request) -> CueResult {
    sender.try_send(request).map_err(|error| match error {
        TrySendError::Full(_) => "notification playback queue is full",
        TrySendError::Disconnected(_) => "notification playback worker stopped",
    })
}

fn wait_for_completion(receiver: Receiver<CueResult>, timeout: Duration) -> CueResult {
    receiver.recv_timeout(timeout).unwrap_or(Err(
        "notification playback did not complete before its deadline",
    ))
}

fn worker_sender<'a>(
    cell: &'a OnceLock<Result<SyncSender<Request>, &'static str>>,
    spawn: WorkerSpawn,
) -> Result<&'a SyncSender<Request>, &'static str> {
    match cell.get_or_init(|| {
        let (sender, receiver) = mpsc::sync_channel(CAPACITY);
        spawn(receiver).map_err(|_| "notification playback worker could not start")?;
        Ok(sender)
    }) {
        Ok(sender) => Ok(sender),
        Err(error) => Err(*error),
    }
}

fn spawn_worker(receiver: Receiver<Request>) -> std::io::Result<()> {
    std::thread::Builder::new()
        .name("kolboo-cue-output".into())
        .spawn(move || run_worker(receiver, &mut Output::open))
        .map(drop)
}

pub(super) fn play(sound: SoundType, cue: AudioCue, blocking: bool) -> CueResult {
    let sender = worker_sender(&REQUESTS, spawn_worker)?;
    play_with_sender(sender, sound, cue, blocking)
}

fn play_with_sender(
    sender: &SyncSender<Request>,
    sound: SoundType,
    cue: AudioCue,
    blocking: bool,
) -> CueResult {
    let (completion, receiver) = if blocking {
        let (sender, receiver) = mpsc::sync_channel(1);
        (Some(sender), Some(receiver))
    } else {
        (None, None)
    };
    submit(
        sender,
        Request {
            sound,
            cue,
            expires_at: Instant::now() + CALLER_LIMIT,
            completion,
        },
    )?;
    if let Some(receiver) = receiver {
        wait_for_completion(receiver, CALLER_LIMIT)?;
    }
    Ok(())
}

fn output_error_handler(failed: Arc<AtomicBool>) -> impl FnMut(cpal::StreamError) + Clone {
    move |_error| failed.store(true, Ordering::Release)
}

struct Output {
    sink: MixerDeviceSink,
    failed: Arc<AtomicBool>,
}

impl Output {
    fn open() -> Result<Self, &'static str> {
        let failed = Arc::new(AtomicBool::new(false));
        let mut sink = DeviceSinkBuilder::from_default_device()
            .map_err(output_device_error)?
            .with_error_callback(output_error_handler(failed.clone()))
            .open_sink_or_fallback()
            .map_err(output_device_error)?;
        // Deliberate idle/fault disposal is normal, not Rodio's stderr warning.
        sink.log_on_drop(false);
        Ok(Self { sink, failed })
    }
}

fn output_device_error(_error: rodio::DeviceSinkError) -> &'static str {
    "notification output is unavailable"
}

struct Pending {
    player: Player,
    deadline: Instant,
    tail_until: Option<Instant>,
    completion: Option<SyncSender<CueResult>>,
}

impl Pending {
    fn completed(&mut self, now: Instant) -> bool {
        if !self.player.empty() {
            return false;
        }
        now >= *self.tail_until.get_or_insert(now + TAIL_PAD)
    }
}

#[derive(Default)]
struct Owner {
    output: Option<Output>,
    pending: Vec<Pending>,
    idle_until: Option<Instant>,
    failure_reported: bool,
}

impl Owner {
    fn report_failure(&mut self, error: &'static str) -> bool {
        if self.failure_reported {
            return false;
        }
        self.failure_reported = true;
        crate::runtime_diagnostics::record(crate::runtime_diagnostics::Event::Failure(
            crate::runtime_diagnostics::Failure::AudioOutput,
        ));
        log::warn!("Audio cue output unavailable: {error}");
        true
    }

    fn poll(&mut self, now: Instant) -> CueResult {
        if self
            .output
            .as_ref()
            .is_some_and(|output| output.failed.load(Ordering::Acquire))
        {
            return Err(OUTPUT_FAILED);
        }
        if self.pending.iter().any(|pending| now >= pending.deadline) {
            return Err(OUTPUT_STALLED);
        }
        let mut completed = false;
        self.pending.retain_mut(|pending| {
            if pending.completed(now) {
                completed = true;
                finish(pending.completion.take(), Ok(()));
                false
            } else {
                true
            }
        });
        if completed {
            // A successful open alone is not recovery: broken backends can
            // repeatedly open and fault. Re-arm reports only after real audio
            // consumption and the tail have completed successfully.
            self.failure_reported = false;
        }
        if self.pending.is_empty() && self.output.is_some() {
            if now >= *self.idle_until.get_or_insert(now + IDLE_GRACE) {
                self.output.take();
                self.idle_until = None;
            }
        }
        Ok(())
    }

    fn stop(&mut self, error: &'static str) {
        // Stop producers before reporting completion or accepting another cue.
        self.output.take();
        for pending in self.pending.drain(..) {
            finish(pending.completion, Err(error));
        }
        self.idle_until = None;
    }

    fn start(
        &mut self,
        request: Request,
        now: Instant,
        source: CueSource,
        open: &mut dyn FnMut() -> Result<Output, &'static str>,
    ) {
        if now >= request.expires_at {
            request.finish(Err(REQUEST_EXPIRED));
            return;
        }
        if self.pending.len() >= CAPACITY {
            request.finish(Err("too many notification cues are already playing"));
            return;
        }
        let source = match source {
            Ok(source) => source,
            Err(error) => {
                request.finish(Err(error));
                return;
            }
        };
        if self.output.is_none() {
            match open() {
                Ok(output) => self.output = Some(output),
                Err(error) => {
                    self.report_failure(error);
                    request.finish(Err(error));
                    return;
                }
            }
        }
        // A slow native open must not play a cue after its waiting caller has
        // already moved on (for example, to muting system audio).
        if Instant::now() >= request.expires_at {
            request.finish(Err(REQUEST_EXPIRED));
            return;
        }
        let output = self.output.as_ref().expect("cue output was opened");
        if output.failed.load(Ordering::Acquire) {
            // Keep the faulted owner for the next poll to retire and reject
            // queued requests, but never append to an already-broken output.
            request.finish(Err(OUTPUT_FAILED));
            return;
        }
        let player = Player::connect_new(output.sink.mixer());
        player.append(source);
        let playback_started = Instant::now().max(now);
        self.pending.push(Pending {
            player,
            deadline: (playback_started + PLAYBACK_LIMIT).min(request.expires_at),
            tail_until: None,
            completion: request.completion,
        });
        self.idle_until = None;
    }
}

fn cue_source(sound: SoundType, cue: AudioCue) -> CueSource {
    match cue {
        AudioCue::Legacy => decode_legacy(match sound {
            SoundType::RecordingStart => START_SOUND,
            SoundType::RecordingStop => STOP_SOUND,
        }),
        _ => Ok(Box::new(build_synth_cue_source(sound, cue).0)),
    }
}

fn decode_legacy(bytes: &'static [u8]) -> CueSource {
    Decoder::new(Cursor::new(bytes))
        .map(|decoded| Box::new(decoded.amplify(0.3)) as Box<dyn Source + Send>)
        .map_err(|_| "notification audio could not be decoded")
}

fn run_worker(receiver: Receiver<Request>, open: &mut dyn FnMut() -> Result<Output, &'static str>) {
    let mut owner = Owner::default();
    loop {
        // No output and no request means a blocking receive, not an idle timer.
        let request = if owner.output.is_some() {
            receiver.recv_timeout(POLL)
        } else {
            receiver
                .recv()
                .map_err(|_| mpsc::RecvTimeoutError::Disconnected)
        };
        let now = Instant::now();
        if let Err(error) = owner.poll(now) {
            owner.report_failure(error);
            owner.stop(error);
            if let Ok(request) = request {
                request.finish(Err(error));
            }
            // Queued cues belonged to the failed output. Do not silently replay
            // them on another device; only a subsequent fresh request reopens.
            for request in receiver.try_iter().take(CAPACITY) {
                request.finish(Err(error));
            }
            continue;
        }
        match request {
            Ok(request) => {
                let source = cue_source(request.sound, request.cue);
                owner.start(request, now, source, open);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                owner.stop("notification playback worker stopped");
                return;
            }
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
pub(super) mod tests;
