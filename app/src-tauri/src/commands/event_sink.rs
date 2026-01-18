use serde::Serialize;
use tauri::Emitter;

pub trait EventSink {
	fn emit<T: Serialize + ?Sized>(&self, event: &str, payload: &T);
}

pub struct AppEventSink<'a>(pub &'a tauri::AppHandle);

impl EventSink for AppEventSink<'_> {
	fn emit<T: Serialize + ?Sized>(&self, event: &str, payload: &T) {
		let _ = self.0.emit(event, payload);
	}
}
