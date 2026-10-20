// Messages to OpenDeck. Sending never waits: the messages go to a queue, in order, and main.rs passes
// them on through openaction.
use std::cell::RefCell;
use std::rc::Rc;

use serde_json::{Map, Value, json};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Clone)]
pub struct Sd {
	tx: UnboundedSender<String>,
	uuid: Rc<str>,
	/// Tests read what was sent from here instead of a socket.
	pub sent: Option<Rc<RefCell<Vec<Value>>>>,
}

impl Sd {
	pub fn new(tx: UnboundedSender<String>, uuid: &str) -> Self {
		Sd { tx, uuid: uuid.into(), sent: None }
	}

	/// For tests: keeps every message.
	pub fn recording() -> Self {
		let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
		std::mem::forget(rx);
		Sd { tx, uuid: "test".into(), sent: Some(Rc::new(RefCell::new(Vec::new()))) }
	}

	fn send(&self, message: Value) {
		if let Some(sent) = &self.sent {
			sent.borrow_mut().push(message.clone());
		}
		let _ = self.tx.send(message.to_string());
	}

	fn to_context(&self, event: &str, context: &str, payload: Option<Value>) {
		let mut message = Map::new();
		message.insert("event".into(), event.into());
		message.insert("context".into(), context.into());
		if let Some(payload) = payload {
			message.insert("payload".into(), payload);
		}
		self.send(Value::Object(message));
	}

	/// None: back to the title the user typed, or none.
	pub fn set_title(&self, context: &str, title: Option<&str>) {
		let mut payload = json!({ "target": 0 });
		if let Some(title) = title {
			payload["title"] = title.into();
		}
		self.to_context("setTitle", context, Some(payload));
	}

	/// image: a data URL, or None for the image from the manifest. state: None for every state.
	pub fn set_image(&self, context: &str, image: Option<&str>, state: Option<u16>) {
		let mut payload = json!({ "target": 0 });
		if let Some(image) = image {
			payload["image"] = image.into();
		}
		if let Some(state) = state {
			payload["state"] = state.into();
		}
		self.to_context("setImage", context, Some(payload));
	}

	pub fn set_state(&self, context: &str, state: u16) {
		self.to_context("setState", context, Some(json!({ "state": state })));
	}

	pub fn set_feedback(&self, context: &str, payload: Value) {
		self.to_context("setFeedback", context, Some(payload));
	}

	pub fn show_alert(&self, context: &str) {
		self.to_context("showAlert", context, None);
	}

	pub fn set_global_settings(&self, settings: &Map<String, Value>) {
		let uuid = self.uuid.clone();
		self.to_context("setGlobalSettings", &uuid, Some(Value::Object(settings.clone())));
	}

	pub fn get_global_settings(&self) {
		let uuid = self.uuid.clone();
		self.to_context("getGlobalSettings", &uuid, None);
	}

	pub fn send_to_property_inspector(&self, context: &str, payload: Value) {
		self.to_context("sendToPropertyInspector", context, Some(payload));
	}

	/// Goes to the plugin log file (OpenDeck catches stdout).
	pub fn log(&self, message: &str) {
		log(message);
	}
}

pub fn log(message: &str) {
	println!("[{}] {message}", crate::js::iso_time(crate::js::now()));
}
