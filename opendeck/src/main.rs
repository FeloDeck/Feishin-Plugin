// Entry point. OpenDeck starts this with the usual Stream Deck arguments.
//
// openaction talks to OpenDeck. Its handlers only pass the events on to the plugin, which runs on
// this one thread.
use std::sync::OnceLock;

use feishin_plugin::actions::Controller;
use feishin_plugin::feishin::Client;
use feishin_plugin::plugin::{Event, EventKind, Plugin};
use feishin_plugin::sd::{Sd, log};
use feishin_plugin::{i18n, version};
use openaction::global_events::{DidReceiveGlobalSettingsEvent, GlobalEventHandler, SystemDidWakeUpEvent, set_global_event_handler};
use openaction::{Action, Instance, OpenActionResult, async_trait, register_action};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio::task::{LocalSet, spawn_local};

enum Inbound {
	Ready,
	GlobalSettings(Value),
	WakeUp,
	Event(Box<Event>),
}

static INBOUND: OnceLock<mpsc::UnboundedSender<Inbound>> = OnceLock::new();

fn send(message: Inbound) {
	if let Some(tx) = INBOUND.get() {
		let _ = tx.send(message);
	}
}

fn forward(kind: EventKind, instance: &Instance, settings: Option<&Value>) -> OpenActionResult<()> {
	send(Inbound::Event(Box::new(Event {
		kind,
		action: instance.action_uuid.clone(),
		context: instance.instance_id.clone(),
		controller: Controller::from_name(&instance.controller),
		is_in_multi_action: instance.is_in_multi_action,
		settings: settings.cloned(),
	})));
	Ok(())
}

// One openaction action per action of the plugin. They all just pass the events on.
macro_rules! actions {
	($($name:ident => $uuid:literal),* $(,)?) => {
		$(
			struct $name;

			#[async_trait]
			impl Action for $name {
				const UUID: &'static str = $uuid;
				type Settings = Value;

				async fn will_appear(&self, i: &Instance, s: &Value) -> OpenActionResult<()> {
					forward(EventKind::WillAppear, i, Some(s))
				}

				async fn will_disappear(&self, i: &Instance, s: &Value) -> OpenActionResult<()> {
					forward(EventKind::WillDisappear, i, Some(s))
				}

				async fn key_down(&self, i: &Instance, s: &Value) -> OpenActionResult<()> {
					forward(EventKind::KeyDown, i, Some(s))
				}

				async fn key_up(&self, i: &Instance, s: &Value) -> OpenActionResult<()> {
					forward(EventKind::KeyUp, i, Some(s))
				}

				async fn dial_rotate(&self, i: &Instance, s: &Value, ticks: i16, _pressed: bool) -> OpenActionResult<()> {
					forward(EventKind::DialRotate(f64::from(ticks)), i, Some(s))
				}

				async fn dial_down(&self, i: &Instance, s: &Value) -> OpenActionResult<()> {
					forward(EventKind::DialDown, i, Some(s))
				}

				async fn touch_tap(&self, i: &Instance, s: &Value, _position: (u16, u16), _hold: bool) -> OpenActionResult<()> {
					forward(EventKind::TouchTap, i, Some(s))
				}

				async fn did_receive_settings(&self, i: &Instance, s: &Value) -> OpenActionResult<()> {
					forward(EventKind::DidReceiveSettings, i, Some(s))
				}

				async fn property_inspector_did_appear(&self, i: &Instance, _s: &Value) -> OpenActionResult<()> {
					forward(EventKind::InspectorDidAppear, i, None)
				}

				async fn property_inspector_did_disappear(&self, i: &Instance, _s: &Value) -> OpenActionResult<()> {
					forward(EventKind::InspectorDidDisappear, i, None)
				}

				async fn send_to_plugin(&self, i: &Instance, _s: &Value, payload: &Value) -> OpenActionResult<()> {
					forward(EventKind::SendToPlugin(payload.clone()), i, None)
				}
			}
		)*

		async fn register_all() {
			$(register_action($name).await;)*
		}

		#[cfg(test)]
		const UUIDS: &[&str] = &[$($uuid),*];
	};
}

actions! {
	PlayPause => "de.felitendo.feishin.playpause",
	Next => "de.felitendo.feishin.next",
	Previous => "de.felitendo.feishin.previous",
	Shuffle => "de.felitendo.feishin.shuffle",
	Repeat => "de.felitendo.feishin.repeat",
	Favorite => "de.felitendo.feishin.favorite",
	VolumeUp => "de.felitendo.feishin.volumeup",
	VolumeDown => "de.felitendo.feishin.volumedown",
	Mute => "de.felitendo.feishin.mute",
	FastForward => "de.felitendo.feishin.forward",
	Rewind => "de.felitendo.feishin.rewind",
	VolumeDial => "de.felitendo.feishin.volumedial",
	SeekDial => "de.felitendo.feishin.seekdial",
}

struct Globals;

#[async_trait]
impl GlobalEventHandler for Globals {
	async fn plugin_ready(&self) -> OpenActionResult<()> {
		send(Inbound::Ready);
		Ok(())
	}

	async fn did_receive_global_settings(&self, event: DidReceiveGlobalSettingsEvent) -> OpenActionResult<()> {
		send(Inbound::GlobalSettings(event.payload.settings));
		Ok(())
	}

	async fn system_did_wake_up(&self, _event: SystemDidWakeUpEvent) -> OpenActionResult<()> {
		send(Inbound::WakeUp);
		Ok(())
	}
}

static GLOBALS: Globals = Globals;

// openaction logs what it could not handle. Those go to the plugin log too.
struct Logger;

impl log::Log for Logger {
	fn enabled(&self, metadata: &log::Metadata) -> bool {
		metadata.level() <= log::Level::Warn
	}

	fn log(&self, record: &log::Record) {
		if self.enabled(record.metadata()) {
			log(&format!("{}: {}", record.level(), record.args()));
		}
	}

	fn flush(&self) {}
}

static LOGGER: Logger = Logger;

fn main() {
	let args: Vec<String> = std::env::args().collect();
	let flag = |name: &str| args.iter().position(|a| a.trim().to_lowercase() == name).and_then(|i| args.get(i + 1)).cloned();
	let info: Value = flag("-info").and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null);
	let uuid = flag("-pluginuuid").unwrap_or_default();
	let language = i18n::set_language(info.pointer("/application/language").and_then(Value::as_str).unwrap_or(""));
	let _ = log::set_logger(&LOGGER).map(|()| log::set_max_level(log::LevelFilter::Warn));
	std::panic::set_hook(Box::new(|panic| log(&format!("uncaught: {panic}"))));

	let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
		Ok(runtime) => runtime,
		Err(error) => {
			log(&format!("Feishin could not start: {error}"));
			std::process::exit(1);
		}
	};
	LocalSet::new().block_on(&runtime, async move {
		let (tx, mut rx) = mpsc::unbounded_channel();
		let _ = INBOUND.set(tx);
		let (out_tx, mut out_rx) = mpsc::unbounded_channel::<String>();
		let sd = Sd::new(out_tx, &uuid);
		let plugin = Plugin::new(sd.clone(), Client::new(), feishin_plugin::actions::all());

		let core_sd = sd.clone();
		spawn_local(async move {
			while let Some(message) = rx.recv().await {
				match message {
					Inbound::Ready => {
						// Messages sent before the connection was up wait in the queue, in order.
						let mut queue = std::mem::replace(&mut out_rx, mpsc::unbounded_channel().1);
						spawn_local(async move {
							while let Some(text) = queue.recv().await {
								let Ok(raw) = serde_json::value::RawValue::from_string(text) else { continue };
								if let Err(error) = openaction::send_arbitrary_json(raw).await {
									log(&format!("OpenDeck socket error: {error}"));
								}
							}
						});
						core_sd.log(&format!("Feishin plugin started (version {}, language {language})", version()));
						core_sd.get_global_settings();
					}
					Inbound::GlobalSettings(settings) => plugin.on_global_settings(Some(&settings)),
					Inbound::WakeUp => plugin.on_wake_up(),
					Inbound::Event(event) => plugin.on_event(*event),
				}
			}
		});

		register_all().await;
		set_global_event_handler(&GLOBALS);
		match openaction::run(args).await {
			Ok(()) => sd.log("OpenDeck closed the connection, exiting"),
			Err(error) => sd.log(&format!("OpenDeck socket error: {error}")),
		}
		std::process::exit(0);
	});
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn every_action_of_the_manifest_is_here() {
		let manifest: Value = serde_json::from_str(include_str!("../../de.felitendo.feishin.sdPlugin/manifest.json")).unwrap_or_default();
		let mut in_manifest: Vec<&str> =
			manifest["Actions"].as_array().map(|a| a.iter().filter_map(|a| a["UUID"].as_str()).collect()).unwrap_or_default();
		let mut kinds: Vec<&str> = feishin_plugin::actions::all().iter().map(|k| k.uuid).collect();
		let mut here = UUIDS.to_vec();
		for list in [&mut in_manifest, &mut kinds, &mut here] {
			list.sort_unstable();
		}
		assert_eq!(kinds, in_manifest, "actions::all()");
		assert_eq!(here, in_manifest, "actions! in main.rs");
	}
}
