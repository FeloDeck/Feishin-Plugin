// Writes an OpenDeck profile for the virtual deck, so tests do not need drag and drop.
// Stop OpenDeck first, it keeps profiles in memory.
//
//   cargo xtask profile <layout.json> [profile name]
//
// layout.json: { "keys": { "0": { "action": "de.felitendo.feishin.playpause", "settings": {...} } },
//                "dials": { "0": { "action": "...", "settings": {...} } } }
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::Result;

const DEVICE: &str = "vd-virtual-deck";

fn home() -> PathBuf {
	std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

pub fn config_dir() -> PathBuf {
	std::env::var_os("XDG_CONFIG_HOME")
		.filter(|v| !v.is_empty())
		.map(PathBuf::from)
		.unwrap_or_else(|| home().join(".config"))
		.join("opendeck")
}

fn env_number(name: &str, fallback: usize) -> usize {
	std::env::var(name).ok().and_then(|v| v.parse().ok()).filter(|n| *n > 0).unwrap_or(fallback)
}

struct Profiles {
	config: PathBuf,
	manifests: HashMap<String, Value>,
}

// Paths relative to the config folder, like OpenDeck keeps them.
fn relative(config: &Path, path: &Path) -> String {
	path.strip_prefix(config).unwrap_or(path).to_string_lossy().into_owned()
}

impl Profiles {
	fn plugin_for(&mut self, uuid: &str) -> Result<(String, Value, Value)> {
		let plugins = self.config.join("plugins");
		let mut dirs: Vec<String> = fs::read_dir(&plugins)
			.map_err(|e| format!("{}: {e}", plugins.display()))?
			.flatten()
			.map(|e| e.file_name().to_string_lossy().into_owned())
			.collect();
		dirs.sort();
		for dir in dirs {
			let file = plugins.join(&dir).join("manifest.json");
			if !file.exists() {
				continue;
			}
			if !self.manifests.contains_key(&dir) {
				let manifest = fs::read_to_string(&file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
				self.manifests.insert(dir.clone(), manifest);
			}
			let manifest = &self.manifests[&dir];
			if let Some(action) = manifest["Actions"].as_array().and_then(|a| a.iter().find(|a| a["UUID"] == uuid)) {
				return Ok((dir.clone(), manifest.clone(), action.clone()));
			}
		}
		Err(format!("no installed plugin has the action {uuid}"))
	}

	// Same lookup as OpenDeck: prefer .svg, then @2x.png, then .png.
	fn icon(&self, dir: &str, file: &str) -> String {
		let base = self.config.join("plugins").join(dir).join(file);
		for ext in [".svg", "@2x.png", ".png"] {
			let path = PathBuf::from(format!("{}{ext}", base.display()));
			if path.exists() {
				return relative(&self.config, &path);
			}
		}
		relative(&self.config, &PathBuf::from(format!("{}.png", base.display())))
	}

	fn state(&self, dir: &str, s: &Value, action_icon: &str) -> Value {
		let image = match s["Image"].as_str().filter(|i| !i.is_empty()) {
			Some(image) => self.icon(dir, image),
			None => action_icon.to_string(),
		};
		json!({
			"image": image,
			"image_scale": 100,
			"background_colour": "#000000",
			"name": s["Name"].as_str().unwrap_or(""),
			"text": s["Title"].as_str().unwrap_or(""),
			"show": s["ShowTitle"].as_bool().unwrap_or(true),
			"colour": s["TitleColor"].as_str().unwrap_or("#FFFFFF"),
			"stroke_colour": "#000000",
			"alignment": s["TitleAlignment"].as_str().unwrap_or("middle"),
			"family": s["FontFamily"].as_str().unwrap_or("Liberation Sans"),
			"style": s["FontStyle"].as_str().unwrap_or("Regular"),
			"size": s.get("FontSize").filter(|v| v.is_number()).cloned().unwrap_or(json!(16)),
			"stroke_size": 3,
			"underline": s["FontUnderline"].as_bool().unwrap_or(false),
		})
	}

	// Multi and toggle actions from OpenDeck itself, with plugin actions as children.
	fn builtin(&mut self, position: usize, spec: &Value, name: &str, image: &str, tooltip: &str) -> Result<Value> {
		let mut state = self.state("", &json!({ "Image": "" }), image);
		state["image"] = image.into();
		let states = json!([state]);
		let mut children = Vec::new();
		for (i, child) in spec["children"].as_array().cloned().unwrap_or_default().iter().enumerate() {
			children.push(self.instance("Keypad", position, child, i + 1)?);
		}
		Ok(json!({
			"action": {
				"name": name,
				"uuid": spec["action"],
				"plugin": "opendeck",
				"tooltip": tooltip,
				"icon": image,
				"disable_automatic_states": false,
				"visible_in_action_list": true,
				"supported_in_multi_actions": false,
				"property_inspector": "",
				"controllers": ["Keypad"],
				"encoder": null,
				"states": states,
			},
			"context": format!("Keypad.{position}.0"),
			"states": states,
			"current_state": 0,
			"settings": {},
			"children": children,
		}))
	}

	fn instance(&mut self, controller: &str, position: usize, spec: &Value, index: usize) -> Result<Value> {
		match spec["action"].as_str().unwrap_or("") {
			"opendeck.multiaction" => {
				return self.builtin(position, spec, "Multi Action", "opendeck/multi-action.png", "Execute multiple actions");
			}
			"opendeck.toggleaction" => {
				return self.builtin(position, spec, "Toggle Action", "opendeck/toggle-action.png", "Cycle through multiple actions");
			}
			_ => {}
		}
		let uuid = spec["action"].as_str().unwrap_or("");
		let (dir, manifest, action) = self.plugin_for(uuid)?;
		let action_icon = self.icon(&dir, action["Icon"].as_str().unwrap_or(""));
		let states: Vec<Value> =
			action["States"].as_array().cloned().unwrap_or_default().iter().map(|s| self.state(&dir, s, &action_icon)).collect();
		let encoder = match action.get("Encoder").filter(|e| e.is_object()) {
			Some(e) => {
				let trigger = |k: &str| e["TriggerDescription"][k].as_str().unwrap_or("").to_string();
				json!({
					"icon": e["Icon"].as_str().map(|i| self.icon(&dir, i)).unwrap_or_default(),
					"stack_color": "",
					"trigger_description": { "long_touch": trigger("LongTouch"), "push": trigger("Push"), "rotate": trigger("Rotate"), "touch": trigger("Touch") },
					"background": "",
					"layout": e["layout"].as_str().unwrap_or("$X1"),
				})
			}
			None => Value::Null,
		};
		let inspector = action["PropertyInspectorPath"].as_str().or(manifest["PropertyInspectorPath"].as_str()).unwrap_or("");
		Ok(json!({
			"action": {
				"name": action["Name"],
				"uuid": action["UUID"],
				"plugin": dir,
				"tooltip": action["Tooltip"].as_str().unwrap_or(""),
				"icon": action_icon,
				"disable_automatic_states": action["DisableAutomaticStates"].as_bool().unwrap_or(false),
				"visible_in_action_list": action["VisibleInActionsList"].as_bool().unwrap_or(true),
				"supported_in_multi_actions": action["SupportedInMultiActions"].as_bool().unwrap_or(true),
				"property_inspector": relative(&self.config, &self.config.join("plugins").join(&dir).join(inspector)),
				"controllers": action.get("Controllers").cloned().unwrap_or(json!(["Keypad"])),
				"encoder": encoder,
				"states": states,
			},
			"context": format!("{controller}.{position}.{index}"),
			"states": states,
			"current_state": 0,
			"settings": expand_home(spec.get("settings").cloned().unwrap_or(json!({}))),
			"children": null,
		}))
	}
}

// "~/" at the start of a text setting means the home folder.
fn expand_home(value: Value) -> Value {
	match value {
		Value::String(s) => match s.strip_prefix("~/") {
			Some(rest) => Value::from(home().join(rest).to_string_lossy().into_owned()),
			None => Value::String(s),
		},
		Value::Array(items) => Value::Array(items.into_iter().map(expand_home).collect()),
		Value::Object(map) => Value::Object(map.into_iter().map(|(k, v)| (k, expand_home(v))).collect::<Map<_, _>>()),
		other => other,
	}
}

/// Writes the profile and selects it.
pub fn write_profile(layout: &Value, name: &str) -> Result {
	let config = config_dir();
	let mut profiles = Profiles { config: config.clone(), manifests: HashMap::new() };
	let rows = env_number("VIRTUAL_DECK_ROWS", 3);
	let columns = env_number("VIRTUAL_DECK_COLUMNS", 5);
	let dials = env_number("VIRTUAL_DECK_DIALS", 4);
	let mut keys = vec![Value::Null; rows * columns];
	let mut sliders = vec![Value::Null; dials];
	for (list, controller, slots) in [("keys", "Keypad", &mut keys), ("dials", "Encoder", &mut sliders)] {
		for (position, spec) in layout[list].as_object().cloned().unwrap_or_default() {
			let position: usize = position.parse().map_err(|_| format!("bad position {position}"))?;
			let instance = profiles.instance(controller, position, &spec, 0)?;
			*slots.get_mut(position).ok_or(format!("no {list} slot {position}"))? = instance;
		}
	}
	let profile = json!({ "keys": keys, "sliders": sliders, "infobars": [] });
	let dir = config.join("profiles").join(DEVICE);
	fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
	let pretty = |v: &Value| serde_json::to_string_pretty(v).unwrap_or_default();
	fs::write(dir.join(format!("{name}.json")), pretty(&profile)).map_err(|e| e.to_string())?;
	select(name)?;
	println!("wrote profile \"{name}\" for {DEVICE}");
	Ok(())
}

/// Selects a profile of the virtual deck (OpenDeck must be stopped).
pub fn select(name: &str) -> Result {
	let file = config_dir().join("profiles").join(format!("{DEVICE}.json"));
	fs::write(&file, serde_json::to_string_pretty(&json!({ "selected_profile": name })).unwrap_or_default())
		.map_err(|e| format!("{}: {e}", file.display()))
}

pub fn run(args: &[String]) -> Result {
	let Some(file) = args.first() else { return Err("usage: cargo xtask profile <layout.json> [profile name]".into()) };
	let name = args.get(1).map_or("Feishin Test", String::as_str);
	let layout: Value =
		serde_json::from_str(&fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?).map_err(|e| format!("{file}: {e}"))?;
	write_profile(&layout, name)
}
