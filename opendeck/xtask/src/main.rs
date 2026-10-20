// Tools to build, package and test the Feishin plugin for OpenDeck. Run `cargo xtask` for the list.
mod deck;
mod e2e;
mod fake;
mod package;
mod profile;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "cargo xtask <command>

  build [target...]     build the plugin into build/de.felitendo.feishin.sdPlugin (default: this PC)
  package               build for x86_64 and aarch64 and write dist/*.streamDeckPlugin
  profile <layout.json> [name]
                        write an OpenDeck profile for the virtual deck (stop OpenDeck first)
  fake-feishin          run a fake Feishin on port 4333, to try the keys without Feishin
  e2e [filter]          end-to-end test through OpenDeck, the virtual deck and a fake Feishin";

/// The folder of the Rust version (opendeck/).
pub fn root() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().map(PathBuf::from).unwrap_or_default()
}

/// The plugin folder of the Stream Deck app version. The OpenDeck version uses its pictures, settings
/// page and texts.
pub fn shared_dir() -> PathBuf {
	root().parent().map(PathBuf::from).unwrap_or_default().join("de.felitendo.feishin.sdPlugin")
}

/// The plugin folder for OpenDeck that `build` puts together.
pub fn plugin_dir() -> PathBuf {
	root().join("build").join("de.felitendo.feishin.sdPlugin")
}

pub type Result<T = ()> = std::result::Result<T, String>;

/// Writes a file, with the folders it needs.
pub fn write(path: &Path, content: impl AsRef<[u8]>) -> Result {
	if let Some(dir) = path.parent() {
		std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
	}
	std::fs::write(path, content).map_err(|e| format!("{}: {e}", path.display()))
}

fn main() -> ExitCode {
	let args: Vec<String> = std::env::args().skip(1).collect();
	let rest = args.get(1..).unwrap_or(&[]);
	let result = match args.first().map(String::as_str) {
		Some("build") => package::build(rest),
		Some("package") => package::run(),
		Some("profile") => profile::run(rest),
		Some("fake-feishin") => fake::run(),
		Some("e2e") => e2e::run(rest),
		_ => Err(USAGE.to_string()),
	};
	match result {
		Ok(()) => ExitCode::SUCCESS,
		Err(error) => {
			eprintln!("{error}");
			ExitCode::FAILURE
		}
	}
}
