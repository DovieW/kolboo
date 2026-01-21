/// Pipeline state machine
///
/// State transition contract (self -> next):
/// - Idle -> Recording | Transcribing | Error
/// - Recording -> Transcribing | Idle | Error
/// - Transcribing -> Routing | Rewriting | Idle | Error
/// - Routing -> Transcribing | Idle | Error
/// - Rewriting -> Idle | Error
/// - Error -> Idle | Recording | Transcribing
/// - Self -> Self is allowed (idempotent updates)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineState {
    /// Pipeline is idle, ready to start recording
    Idle,
    /// Pipeline is actively recording audio
    Recording,
    /// Pipeline is transcribing recorded audio
    Transcribing,
    /// Pipeline is running the intent router (preset selection) after STT
    Routing,
    /// Pipeline is rewriting/formatting text via an LLM (optional step)
    Rewriting,
    /// Pipeline encountered an error (recoverable - can start new recording)
    Error,
}

impl PipelineState {
    /// Check if this state allows starting a new recording
    pub fn can_start_recording(&self) -> bool {
        matches!(self, PipelineState::Idle | PipelineState::Error)
    }

    /// Check if this state allows stopping a recording
    pub fn can_stop_recording(&self) -> bool {
        matches!(self, PipelineState::Recording)
    }

    /// Check if this state allows cancellation
    pub fn can_cancel(&self) -> bool {
        matches!(
            self,
            PipelineState::Recording
                | PipelineState::Transcribing
                | PipelineState::Rewriting
                | PipelineState::Routing
        )
    }

    pub fn can_transition_to(self, next: PipelineState) -> bool {
        if self == next {
            return true;
        }

        match self {
            PipelineState::Idle => matches!(
                next,
                PipelineState::Recording | PipelineState::Transcribing | PipelineState::Error
            ),
            PipelineState::Recording => matches!(
                next,
                PipelineState::Transcribing | PipelineState::Idle | PipelineState::Error
            ),
            PipelineState::Transcribing => matches!(
                next,
                PipelineState::Routing
                    | PipelineState::Rewriting
                    | PipelineState::Idle
                    | PipelineState::Error
            ),
            PipelineState::Routing => matches!(
                next,
                PipelineState::Transcribing | PipelineState::Idle | PipelineState::Error
            ),
            PipelineState::Rewriting => matches!(next, PipelineState::Idle | PipelineState::Error),
            PipelineState::Error => matches!(
                next,
                PipelineState::Idle | PipelineState::Recording | PipelineState::Transcribing
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_guards() {
        assert!(PipelineState::Idle.can_start_recording());
        assert!(PipelineState::Error.can_start_recording());
        assert!(!PipelineState::Recording.can_start_recording());
        assert!(!PipelineState::Transcribing.can_start_recording());

        assert!(PipelineState::Recording.can_stop_recording());
        assert!(!PipelineState::Idle.can_stop_recording());

        assert!(PipelineState::Recording.can_cancel());
        assert!(PipelineState::Transcribing.can_cancel());
        assert!(!PipelineState::Idle.can_cancel());
    }

    #[test]
    fn pipeline_state_transition_contract() {
        fn allowed_transitions(state: PipelineState) -> &'static [PipelineState] {
            match state {
                PipelineState::Idle => &[
                    PipelineState::Idle,
                    PipelineState::Recording,
                    PipelineState::Transcribing,
                    PipelineState::Error,
                ],
                PipelineState::Recording => &[
                    PipelineState::Recording,
                    PipelineState::Transcribing,
                    PipelineState::Idle,
                    PipelineState::Error,
                ],
                PipelineState::Transcribing => &[
                    PipelineState::Transcribing,
                    PipelineState::Routing,
                    PipelineState::Rewriting,
                    PipelineState::Idle,
                    PipelineState::Error,
                ],
                PipelineState::Routing => &[
                    PipelineState::Routing,
                    PipelineState::Transcribing,
                    PipelineState::Idle,
                    PipelineState::Error,
                ],
                PipelineState::Rewriting => &[
                    PipelineState::Rewriting,
                    PipelineState::Idle,
                    PipelineState::Error,
                ],
                PipelineState::Error => &[
                    PipelineState::Error,
                    PipelineState::Idle,
                    PipelineState::Recording,
                    PipelineState::Transcribing,
                ],
            }
        }

        let all_states = [
            PipelineState::Idle,
            PipelineState::Recording,
            PipelineState::Transcribing,
            PipelineState::Routing,
            PipelineState::Rewriting,
            PipelineState::Error,
        ];

        for &from in &all_states {
            for &to in &all_states {
                let expected = allowed_transitions(from).contains(&to);
                assert_eq!(
                    from.can_transition_to(to),
                    expected,
                    "unexpected transition {:?} -> {:?}",
                    from,
                    to
                );
            }
        }
    }
}
