// Routes OpenDeck events to the keys, keeps the connection settings (global, for every key) and tells
// the settings page how the connection is.
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};
#[cfg(test)]
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::actions::{Controller, Key, KeyRc, Kind};
use crate::feishin::{Change, Client, Connection, Settings};
use crate::js::round;
use crate::sd::Sd;
use crate::settings;
use crate::timers::{Timer, set_timeout};

/// The settings page changes the global settings while the user types: connect again after a pause.
const CONFIGURE_DELAY_MS: f64 = 600.0;
const SAVE_VOLUME_DELAY_MS: f64 = 2000.0;

/// What happened to a key.
#[derive(Debug, Clone)]
pub enum EventKind {
	WillAppear,
	WillDisappear,
	KeyDown,
	KeyUp,
	DialRotate(f64),
	DialDown,
	TouchTap,
	DidReceiveSettings,
	InspectorDidAppear,
	InspectorDidDisappear,
	/// A message of the settings page.
	SendToPlugin(Value),
}

/// An event from OpenDeck for one key.
#[derive(Debug, Clone)]
pub struct Event {
	pub kind: EventKind,
	pub action: String,
	pub context: String,
	pub controller: Controller,
	pub is_in_multi_action: bool,
	pub settings: Option<Value>,
}

/// Where Feishin is, from the settings page. Empty: the defaults.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Global {
	host: String,
	port: String,
	username: String,
	password: String,
}

/// A message of the settings page.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Message {
	/// "getConnection" or "reconnect".
	event: String,
}

pub struct Plugin {
	sd: Sd,
	feishin: Rc<Client>,
	kinds: HashMap<&'static str, &'static Kind>,
	keys: RefCell<HashMap<String, KeyRc>>,
	/// The keys whose settings page shows.
	inspectors: RefCell<Vec<String>>,
	/// As OpenDeck keeps them: the connection settings and lastVolume.
	global: RefCell<Map<String, Value>>,
	configured: Cell<bool>,
	configure_timer: RefCell<Option<Timer>>,
	save_volume_timer: RefCell<Option<Timer>>,
	me: Weak<Plugin>,
}

impl Plugin {
	pub fn new(sd: Sd, feishin: Rc<Client>, kinds: Vec<&'static Kind>) -> Rc<Plugin> {
		let plugin = Rc::new_cyclic(|me| Plugin {
			sd,
			feishin,
			kinds: kinds.into_iter().map(|k| (k.uuid, k)).collect(),
			keys: RefCell::default(),
			inspectors: RefCell::default(),
			global: RefCell::default(),
			configured: Cell::new(false),
			configure_timer: RefCell::default(),
			save_volume_timer: RefCell::default(),
			me: me.clone(),
		});
		let me = Rc::downgrade(&plugin);
		plugin.feishin.on_connection.on({
			let me = me.clone();
			move |connection| {
				if let Some(plugin) = me.upgrade() {
					plugin.connection_changed(connection);
				}
			}
		});
		plugin.feishin.on_state.on(move |changes| {
			if let Some(plugin) = me.upgrade() {
				plugin.state_changed(changes);
			}
		});
		plugin
	}

	fn keys(&self) -> Vec<KeyRc> {
		self.keys.borrow().values().cloned().collect()
	}

	// ----- from OpenDeck -----

	/// The first ones connect right away, later ones (typed on the settings page) after a pause.
	pub fn on_global_settings(&self, settings: Option<&Value>) {
		let global = settings.and_then(Value::as_object).cloned().unwrap_or_default();
		let g: Global = settings::read(&global);
		let connection = Settings::new(&g.host, &g.port, &g.username, &g.password);
		let last_volume = global.get("lastVolume").and_then(Value::as_f64);
		*self.global.borrow_mut() = global;
		if let Some(timer) = self.configure_timer.borrow_mut().take() {
			timer.clear();
		}
		if self.configured.replace(true) {
			let feishin = self.feishin.clone();
			*self.configure_timer.borrow_mut() = Some(set_timeout(CONFIGURE_DELAY_MS, move || feishin.configure(connection)));
			return;
		}
		// Feishin only says its volume once it changed, so the volume keys start with the last one seen.
		// Feishin starts with the volume it had, so it is right most of the time.
		if let Some(volume) = last_volume {
			self.feishin.assume_volume(volume);
		}
		self.feishin.configure(connection);
	}

	/// After sleep the connection may be dead without anyone noticing.
	pub fn on_wake_up(&self) {
		if self.configured.get() {
			self.feishin.reconnect();
		}
	}

	pub fn on_event(&self, e: Event) {
		match &e.kind {
			EventKind::WillAppear => {
				if let Some(key) = self.key_for(&e) {
					key.appear(e.settings.as_ref());
				}
			}
			EventKind::WillDisappear => {
				let key = self.keys.borrow_mut().remove(&e.context);
				if let Some(key) = key {
					key.disappear();
				}
			}
			EventKind::KeyDown => {
				if let Some(key) = self.pressed(&e) {
					key.key_down(e.is_in_multi_action);
				}
			}
			EventKind::KeyUp => {
				if let Some(key) = self.pressed(&e) {
					key.key_up();
				}
			}
			EventKind::DialRotate(ticks) => {
				if let Some(key) = self.pressed(&e) {
					key.rotate(*ticks);
				}
			}
			EventKind::DialDown | EventKind::TouchTap => {
				if let Some(key) = self.pressed(&e) {
					key.dial_press();
				}
			}
			EventKind::DidReceiveSettings => {
				if let Some(key) = self.key_for(&e) {
					key.update_settings(e.settings.as_ref());
				}
			}
			// OpenDeck loads settings pages in the background and only says they show once the key is
			// picked. Before that, the page cannot get the status it asks for.
			EventKind::InspectorDidAppear => {
				let mut inspectors = self.inspectors.borrow_mut();
				if !inspectors.contains(&e.context) {
					inspectors.push(e.context.clone());
				}
				drop(inspectors);
				self.send_connection(&e.context);
			}
			EventKind::InspectorDidDisappear => self.inspectors.borrow_mut().retain(|c| *c != e.context),
			EventKind::SendToPlugin(payload) => match settings::read_value::<Message>(payload).event.as_str() {
				"getConnection" => self.send_connection(&e.context),
				"reconnect" => self.feishin.reconnect(),
				_ => {}
			},
		}
	}

	fn key_for(&self, e: &Event) -> Option<KeyRc> {
		let known = self.keys.borrow().get(&e.context).cloned();
		if known.is_some() {
			return known;
		}
		let kind = *self.kinds.get(e.action.as_str())?;
		let key = Key::new(kind, &e.context, e.controller, self.sd.clone(), self.feishin.clone());
		self.keys.borrow_mut().insert(e.context.clone(), key.clone());
		Some(key)
	}

	/// Presses carry the settings. Keys in a multi action never appear, so they only have these.
	fn pressed(&self, e: &Event) -> Option<KeyRc> {
		let key = self.key_for(e)?;
		if e.settings.is_some() {
			key.set_settings(e.settings.as_ref());
		}
		Some(key)
	}

	// ----- from Feishin -----

	fn connection_changed(&self, c: &Connection) {
		let detail = if c.detail.is_empty() { String::new() } else { format!(" ({})", c.detail) };
		self.sd.log(&format!("Feishin ({}): {}{detail}", c.address, c.status.name()));
		self.send_connection_to_inspectors();
		for key in self.keys() {
			key.request_render();
		}
	}

	fn state_changed(&self, changes: &[Change]) {
		for key in self.keys() {
			if key.kind.watches.iter().any(|w| changes.contains(w)) {
				key.request_render();
			}
		}
		if !changes.contains(&Change::Volume) {
			return;
		}
		// For the hint that the volume is not known yet.
		self.send_connection_to_inspectors();
		if let Some(timer) = self.save_volume_timer.borrow_mut().take() {
			timer.clear();
		}
		let me = self.me.clone();
		let timer = set_timeout(SAVE_VOLUME_DELAY_MS, move || {
			if let Some(plugin) = me.upgrade() {
				plugin.save_volume();
			}
		});
		*self.save_volume_timer.borrow_mut() = Some(timer);
	}

	/// For the next start, see on_global_settings. Waits until the volume stays put for 2 seconds.
	fn save_volume(&self) {
		let Some(volume) = self.feishin.state().volume.map(round) else { return };
		let global = {
			let mut global = self.global.borrow_mut();
			if global.get("lastVolume").and_then(Value::as_f64) == Some(volume) {
				return;
			}
			global.insert("lastVolume".into(), json!(volume as i64));
			global.clone()
		};
		self.sd.set_global_settings(&global);
	}

	// ----- to the settings page -----

	/// The settings page uses it to guide the user through the setup.
	fn send_connection(&self, context: &str) {
		let c = self.feishin.connection();
		let volume_known = self.feishin.state().volume.is_some();
		let message = json!({ "event": "connection", "status": c.status.name(), "address": c.address, "detail": c.detail, "volumeKnown": volume_known });
		self.sd.send_to_property_inspector(context, message);
	}

	fn send_connection_to_inspectors(&self) {
		let inspectors = self.inspectors.borrow().clone();
		for context in inspectors {
			self.send_connection(&context);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::actions::all;
	use crate::actions::testing::{later, local, sent};
	use crate::feishin::testing::connected;

	fn plugin(state: Value) -> (Rc<Plugin>, Sd) {
		let sd = Sd::recording();
		(Plugin::new(sd.clone(), connected(state), all()), sd)
	}

	fn event(kind: EventKind, action: &str, context: &str, settings: Value) -> Event {
		let controller = if action.ends_with("dial") { Controller::Encoder } else { Controller::Keypad };
		Event {
			kind,
			action: format!("de.felitendo.feishin.{action}"),
			context: context.into(),
			controller,
			is_in_multi_action: false,
			settings: Some(settings),
		}
	}

	#[tokio::test(start_paused = true)]
	async fn keys_draw_again_when_what_they_show_changes() {
		local(async {
			let (p, sd) = plugin(json!({ "shuffle": false }));
			p.on_event(event(EventKind::WillAppear, "shuffle", "a", json!({})));
			p.on_event(event(EventKind::WillAppear, "repeat", "b", json!({})));
			let images = sent(&sd, "setImage").len();
			p.feishin.on_message(&json!({ "event": "shuffle", "data": true }));
			later().await;
			assert_eq!(sent(&sd, "setState").last(), Some(&json!({ "state": 1 })));
			assert_eq!(sent(&sd, "setImage").len(), images, "repeat does not show shuffle");
		})
		.await;
	}

	#[tokio::test(start_paused = true)]
	async fn presses_use_their_own_settings() {
		local(async {
			let (p, _sd) = plugin(json!({ "volume": 50 }));
			// In a multi action: no willAppear.
			let mut press = event(EventKind::KeyDown, "volumeup", "m", json!({ "step": "20" }));
			press.is_in_multi_action = true;
			p.on_event(press);
			assert_eq!(p.feishin.state().volume, Some(70.0));
		})
		.await;
	}

	#[tokio::test(start_paused = true)]
	async fn the_settings_page_gets_the_connection() {
		local(async {
			let (p, sd) = plugin(json!({}));
			p.on_event(event(EventKind::SendToPlugin(json!({ "event": "getConnection" })), "next", "a", json!({})));
			let connection =
				json!({ "event": "connection", "status": "connected", "address": "localhost:4333", "detail": "", "volumeKnown": false });
			assert_eq!(sent(&sd, "sendToPropertyInspector"), std::slice::from_ref(&connection));
			// Open pages also hear when the volume becomes known.
			p.on_event(event(EventKind::InspectorDidAppear, "next", "a", json!({})));
			p.feishin.on_message(&json!({ "event": "volume", "data": 30 }));
			let mut known = connection;
			known["volumeKnown"] = true.into();
			assert_eq!(sent(&sd, "sendToPropertyInspector").last(), Some(&known));
		})
		.await;
	}

	#[tokio::test(start_paused = true)]
	async fn the_last_volume_is_kept_for_the_next_start() {
		local(async {
			let (p, sd) = plugin(json!({}));
			p.on_global_settings(Some(&json!({ "host": "pc", "lastVolume": 30 })));
			assert_eq!(p.feishin.state().volume, Some(30.0), "until Feishin says its own");
			tokio::time::sleep(Duration::from_secs(3)).await;
			assert!(sent(&sd, "setGlobalSettings").is_empty(), "nothing new");
			p.feishin.on_message(&json!({ "event": "volume", "data": 41.6 }));
			tokio::time::sleep(Duration::from_secs(1)).await;
			// Typed on the settings page meanwhile: kept.
			p.on_global_settings(Some(&json!({ "host": "pc2", "lastVolume": 30 })));
			tokio::time::sleep(Duration::from_millis(1100)).await;
			assert_eq!(sent(&sd, "setGlobalSettings"), [json!({ "host": "pc2", "lastVolume": 42 })]);
		})
		.await;
	}
}
