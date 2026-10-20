// Feishin for OpenDeck. The plugin runs on one thread: every event from OpenDeck or Feishin runs to its
// end before the next one.
pub mod actions;
pub mod emitter;
pub mod feishin;
pub mod i18n;
pub mod images;
pub mod js;
pub mod plugin;
pub mod sd;
pub mod settings;
pub mod timers;

/// The version in the manifest, like "2.1.0.0".
pub fn version() -> String {
	let manifest: serde_json::Value =
		serde_json::from_str(include_str!("../../de.felitendo.feishin.sdPlugin/manifest.json")).unwrap_or_default();
	manifest["Version"].as_str().unwrap_or_default().to_string()
}
