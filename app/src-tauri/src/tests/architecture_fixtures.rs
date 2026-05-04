#![allow(dead_code)]

use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedEffect {
    pub kind: &'static str,
    pub detail: String,
}

#[derive(Debug, Default, Clone)]
pub struct EffectRecorder {
    effects: Arc<Mutex<Vec<RecordedEffect>>>,
}

impl EffectRecorder {
    pub fn push(&self, kind: &'static str, detail: impl Into<String>) {
        let mut effects = self.effects.lock().expect("effect recorder lock poisoned");
        effects.push(RecordedEffect {
            kind,
            detail: detail.into(),
        });
    }

    pub fn snapshot(&self) -> Vec<RecordedEffect> {
        self.effects
            .lock()
            .expect("effect recorder lock poisoned")
            .clone()
    }

    pub fn count_kind(&self, kind: &'static str) -> usize {
        self.effects
            .lock()
            .expect("effect recorder lock poisoned")
            .iter()
            .filter(|effect| effect.kind == kind)
            .count()
    }
}

pub fn fixture_request_id(suffix: &str) -> String {
    format!("architecture-fixture-{suffix}")
}

pub fn sanitized_fixture_text(label: &str) -> String {
    format!("deterministic fixture text: {label}")
}
