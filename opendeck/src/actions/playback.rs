// Playback: play/pause with the time of the track, next, previous, shuffle and repeat.
use serde::Deserialize;

use super::seek::format_time;
use super::{Behavior, Key, Kind};
use crate::feishin::{Change, Playback};
use crate::images::{svg_image, svg_with_title};

// ----- Play/Pause. State 0 shows "play" (paused), state 1 "pause" (playing). The time of the track -----
// goes under or above the icon (unless turned off), drawn into the image like on the volume keys.

#[derive(Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct PlayPauseSettings {
	show_time: bool,
	/// "below" or "above".
	time_at: String,
	/// "left" or "played".
	time_shows: String,
}

impl Default for PlayPauseSettings {
	fn default() -> Self {
		PlayPauseSettings { show_time: true, time_at: String::new(), time_shows: String::new() }
	}
}

/// Icon per state: play while paused, pause while playing.
const PLAY_PAUSE_ICONS: [&str; 2] = ["imgs/actions/play-pause/play.svg", "imgs/actions/play-pause/pause.svg"];
/// How tall the play triangle is with its outline, in units of its viewBox. Pause uses it too, so the
/// time stays put.
const PLAY_HEIGHT: f64 = 15.5;

struct PlayPause;

/// The time left ("-2:31") or played ("1:05") of the track, or "" while nothing plays or pauses.
fn track_time(k: &Key, played: bool) -> String {
	let feishin = k.feishin();
	let state = feishin.state();
	let Some(song) = state.song.filter(|_| feishin.connected() && state.playback != Playback::Stopped) else { return String::new() };
	let position = feishin.position().floor();
	if played {
		format_time(position)
	} else if song.duration > 0.0 {
		format!("-{}", format_time(song.duration - position))
	} else {
		String::new()
	}
}

impl Behavior for PlayPause {
	fn press(&self, k: &Key) -> bool {
		k.feishin().toggle_play_pause()
	}

	fn render(&self, k: &Key) {
		let s: PlayPauseSettings = k.settings();
		let time = if s.show_time { track_time(k, s.time_shows == "played") } else { String::new() };
		for (state, icon) in (0..).zip(PLAY_PAUSE_ICONS) {
			k.set_state_image(state, (!time.is_empty()).then(|| svg_with_title(icon, &time, PLAY_HEIGHT, s.time_at == "above")));
		}
		k.set_state(u16::from(k.feishin().state().playback == Playback::Playing));
		// The key draws the time itself, so the title stays empty meanwhile.
		k.set_title((!time.is_empty()).then_some(""));
	}
}

pub static PLAY_PAUSE: Kind = Kind {
	uuid: "de.felitendo.feishin.playpause",
	name: "PlayPause",
	watches: &[Change::Playback, Change::Song, Change::Position],
	behavior: &PlayPause,
	..Kind::BASE
};

// ----- Next and previous -----

struct Next;

impl Behavior for Next {
	fn press(&self, k: &Key) -> bool {
		k.feishin().next()
	}
}

pub static NEXT: Kind = Kind { uuid: "de.felitendo.feishin.next", name: "Next", behavior: &Next, ..Kind::BASE };

struct Previous;

impl Behavior for Previous {
	fn press(&self, k: &Key) -> bool {
		k.feishin().previous()
	}
}

pub static PREVIOUS: Kind = Kind { uuid: "de.felitendo.feishin.previous", name: "Previous", behavior: &Previous, ..Kind::BASE };

// ----- Shuffle. State 0 is off, state 1 on. -----

struct Shuffle;

impl Behavior for Shuffle {
	fn press(&self, k: &Key) -> bool {
		k.feishin().toggle_shuffle()
	}

	fn render(&self, k: &Key) {
		k.set_state(u16::from(k.feishin().state().shuffle));
	}
}

pub static SHUFFLE: Kind =
	Kind { uuid: "de.felitendo.feishin.shuffle", name: "Shuffle", watches: &[Change::Shuffle], behavior: &Shuffle, ..Kind::BASE };

// ----- Repeat: off, all, one. Keys have only two states, so the key changes its image. -----

struct Repeat;

impl Behavior for Repeat {
	fn press(&self, k: &Key) -> bool {
		k.feishin().toggle_repeat()
	}

	fn render(&self, k: &Key) {
		k.set_image(svg_image(&format!("imgs/actions/repeat/{}.svg", k.feishin().state().repeat.name())));
	}
}

pub static REPEAT: Kind =
	Kind { uuid: "de.felitendo.feishin.repeat", name: "Repeat", watches: &[Change::Repeat], behavior: &Repeat, ..Kind::BASE };

#[cfg(test)]
mod tests {
	use serde_json::json;

	use super::super::key::testing::*;
	use super::*;
	use crate::feishin::testing::sent as to_feishin;
	use crate::images::svg_with_title;

	fn song() -> serde_json::Value {
		json!({ "name": "Song", "artistName": "Artist", "duration": 180000 })
	}

	#[tokio::test(start_paused = true)]
	async fn play_pause_shows_the_time_left_or_played() {
		let (k, sd) = key(&PLAY_PAUSE, json!({ "status": "playing", "position": 29, "song": song() }), no_settings());
		let images = sent(&sd, "setImage");
		assert_eq!(images[1]["image"].as_str(), Some(svg_with_title(PLAY_PAUSE_ICONS[1], "-2:31", PLAY_HEIGHT, false).as_str()));
		assert_eq!(sent(&sd, "setState").last(), Some(&json!({ "state": 1 })));
		assert_eq!(sent(&sd, "setTitle").last(), Some(&json!({ "target": 0, "title": "" })));
		k.update_settings(Some(&json!({ "timeShows": "played", "timeAt": "above" })));
		let image = sent(&sd, "setImage").last().cloned().unwrap_or_default();
		assert_eq!(image["image"].as_str(), Some(svg_with_title(PLAY_PAUSE_ICONS[1], "0:29", PLAY_HEIGHT, true).as_str()));
		// Turned off: the images of the manifest and the title of the user.
		k.update_settings(Some(&json!({ "showTime": false })));
		assert_eq!(sent(&sd, "setImage").last(), Some(&json!({ "target": 0, "state": 1 })));
		assert_eq!(sent(&sd, "setTitle").last(), Some(&json!({ "target": 0 })));
	}

	#[tokio::test(start_paused = true)]
	async fn play_pause_tells_feishin_and_shows_it() {
		local(async {
			let (k, sd) = key(&PLAY_PAUSE, json!({ "status": "paused" }), no_settings());
			k.key_down(false);
			assert_eq!(to_feishin(k.feishin()), [json!({ "event": "play" })]);
			k.request_render();
			later().await;
			assert_eq!(sent(&sd, "setState").last(), Some(&json!({ "state": 1 })));
		})
		.await;
	}

	#[test]
	fn repeat_shows_its_mode() {
		let (k, sd) = key(&REPEAT, json!({ "repeat": "one" }), no_settings());
		assert_eq!(sent(&sd, "setImage"), [json!({ "target": 0, "image": svg_image("imgs/actions/repeat/one.svg") })]);
		k.key_down(false);
		assert_eq!(to_feishin(k.feishin()), [json!({ "event": "repeat" })]);
	}
}
