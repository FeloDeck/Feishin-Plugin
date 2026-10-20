// The actions. Each one is a Kind: what it shows of the player and what it does (Behavior). key.rs
// does the rest for every key: drawing at most 10 times a second, the status image, alerts, holding.
mod key;
pub mod playback;
pub mod seek;
pub mod volume;

#[cfg(test)]
pub(crate) use key::testing;
pub use key::{Key, KeyRc};

use crate::feishin::Change;

/// A key (Keypad) or a dial with its touch strip (Encoder).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Controller {
	#[default]
	Keypad,
	Encoder,
}

impl Controller {
	/// From the name OpenDeck uses.
	pub fn from_name(name: &str) -> Controller {
		if name == "Encoder" { Controller::Encoder } else { Controller::Keypad }
	}
}

/// An action: what all its keys have in common.
pub struct Kind {
	pub uuid: &'static str,
	/// For the log.
	pub name: &'static str,
	/// What of the player the action shows: its keys draw again when it changes.
	pub watches: &'static [Change],
	/// Holding the key repeats the press (volume and seek keys).
	pub repeats: bool,
	pub behavior: &'static dyn Behavior,
}

impl Kind {
	/// What an action does not have to set: it shows nothing of the player, and does not repeat.
	/// `Kind { uuid, name, behavior, ..Kind::BASE }`
	pub const BASE: Kind = Kind { uuid: "", name: "", watches: &[], repeats: false, behavior: &NoHooks };
}

struct NoHooks;

impl Behavior for NoHooks {}

/// The hooks of an action. All are optional. A command returns false when Feishin did not get it
/// (not connected): the key shows an alert then.
pub trait Behavior: Sync {
	fn press(&self, _k: &Key) -> bool {
		true
	}
	fn rotate(&self, _k: &Key, _ticks: f64) -> bool {
		true
	}
	/// A press of the dial, or a touch on its strip.
	fn dial_press(&self, _k: &Key) -> bool {
		true
	}
	/// Draws the key or the touch strip from the player state.
	fn render(&self, _k: &Key) {}
}

/// A number setting, like the seconds to jump. Empty, 0 or less: the fallback.
pub fn positive_or(value: f64, fallback: f64) -> f64 {
	if value.is_finite() && value > 0.0 { value } else { fallback }
}

/// All actions.
pub fn all() -> Vec<&'static Kind> {
	vec![
		&playback::PLAY_PAUSE,
		&playback::NEXT,
		&playback::PREVIOUS,
		&playback::SHUFFLE,
		&playback::REPEAT,
		&volume::VOLUME_UP,
		&volume::VOLUME_DOWN,
		&volume::MUTE,
		&seek::FORWARD,
		&seek::REWIND,
		&volume::VOLUME_DIAL,
		&seek::SEEK_DIAL,
	]
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::settings::testing::fields;

	#[test]
	fn the_settings_page_writes_every_setting() {
		let page = concat!(
			include_str!("../../../de.felitendo.feishin.sdPlugin/ui/property-inspector.html"),
			include_str!("../../../de.felitendo.feishin.sdPlugin/ui/property-inspector.js"),
		);
		let read = [
			fields::<playback::PlayPauseSettings>(),
			fields::<volume::VolumeKeySettings>(),
			fields::<volume::VolumeDialSettings>(),
			fields::<seek::SeekSettings>(),
			fields::<seek::SeekDialSettings>(),
		];
		for field in read.concat() {
			assert!(page.contains(&format!("\"{field}\"")), "the settings page has no {field}");
		}
	}
}
