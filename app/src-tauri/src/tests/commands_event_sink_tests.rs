use crate::commands::event_sink::EventSink;
use crate::events;
use crate::recording_completion::emit_pipeline_recording_started;
use crate::PipelineStateEvent;
use serde::Serialize;
use serde_json::Value;
use std::cell::RefCell;

#[derive(Default)]
struct VecSink {
    events: RefCell<Vec<(String, Value)>>,
}

impl EventSink for VecSink {
    fn emit<T: Serialize + ?Sized>(&self, event: &str, payload: &T) {
        let value = serde_json::to_value(payload).unwrap_or(Value::Null);
        self.events.borrow_mut().push((event.to_string(), value));
    }
}

#[test]
fn test_emit_pipeline_recording_started_emits_expected_events() {
    let sink = VecSink::default();

    emit_pipeline_recording_started(&sink);

    let events = sink.events.borrow();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].0, events::EVENT_PIPELINE_RECORDING_STARTED);
    assert_eq!(events[0].1, Value::Null);
    assert_eq!(events[1].0, events::EVENT_PIPELINE_STATE_CHANGED);
    assert_eq!(
        events[1].1,
        serde_json::to_value(PipelineStateEvent::Recording).unwrap()
    );
}
