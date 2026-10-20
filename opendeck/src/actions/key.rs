// One key or dial: its settings, drawing at most 10 times a second, the status image while Feishin
// cannot be reached, alerts, and repeating while held. Sends to OpenDeck only what changed.
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use tokio::time::Instant;

use super::{Behavior, Controller, Kind};
use crate::feishin::{Client, Status};
use crate::i18n::t;
use crate::images::status_image;
use crate::sd::Sd;
use crate::settings;
use crate::timers::{Timer, set_timeout};

/// The guidelines of Stream Deck allow at most 10 updates a second per key or touch strip. A fast
/// turn of a dial or a held volume key changes the state faster than that.
const MIN_RENDER_INTERVAL: Duration = Duration::from_millis(100);
const REPEAT_DELAY_MS: f64 = 400.0;
const REPEAT_EVERY_MS: f64 = 150.0;
const REPEAT_MAX: u32 = 100;

pub type KeyRc = Rc<Key>;

pub struct Key {
	pub kind: &'static Kind,
	pub context: String,
	pub controller: Controller,
	sd: Sd,
	client: Rc<Client>,
	b: RefCell<Base>,
	me: Weak<Key>,
}

#[derive(Default)]
struct Base {
	settings: Map<String, Value>,
	/// On a page of the deck. Keys in a multi action are not.
	visible: bool,
	last_render: Option<Instant>,
	pending_render: Option<Timer>,
	repeat: Option<Timer>,
	screen: Screen,
}

/// What was sent to OpenDeck, so the same is not sent twice. Starts empty when the key appears.
#[derive(Default)]
struct Screen {
	/// The image per state. None in it: the image from the manifest.
	images: HashMap<u16, Option<String>>,
	/// The status image, while Feishin cannot be reached.
	status: Option<String>,
	/// None in it: the title the user typed.
	title: Option<Option<String>>,
	state: Option<u16>,
	/// The touch strip content as JSON text.
	feedback: Option<String>,
}

impl Key {
	pub fn new(kind: &'static Kind, context: &str, controller: Controller, sd: Sd, client: Rc<Client>) -> KeyRc {
		Rc::new_cyclic(|me| Key {
			kind,
			context: context.to_string(),
			controller,
			sd,
			client,
			b: RefCell::new(Base::default()),
			me: me.clone(),
		})
	}

	pub fn feishin(&self) -> &Client {
		&self.client
	}

	/// The settings as the struct of the action (see settings.rs).
	pub fn settings<T: DeserializeOwned + Default>(&self) -> T {
		settings::read(&self.b.borrow().settings)
	}

	pub fn set_settings(&self, settings: Option<&Value>) {
		self.b.borrow_mut().settings = settings.and_then(Value::as_object).cloned().unwrap_or_default();
	}

	fn behavior(&self) -> &'static dyn Behavior {
		self.kind.behavior
	}

	// ----- from the plugin -----

	pub fn appear(&self, settings: Option<&Value>) {
		self.client.wake();
		self.set_settings(settings);
		{
			let mut b = self.b.borrow_mut();
			b.visible = true;
			b.screen = Screen::default();
		}
		self.render_now();
	}

	pub fn disappear(&self) {
		let mut b = self.b.borrow_mut();
		b.visible = false;
		b.screen = Screen::default();
		b.last_render = None;
		for timer in [b.pending_render.take(), b.repeat.take()].into_iter().flatten() {
			timer.clear();
		}
	}

	pub fn update_settings(&self, settings: Option<&Value>) {
		self.set_settings(settings);
		self.render_now();
	}

	pub fn key_down(&self, is_in_multi_action: bool) {
		self.stop_repeat();
		self.run(|| self.behavior().press(self));
		// Multi actions do not say when the key is let go.
		if !self.kind.repeats || is_in_multi_action || !self.client.connected() {
			return;
		}
		self.repeat(REPEAT_DELAY_MS, 1);
	}

	pub fn key_up(&self) {
		self.stop_repeat();
	}

	pub fn rotate(&self, ticks: f64) {
		self.run(|| self.behavior().rotate(self, ticks));
	}

	pub fn dial_press(&self) {
		self.run(|| self.behavior().dial_press(self));
	}

	/// The player state or the connection changed. Draws now, or once 100 ms passed since the last
	/// draw. That one uses the state at that time, so the last change is never lost.
	pub fn request_render(&self) {
		if self.b.borrow().pending_render.is_some() {
			return;
		}
		let last = self.b.borrow().last_render;
		let wait = last.map_or(Duration::ZERO, |last| (last + MIN_RENDER_INTERVAL).saturating_duration_since(Instant::now()));
		if wait.is_zero() {
			self.render_now();
			return;
		}
		let me = self.me.clone();
		let timer = set_timeout(wait.as_secs_f64() * 1000.0, move || {
			if let Some(me) = me.upgrade() {
				me.b.borrow_mut().pending_render = None;
				me.render_now();
			}
		});
		self.b.borrow_mut().pending_render = Some(timer);
	}

	fn render_now(&self) {
		if !self.b.borrow().visible {
			return;
		}
		self.b.borrow_mut().last_render = Some(Instant::now());
		if self.controller == Controller::Keypad && self.show_status() {
			return;
		}
		self.behavior().render(self);
	}

	// ----- commands -----

	/// A command that Feishin did not get shows an alert (the settings page says how to connect),
	/// and connecting starts again right away.
	fn run(&self, command: impl FnOnce() -> bool) {
		if !command() {
			self.client.wake();
			self.sd.show_alert(&self.context);
		}
	}

	fn repeat(&self, delay: f64, count: u32) {
		let me = self.me.clone();
		let timer = set_timeout(delay, move || {
			let Some(me) = me.upgrade() else { return };
			if count > REPEAT_MAX || !me.behavior().press(&me) {
				me.stop_repeat();
				return;
			}
			me.repeat(REPEAT_EVERY_MS, count + 1);
		});
		self.b.borrow_mut().repeat = Some(timer);
	}

	fn stop_repeat(&self) {
		if let Some(timer) = self.b.borrow_mut().repeat.take() {
			timer.clear();
		}
	}

	// ----- what the key shows -----

	/// Why Feishin cannot be reached, on the key, like in OBS Tools. It stays while the plugin tries
	/// again and goes once connected. Returns whether it shows, so the key draws nothing else.
	fn show_status(&self) -> bool {
		let status = self.client.connection().status;
		let image = match status {
			Status::Unreachable => Some(status_image(&t("status-offline"), false)),
			Status::Unauthorized => Some(status_image(&t("status-password"), true)),
			Status::Error => Some(status_image(&t("status-error"), true)),
			Status::Connecting | Status::Connected => None,
		};
		let shown = self.b.borrow().screen.status.clone();
		if image == shown || (image.is_none() && status == Status::Connecting) {
			return shown.is_some();
		}
		{
			let screen = &mut self.b.borrow_mut().screen;
			screen.status = image.clone();
			// For every state. Back to the images of the manifest once connected: render() sets its own.
			screen.images.insert(0, image.clone());
			screen.images.insert(1, image.clone());
		}
		self.sd.set_image(&self.context, image.as_deref(), None);
		image.is_some()
	}

	/// The image of one state. None: the image from the manifest.
	pub fn set_state_image(&self, state: u16, image: Option<String>) {
		{
			let screen = &mut self.b.borrow_mut().screen;
			if screen.images.get(&state) == Some(&image) {
				return;
			}
			screen.images.insert(state, image.clone());
		}
		self.sd.set_image(&self.context, image.as_deref(), Some(state));
	}

	/// The same image for every state.
	pub fn set_image(&self, image: String) {
		{
			let screen = &mut self.b.borrow_mut().screen;
			let image = Some(image.clone());
			if screen.images.get(&0) == Some(&image) && screen.images.get(&1) == Some(&image) {
				return;
			}
			screen.images.insert(0, image.clone());
			screen.images.insert(1, image);
		}
		self.sd.set_image(&self.context, Some(&image), None);
	}

	pub fn set_state(&self, state: u16) {
		if self.b.borrow().screen.state == Some(state) {
			return;
		}
		self.b.borrow_mut().screen.state = Some(state);
		self.sd.set_state(&self.context, state);
	}

	/// None: the title the user typed. Keys that draw their own text clear it with Some("").
	pub fn set_title(&self, title: Option<&str>) {
		let title = title.map(String::from);
		if self.b.borrow().screen.title.as_ref() == Some(&title) {
			return;
		}
		self.b.borrow_mut().screen.title = Some(title.clone());
		self.sd.set_title(&self.context, title.as_deref());
	}

	pub fn set_feedback(&self, feedback: Value) {
		let text = feedback.to_string();
		if self.b.borrow().screen.feedback.as_ref() == Some(&text) {
			return;
		}
		self.b.borrow_mut().screen.feedback = Some(text);
		self.sd.set_feedback(&self.context, feedback);
	}
}

#[cfg(test)]
pub(crate) mod testing {
	use serde_json::json;

	use super::*;
	use crate::feishin::testing::connected;

	/// A visible key of this action, with a Feishin in this state that only writes down what it gets.
	pub fn key(kind: &'static Kind, state: Value, settings: Value) -> (KeyRc, Sd) {
		let sd = Sd::recording();
		let controller = if kind.uuid.ends_with("dial") { Controller::Encoder } else { Controller::Keypad };
		let key = Key::new(kind, "c", controller, sd.clone(), connected(state));
		key.appear(Some(&settings));
		(key, sd)
	}

	/// The payloads OpenDeck got for this event.
	pub fn sent(sd: &Sd, event: &str) -> Vec<Value> {
		sd.sent
			.as_ref()
			.map(|s| s.borrow().iter().filter(|m| m["event"] == event).map(|m| m["payload"].clone()).collect())
			.unwrap_or_default()
	}

	pub fn no_settings() -> Value {
		json!({})
	}

	/// Runs a test with timers on one thread, like the plugin.
	pub async fn local<F: Future>(f: F) -> F::Output {
		tokio::task::LocalSet::new().run_until(f).await
	}

	/// Lets a draw happen that waits for the 100 ms since the last one.
	pub async fn later() {
		tokio::time::sleep(MIN_RENDER_INTERVAL + Duration::from_millis(10)).await;
	}
}
