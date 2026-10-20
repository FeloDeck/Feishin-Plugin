// Key images as SVG data URLs. The icons come from the plugin folder, built in. Data URLs work in
// every app: OpenDeck adds an extension to a file path, so "none.svg" would become "none.svg.svg".
use base64::Engine;

use crate::js::{fmt, round};

/// The icons the keys draw themselves, by their path in the plugin folder.
static ICONS: &[(&str, &str)] = &[
	("imgs/actions/play-pause/play.svg", include_str!("../../de.felitendo.feishin.sdPlugin/imgs/actions/play-pause/play.svg")),
	("imgs/actions/play-pause/pause.svg", include_str!("../../de.felitendo.feishin.sdPlugin/imgs/actions/play-pause/pause.svg")),
	("imgs/actions/repeat/none.svg", include_str!("../../de.felitendo.feishin.sdPlugin/imgs/actions/repeat/none.svg")),
	("imgs/actions/repeat/all.svg", include_str!("../../de.felitendo.feishin.sdPlugin/imgs/actions/repeat/all.svg")),
	("imgs/actions/repeat/one.svg", include_str!("../../de.felitendo.feishin.sdPlugin/imgs/actions/repeat/one.svg")),
	("imgs/actions/volume/up.svg", include_str!("../../de.felitendo.feishin.sdPlugin/imgs/actions/volume/up.svg")),
	("imgs/actions/volume/down.svg", include_str!("../../de.felitendo.feishin.sdPlugin/imgs/actions/volume/down.svg")),
	("imgs/actions/volume/unmuted.svg", include_str!("../../de.felitendo.feishin.sdPlugin/imgs/actions/volume/unmuted.svg")),
	("imgs/actions/volume/muted.svg", include_str!("../../de.felitendo.feishin.sdPlugin/imgs/actions/volume/muted.svg")),
];

fn icon(path: &str) -> &'static str {
	ICONS.iter().find(|(p, _)| *p == path).map_or("", |(_, svg)| svg)
}

fn data_url(svg: &str) -> String {
	format!("data:image/svg+xml;base64,{}", base64::engine::general_purpose::STANDARD.encode(svg))
}

fn escape(text: &str) -> String {
	text.replace('&', "&amp;").replace('<', "&lt;")
}

/// An icon of the plugin folder as a data URL.
pub fn svg_image(path: &str) -> String {
	data_url(icon(path))
}

/// Keys are drawn on 144 x 144.
const KEY_SIZE: f64 = 144.0;
/// The font of the titles OpenDeck draws (font size 11 is about 22 px on a key), but bold to read it
/// on the deck.
const TITLE_FONT: &str = "'Liberation Sans',Arial,sans-serif";
const TITLE_SIZE: f64 = 22.0;
/// How tall digits are in that size, to place the line.
const TITLE_INK: f64 = 16.0;
/// Space between icon and text.
const TITLE_GAP: f64 = 16.0;

/// An icon with a line of text under or above it, both together in the middle of the key. Keys draw
/// the text themselves: OpenDeck puts its title at the bottom edge, far from the icon.
/// The icon must be in the middle of its square viewBox. height: how tall the icon is, in units of
/// its viewBox.
pub fn svg_with_title(path: &str, title: &str, height: f64, above: bool) -> String {
	let svg = icon(path);
	let view_box: Vec<f64> = svg
		.split("viewBox=\"")
		.nth(1)
		.and_then(|rest| rest.split('"').next())
		.unwrap_or("0 0 144 144")
		.split(|c: char| c.is_whitespace() || c == ',')
		.filter(|n| !n.is_empty())
		.map(|n| n.parse().unwrap_or(f64::NAN))
		.collect();
	let body = match (svg.find("<svg"), svg.rfind("</svg>")) {
		(Some(start), Some(end)) => svg[start..end].split_once('>').map_or("", |(_, body)| body),
		_ => "",
	};
	let (x, y, width) =
		(view_box.first().copied().unwrap_or(0.0), view_box.get(1).copied().unwrap_or(0.0), view_box.get(2).copied().unwrap_or(KEY_SIZE));
	let scale = KEY_SIZE / width;
	// The icon moves away from the text by half of what the text needs, so both are in the middle.
	let shift = (TITLE_GAP + TITLE_INK) / 2.0;
	let ink = height * scale;
	let baseline = if above { (KEY_SIZE - ink) / 2.0 + shift - TITLE_GAP } else { (KEY_SIZE + ink) / 2.0 + shift };
	let size = fmt(KEY_SIZE);
	let svg = format!(
		r##"<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" viewBox="0 0 {size} {size}"><g transform="translate({} {}) scale({})">{body}</g><text x="{}" y="{}" font-family="{TITLE_FONT}" font-size="{}" font-weight="700" fill="#FFFFFF" text-anchor="middle">{}</text></svg>"##,
		fmt(-x * scale),
		fmt(-y * scale + if above { shift } else { -shift }),
		fmt(scale),
		fmt(KEY_SIZE / 2.0),
		fmt(round(baseline * 10.0) / 10.0),
		fmt(TITLE_SIZE),
		escape(title),
	);
	data_url(&svg)
}

// Font, colors and glyphs of the status image of OBS Tools, so both look the same.
const STATUS_FONT: &str = "'Inter','Noto Sans','DejaVu Sans','Liberation Sans',sans-serif";
const GRAY: &str = "#6B6B6B";
const MUTED: &str = "#9A9AA2";
const AMBER: &str = "#F5A524";

/// Shown on every key while Feishin cannot be reached: a gray power glyph, or an amber lock when
/// Feishin is there but something is wrong (like the password), with a word or two under it.
pub fn status_image(label: &str, problem: bool) -> String {
	let glyph = if problem {
		format!(
			r#"<g transform="translate(44 22) scale(2.333)" fill="none" stroke="{AMBER}" stroke-width="2.248" stroke-linecap="round" stroke-linejoin="round"><g transform="translate(12 12) scale(1.067) translate(-12 -12.396)"><rect x="5" y="11" width="14" height="10" rx="2" fill="{AMBER}" stroke="none"/><path d="M8 11V7a4 4 0 0 1 8 0v4"/></g></g>"#
		)
	} else {
		format!(
			r#"<g transform="translate(44 22) scale(2.333)" fill="none" stroke="{GRAY}" stroke-width="2.862" stroke-linecap="round" stroke-linejoin="round"><g transform="translate(12 12) scale(0.839) translate(-12 -13.103)"><path d="M12 3v9"/><path d="M18.4 6.6a9 9 0 1 1-12.8 0"/></g></g>"#
		)
	};
	// Smaller for longer text: half the font size per letter, so "Feishin offline" still fits at 17 px.
	let letters = label.encode_utf16().count().max(1) as f64;
	let size = (132.0 / (letters * 0.5)).floor().clamp(12.0, 20.0);
	let svg = format!(
		r#"<svg xmlns="http://www.w3.org/2000/svg" width="144" height="144" viewBox="0 0 144 144">{glyph}<text x="72" y="112" font-family="{STATUS_FONT}" font-size="{}" font-weight="700" fill="{}" text-anchor="middle">{}</text></svg>"#,
		fmt(size),
		if problem { AMBER } else { MUTED },
		escape(label),
	);
	data_url(&svg)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn decode(url: &str) -> String {
		let data = url.strip_prefix("data:image/svg+xml;base64,").unwrap_or_default();
		String::from_utf8(base64::engine::general_purpose::STANDARD.decode(data).unwrap_or_default()).unwrap_or_default()
	}

	#[test]
	fn every_icon_is_built_in() {
		assert!(ICONS.iter().all(|(_, svg)| svg.contains("viewBox=\"-8 -8 40 40\"")));
		assert_eq!(decode(&svg_image("imgs/actions/repeat/one.svg")), icon("imgs/actions/repeat/one.svg"));
	}

	#[test]
	fn the_title_goes_under_or_above_the_icon() {
		let below = decode(&svg_with_title("imgs/actions/volume/up.svg", "50%", 15.6, false));
		assert!(below.starts_with(r#"<svg xmlns="http://www.w3.org/2000/svg" width="144" height="144" viewBox="0 0 144 144"><g transform="translate(28.8 12.8) scale(3.6)"><path"#), "{below}");
		assert!(below.ends_with(r##"<text x="72" y="116.1" font-family="'Liberation Sans',Arial,sans-serif" font-size="22" font-weight="700" fill="#FFFFFF" text-anchor="middle">50%</text></svg>"##), "{below}");
		let above = decode(&svg_with_title("imgs/actions/play-pause/pause.svg", "-2:31 <&>", 15.5, true));
		assert!(
			above.contains(r#"translate(28.8 44.8)"#) && above.contains(r#"y="44.1""#) && above.contains("-2:31 &lt;&amp;>"),
			"{above}"
		);
	}

	// test/svg-with-title.txt: what svgWithTitle of the Stream Deck app version (src/images.ts) makes.
	// test/pictures.mts writes it.
	#[test]
	fn pictures_are_the_same_as_in_the_stream_deck_app_version() {
		let cases = [
			("imgs/actions/volume/up.svg", "50%", 15.6, false),
			("imgs/actions/play-pause/pause.svg", "-2:31 <&>", 15.5, true),
			("imgs/actions/play-pause/play.svg", "1:05", 15.5, false),
			("imgs/actions/volume/muted.svg", "0%", 15.6, true),
		];
		let expected: Vec<&str> = include_str!("../test/svg-with-title.txt").lines().collect();
		assert_eq!(expected.len(), cases.len());
		for ((path, title, height, above), expected) in cases.into_iter().zip(expected) {
			assert_eq!(svg_with_title(path, title, height, above), format!("data:image/svg+xml;base64,{expected}"), "{path}");
		}
	}

	// test/status-image.txt: what statusImage of src/images.ts makes.
	#[test]
	fn status_images_are_the_same_as_in_the_stream_deck_app_version() {
		let cases =
			[("Feishin offline", false), ("Password?", true), ("Feishin-Fehler", true), ("Nicht verbunden mit Feishin hier", false)];
		let expected: Vec<&str> = include_str!("../test/status-image.txt").lines().collect();
		assert_eq!(expected.len(), cases.len());
		for ((label, problem), expected) in cases.into_iter().zip(expected) {
			assert_eq!(status_image(label, problem), format!("data:image/svg+xml;base64,{expected}"), "{label}");
		}
	}

	#[test]
	fn long_status_texts_get_smaller() {
		assert!(decode(&status_image("Feishin offline", false)).contains(r##"font-size="17" font-weight="700" fill="#9A9AA2""##));
		assert!(decode(&status_image("Password?", true)).contains(r##"font-size="20" font-weight="700" fill="#F5A524""##));
	}
}
