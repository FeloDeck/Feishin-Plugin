// Volume: up, down, mute, and the volume dial.
use serde::Deserialize;
use serde_json::json;

use super::{Behavior, Key, Kind, positive_or};
use crate::feishin::Change;
use crate::i18n::t;
use crate::images::svg_with_title;
use crate::js::round;

const DEFAULT_STEP: f64 = 5.0;
/// How tall the speaker of the volume icons is, in units of their viewBox (y 4.2 to 19.8 with its
/// outline).
const SPEAKER_HEIGHT: f64 = 15.6;

#[derive(Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct VolumeKeySettings {
	/// Percent per press.
	step: f64,
	show_volume: bool,
	/// "below" or "above".
	volume_at: String,
}

impl Default for VolumeKeySettings {
	fn default() -> Self {
		VolumeKeySettings { step: 0.0, show_volume: true, volume_at: String::new() }
	}
}

/// The volume under or above the icon (unless turned off), both together in the middle of the key.
/// The key draws it into its image and clears its title meanwhile. Else the icon from the manifest.
/// icons: the icon per state.
fn render_volume(k: &Key, icons: &[&str]) {
	let s: VolumeKeySettings = k.settings();
	let volume = k.feishin().state().volume.filter(|_| s.show_volume && k.feishin().connected());
	k.set_title(volume.map(|_| ""));
	for (state, icon) in (0..).zip(icons) {
		let image = volume.map(|v| svg_with_title(icon, &format!("{}%", round(v)), SPEAKER_HEIGHT, s.volume_at == "above"));
		k.set_state_image(state, image);
	}
}

// ----- Volume up and down. Holding keeps going. -----

struct Step(f64);

impl Behavior for Step {
	fn press(&self, k: &Key) -> bool {
		let feishin = k.feishin();
		// At the end already: nothing to do, but no reason for an alert either.
		if feishin.connected() && feishin.state().volume == Some(if self.0 > 0.0 { 100.0 } else { 0.0 }) {
			return true;
		}
		feishin.change_volume(self.0 * positive_or(k.settings::<VolumeKeySettings>().step, DEFAULT_STEP))
	}

	fn render(&self, k: &Key) {
		render_volume(k, &[if self.0 > 0.0 { "imgs/actions/volume/up.svg" } else { "imgs/actions/volume/down.svg" }]);
	}
}

pub static VOLUME_UP: Kind =
	Kind { uuid: "de.felitendo.feishin.volumeup", name: "VolumeUp", watches: &[Change::Volume], repeats: true, behavior: &Step(1.0) };

pub static VOLUME_DOWN: Kind =
	Kind { uuid: "de.felitendo.feishin.volumedown", name: "VolumeDown", watches: &[Change::Volume], repeats: true, behavior: &Step(-1.0) };

// ----- Mute. State 0 is sound on, state 1 muted. -----

struct Mute;

impl Behavior for Mute {
	fn press(&self, k: &Key) -> bool {
		k.feishin().toggle_mute()
	}

	fn render(&self, k: &Key) {
		let feishin = k.feishin();
		k.set_state(u16::from(feishin.connected() && feishin.state().volume == Some(0.0)));
		render_volume(k, &["imgs/actions/volume/unmuted.svg", "imgs/actions/volume/muted.svg"]);
	}
}

pub static MUTE: Kind = Kind { uuid: "de.felitendo.feishin.mute", name: "Mute", watches: &[Change::Volume], behavior: &Mute, ..Kind::BASE };

// ----- Volume dial (Stream Deck +): turn for the volume, press or touch to mute. -----

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct VolumeDialSettings {
	/// Percent per tick.
	tick_step: f64,
}

struct VolumeDial;

impl Behavior for VolumeDial {
	fn rotate(&self, k: &Key, ticks: f64) -> bool {
		k.feishin().change_volume(ticks * positive_or(k.settings::<VolumeDialSettings>().tick_step, 2.0))
	}

	fn dial_press(&self, k: &Key) -> bool {
		k.feishin().toggle_mute()
	}

	fn render(&self, k: &Key) {
		const ICON: &str = "imgs/actions/volume/dial.svg";
		let feishin = k.feishin();
		let feedback = match feishin.state().volume.filter(|_| feishin.connected()) {
			_ if !feishin.connected() => json!({ "value": t("offline"), "indicator": { "value": 0 }, "icon": ICON }),
			// Feishin did not say its volume yet, see the hint on the settings page.
			None => json!({ "value": "–", "indicator": { "value": 0 }, "icon": ICON }),
			Some(volume) => {
				let volume = round(volume) as i64;
				json!({
					"value": if volume == 0 { t("muted") } else { format!("{volume}%") },
					"indicator": { "value": volume },
					"icon": if volume == 0 { "imgs/actions/volume/dial-muted.svg" } else { ICON },
				})
			}
		};
		k.set_feedback(feedback);
	}
}

pub static VOLUME_DIAL: Kind =
	Kind { uuid: "de.felitendo.feishin.volumedial", name: "VolumeDial", watches: &[Change::Volume], behavior: &VolumeDial, ..Kind::BASE };

#[cfg(test)]
mod tests {
	use super::super::key::testing::*;
	use super::*;
	use crate::feishin::testing::sent as to_feishin;

	#[test]
	fn volume_keys_step_and_stop_at_the_ends() {
		let (k, _sd) = key(&VOLUME_UP, json!({ "volume": 97 }), json!({ "step": "10" }));
		k.key_down(true);
		k.key_down(true);
		assert_eq!(to_feishin(k.feishin()), [json!({ "event": "volume", "volume": 100 })], "no second send at 100");
		let (down, _sd) = key(&VOLUME_DOWN, json!({ "volume": 50 }), json!({ "step": "" }));
		down.key_down(true);
		assert_eq!(down.feishin().state().volume, Some(45.0), "empty step: 5");
	}

	#[tokio::test(start_paused = true)]
	async fn the_volume_shows_on_the_key_and_mute_flips_the_state() {
		local(async {
			let (k, sd) = key(&MUTE, json!({ "volume": 50 }), json!({ "volumeAt": "above" }));
			let image =
				|name: &str, volume: &str| json!({ "target": 0, "image": svg_with_title(name, volume, SPEAKER_HEIGHT, true), "state": 0 });
			assert_eq!(sent(&sd, "setImage")[0], image("imgs/actions/volume/unmuted.svg", "50%"));
			k.key_down(false);
			k.request_render();
			later().await;
			assert_eq!(sent(&sd, "setState").last(), Some(&json!({ "state": 1 })));
			assert_eq!(sent(&sd, "setImage").len(), 4, "both states again");
			assert_eq!(sent(&sd, "setImage")[2], image("imgs/actions/volume/unmuted.svg", "0%"));
		})
		.await;
	}

	#[tokio::test(start_paused = true)]
	async fn the_dial_shows_a_dash_while_the_volume_is_not_known() {
		local(async {
			let (k, sd) = key(&VOLUME_DIAL, json!({}), no_settings());
			assert_eq!(
				sent(&sd, "setFeedback"),
				[json!({ "value": "–", "indicator": { "value": 0 }, "icon": "imgs/actions/volume/dial.svg" })]
			);
			k.feishin().assume_volume(40.0);
			k.rotate(-3.0);
			assert_eq!(k.feishin().state().volume, Some(34.0));
			k.request_render();
			later().await;
			let feedback = json!({ "value": "34%", "indicator": { "value": 34 }, "icon": "imgs/actions/volume/dial.svg" });
			assert_eq!(sent(&sd, "setFeedback").last(), Some(&feedback));
		})
		.await;
	}
}
