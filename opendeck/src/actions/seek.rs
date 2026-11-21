// Seeking: fast forward and rewind (holding keeps going), and the seek dial.
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Behavior, Key, Kind, positive_or};
use crate::feishin::{Change, Playback};
use crate::i18n::t;
use crate::images::no_cover;

/// 300 → "5:00", 3723 → "1:02:03"
pub fn format_time(seconds: f64) -> String {
	let total = seconds.floor().max(0.0) as u64;
	let (h, m, s) = (total / 3600, total % 3600 / 60, total % 60);
	if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct SeekSettings {
	/// Seconds per press.
	seconds: f64,
}

/// Forward (1) or back (-1).
struct Seek(f64);

impl Behavior for Seek {
	fn press(&self, k: &Key) -> bool {
		k.feishin().seek_by(self.0 * positive_or(k.settings::<SeekSettings>().seconds, 10.0))
	}
}

pub static FORWARD: Kind =
	Kind { uuid: "de.felitendo.feishin.forward", name: "FastForward", repeats: true, behavior: &Seek(1.0), ..Kind::BASE };

pub static REWIND: Kind = Kind { uuid: "de.felitendo.feishin.rewind", name: "Rewind", repeats: true, behavior: &Seek(-1.0), ..Kind::BASE };

// ----- Seek dial (Stream Deck +): shows the track, with the album art on the left (unless turned -----
// off). Turn to seek, press or touch to play or pause.

#[derive(Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct SeekDialSettings {
	/// Seconds per tick.
	tick_seconds: f64,
	show_cover: bool,
}

impl Default for SeekDialSettings {
	fn default() -> Self {
		SeekDialSettings { tick_seconds: 0.0, show_cover: true }
	}
}

const LAYOUT_COVER: &str = "layouts/seek-dial-cover.json";
const LAYOUT_PLAIN: &str = "layouts/seek-dial.json";

struct SeekDial;

impl Behavior for SeekDial {
	fn rotate(&self, k: &Key, ticks: f64) -> bool {
		k.feishin().seek_by(ticks * positive_or(k.settings::<SeekDialSettings>().tick_seconds, 5.0))
	}

	fn dial_press(&self, k: &Key) -> bool {
		k.feishin().toggle_play_pause()
	}

	fn render(&self, k: &Key) {
		let s: SeekDialSettings = k.settings();
		k.set_feedback_layout(if s.show_cover { LAYOUT_COVER } else { LAYOUT_PLAIN });
		let feishin = k.feishin();
		let state = feishin.state();
		if s.show_cover {
			// The cover is big, so it only goes out when it changed, not with every time update.
			let cover = state.song.as_ref().filter(|_| feishin.connected()).and_then(|_| feishin.cover());
			k.set_feedback_item("cover", cover.unwrap_or_else(no_cover).into());
		}
		let empty = |track: String| -> Value {
			json!({ "artist": "", "time": "", "status": { "enabled": false }, "progress": { "value": 0 }, "track": track })
		};
		let feedback = match &state.song {
			_ if !feishin.connected() => empty(t("offline")),
			None => empty(t("nothing-playing")),
			Some(song) => {
				let position = feishin.position();
				let status = if state.playback == Playback::Playing { "playing" } else { "paused" };
				// Whole percent: about one update a second, together with the time.
				let progress = if song.duration > 0.0 { crate::js::round(position / song.duration * 100.0) as i64 } else { 0 };
				json!({
					"track": song.name,
					"artist": song.artist,
					"time": format!("{} / {}", format_time(position), format_time(song.duration)),
					"status": { "enabled": true, "value": format!("imgs/actions/seek/{status}.svg") },
					"progress": { "value": progress },
				})
			}
		};
		k.set_feedback(feedback);
	}
}

pub static SEEK_DIAL: Kind = Kind {
	uuid: "de.felitendo.feishin.seekdial",
	name: "SeekDial",
	watches: &[Change::Song, Change::Playback, Change::Position, Change::Cover],
	behavior: &SeekDial,
	..Kind::BASE
};

#[cfg(test)]
mod tests {
	use super::super::key::testing::*;
	use super::*;
	use crate::feishin::testing::sent as to_feishin;

	#[test]
	fn times() {
		assert_eq!(format_time(300.0), "5:00");
		assert_eq!(format_time(3723.9), "1:02:03");
		assert_eq!(format_time(-4.0), "0:00");
	}

	#[test]
	fn rewind_jumps_back_by_its_seconds() {
		let (k, _sd) =
			key(&REWIND, json!({ "status": "paused", "position": 50, "song": { "duration": 120000 } }), json!({ "seconds": "15" }));
		k.key_down(true);
		assert_eq!(to_feishin(k.feishin()), [json!({ "event": "position", "position": 35.0 })]);
	}

	#[test]
	fn the_seek_dial_shows_the_track() {
		let (k, sd) = key(
			&SEEK_DIAL,
			json!({ "status": "paused", "position": 60, "song": { "name": "Song", "artistName": "Artist", "duration": 240000 } }),
			no_settings(),
		);
		assert_eq!(sent(&sd, "setFeedbackLayout"), [json!({ "layout": LAYOUT_COVER })]);
		assert_eq!(
			sent(&sd, "setFeedback"),
			[
				json!({ "cover": no_cover() }),
				json!({
					"track": "Song",
					"artist": "Artist",
					"time": "1:00 / 4:00",
					"status": { "enabled": true, "value": "imgs/actions/seek/paused.svg" },
					"progress": { "value": 25 },
				})
			]
		);
		k.rotate(2.0);
		assert_eq!(to_feishin(k.feishin()), [json!({ "event": "position", "position": 70.0 })]);
	}

	#[tokio::test(start_paused = true)]
	async fn the_seek_dial_shows_the_cover_once_it_is_there() {
		local(async {
			let song = json!({ "name": "Song", "duration": 180000, "imageUrl": "http://music/cover" });
			let (k, sd) = key(&SEEK_DIAL, json!({ "status": "playing", "position": 29, "song": song }), no_settings());
			assert_eq!(to_feishin(k.feishin()), [json!({ "event": "proxy" })]);
			k.feishin().on_message(&json!({ "event": "proxy", "data": crate::cover::testing::png(8, 8) }));
			k.request_render();
			later().await;
			let cover = k.feishin().cover().unwrap_or_default();
			let covers: Vec<_> = sent(&sd, "setFeedback").into_iter().filter(|f| f.get("cover").is_some()).collect();
			assert_eq!(covers, [json!({ "cover": no_cover() }), json!({ "cover": cover })]);
			// Not again with the time.
			k.request_render();
			later().await;
			assert_eq!(sent(&sd, "setFeedback").iter().filter(|f| f.get("cover").is_some()).count(), 2);
		})
		.await;
	}

	#[test]
	fn the_seek_dial_can_hide_the_cover() {
		let (k, sd) = key(&SEEK_DIAL, json!({ "song": { "name": "Song", "duration": 240000 } }), json!({ "showCover": false }));
		assert_eq!(sent(&sd, "setFeedbackLayout"), [json!({ "layout": LAYOUT_PLAIN })]);
		assert_eq!(sent(&sd, "setFeedback").len(), 1, "no cover");
		// The other layout starts empty: everything again.
		k.update_settings(Some(&json!({ "showCover": true })));
		assert_eq!(sent(&sd, "setFeedbackLayout")[1], json!({ "layout": LAYOUT_COVER }));
		assert_eq!(sent(&sd, "setFeedback").len(), 3);
	}
}
