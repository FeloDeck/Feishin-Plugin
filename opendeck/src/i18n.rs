// Texts on the keys and dials, from "Localization" in en.json and de.json of the plugin folder. The
// Stream Deck app version uses the same files.
use std::sync::OnceLock;

use serde_json::Value;

static EN: &str = include_str!("../../de.felitendo.feishin.sdPlugin/en.json");
static DE: &str = include_str!("../../de.felitendo.feishin.sdPlugin/de.json");

static GERMAN: OnceLock<bool> = OnceLock::new();

fn localization(json: &str) -> Value {
	serde_json::from_str::<Value>(json).map(|v| v["Localization"].clone()).unwrap_or_default()
}

/// The language of OpenDeck, like "de" or "en". Returns the one in use.
pub fn set_language(language: &str) -> &'static str {
	let german = *GERMAN.get_or_init(|| language.to_lowercase().starts_with("de"));
	if german { "de" } else { "en" }
}

/// The text of a key of "Localization", in English when German has none.
pub fn t(key: &str) -> String {
	static TEXTS: OnceLock<(Value, Value)> = OnceLock::new();
	let (en, de) = TEXTS.get_or_init(|| (localization(EN), localization(DE)));
	let german = GERMAN.get().copied().unwrap_or(false);
	let text = if german { de[key].as_str().or(en[key].as_str()) } else { en[key].as_str() };
	text.unwrap_or(key).to_string()
}

/// The keys the plugin uses.
pub const KEYS: &[&str] = &["offline", "muted", "nothing-playing", "status-offline", "status-password", "status-error"];

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn every_text_is_in_english_and_german() {
		let (en, de) = (localization(EN), localization(DE));
		for key in KEYS {
			assert!(en[key].is_string() && de[key].is_string(), "{key}");
		}
		assert_eq!(t("status-password"), "Password?");
	}
}
