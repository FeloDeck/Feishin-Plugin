// Puts the plugin folder for OpenDeck together and writes the plugin file for the plugin store. The
// folder has the files of the Stream Deck app version (pictures, settings page, texts), its own
// manifest and the binaries instead of Node.js.
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Map, Value, json};
use zip::write::SimpleFileOptions;

use crate::{Result, plugin_dir, root, shared_dir, write};

/// The targets the release is built for.
pub const TARGETS: &[&str] = &["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"];
const PACKAGE: &str = "feishin-plugin";
/// The oldest glibc the release runs with (Debian 10, Ubuntu 18.10). cargo-zigbuild links against it.
const GLIBC: &str = "2.28";
/// Files of the Stream Deck app version that the OpenDeck version does not take.
const NOT_SHARED: &[&str] = &["bin", "logs", "manifest.json"];

fn binary_path(target: &str) -> String {
	format!("bin/{target}/{PACKAGE}")
}

/// JSON.stringify(value, null, "\t")
fn pretty(value: &Value) -> String {
	use serde::Serialize;
	let mut out = Vec::new();
	let mut serializer = serde_json::Serializer::with_formatter(&mut out, serde_json::ser::PrettyFormatter::with_indent(b"\t"));
	let _ = value.serialize(&mut serializer);
	String::from_utf8(out).unwrap_or_default()
}

/// The manifest of the Stream Deck app version, for Linux and the binaries.
fn manifest() -> Result<Value> {
	let file = shared_dir().join("manifest.json");
	let text = fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
	let shared: Map<String, Value> = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", file.display()))?;
	let mut out = Map::new();
	for (key, value) in shared {
		match key.as_str() {
			// OpenDeck picks the binary for its CPU from CodePaths. Older versions take CodePath.
			"CodePath" => {
				out.insert(key, binary_path(TARGETS[0]).into());
				out.insert("CodePaths".into(), TARGETS.iter().map(|t| (t.to_string(), Value::from(binary_path(t)))).collect());
			}
			"OS" => {
				out.insert(key, json!([{ "Platform": "linux" }]));
			}
			"Nodejs" | "Software" => {}
			_ => {
				out.insert(key, value);
			}
		}
	}
	Ok(Value::Object(out))
}

fn copy_dir(from: &Path, to: &Path, skip: &[&str]) -> Result {
	fs::create_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
	for entry in fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))?.flatten() {
		let name = entry.file_name();
		if skip.iter().any(|s| name == *s) {
			continue;
		}
		let (from, to) = (entry.path(), to.join(&name));
		if from.is_dir() {
			copy_dir(&from, &to, &[])?;
		} else {
			fs::copy(&from, &to).map_err(|e| format!("{}: {e}", from.display()))?;
		}
	}
	Ok(())
}

/// The plugin folder without binaries: everything else new, the binaries stay.
fn assemble() -> Result {
	let dir = plugin_dir();
	if let Ok(entries) = fs::read_dir(&dir) {
		for entry in entries.flatten().filter(|e| e.file_name() != "bin") {
			let path = entry.path();
			let removed = if path.is_dir() { fs::remove_dir_all(&path) } else { fs::remove_file(&path) };
			removed.map_err(|e| format!("{}: {e}", path.display()))?;
		}
	}
	copy_dir(&shared_dir(), &dir, NOT_SHARED)?;
	write(&dir.join("manifest.json"), pretty(&manifest()?) + "\n")
}

fn target_dir() -> PathBuf {
	std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from).unwrap_or_else(|| root().join("target"))
}

fn exec(command: &mut Command) -> Result {
	let status = command.status().map_err(|e| format!("{command:?}: {e}"))?;
	if status.success() { Ok(()) } else { Err(format!("{command:?} failed")) }
}

fn host() -> Result<String> {
	let output = Command::new("rustc").arg("-vV").output().map_err(|e| format!("rustc: {e}"))?;
	String::from_utf8_lossy(&output.stdout)
		.lines()
		.find_map(|l| l.strip_prefix("host: ").map(String::from))
		.ok_or_else(|| "rustc did not say its host".into())
}

fn install_binary(built: &Path, target: &str) -> Result {
	let to = plugin_dir().join(binary_path(target));
	if let Some(dir) = to.parent() {
		fs::create_dir_all(dir).map_err(|e| e.to_string())?;
	}
	fs::copy(built, &to).map_err(|e| format!("{}: {e}", built.display()))?;
	println!("{}", to.strip_prefix(root()).unwrap_or(&to).display());
	Ok(())
}

/// Builds for this PC, or with cargo-zigbuild for the given targets (an old glibc, so it runs on
/// older distributions too).
pub fn build(targets: &[String]) -> Result {
	assemble()?;
	if targets.is_empty() {
		exec(Command::new("cargo").args(["build", "--release", "--package", PACKAGE]).current_dir(root()))?;
		return install_binary(&target_dir().join("release").join(PACKAGE), &host()?);
	}
	for target in targets {
		let zigbuild = Command::new("cargo")
			.args(["zigbuild", "--release", "--package", PACKAGE, "--target", &format!("{target}.{GLIBC}")])
			.current_dir(root())
			.status()
			.map_err(|e| format!("cargo zigbuild: {e}"))?;
		if !zigbuild.success() {
			return Err("cargo zigbuild failed. It needs zig and cargo-zigbuild (cargo install --locked cargo-zigbuild), and the Rust library of the target (rustup target add, or the rust-aarch64-gnu package on Arch).".into());
		}
		install_binary(&target_dir().join(target).join("release").join(PACKAGE), target)?;
	}
	Ok(())
}

fn add_dir(zip: &mut zip::ZipWriter<fs::File>, dir: &Path, base: &Path) -> Result {
	let mut entries: Vec<PathBuf> = fs::read_dir(dir).map_err(|e| e.to_string())?.flatten().map(|e| e.path()).collect();
	entries.sort();
	for path in entries {
		let name = path.strip_prefix(base).map_err(|e| e.to_string())?.to_string_lossy().replace('\\', "/");
		if path.is_dir() {
			zip.add_directory(format!("{name}/"), SimpleFileOptions::default().unix_permissions(0o755)).map_err(|e| e.to_string())?;
			add_dir(zip, &path, base)?;
		} else {
			let executable = name.contains("/bin/");
			let options = SimpleFileOptions::default()
				.compression_method(zip::CompressionMethod::Deflated)
				.unix_permissions(if executable { 0o755 } else { 0o644 });
			zip.start_file(name, options).map_err(|e| e.to_string())?;
			zip.write_all(&fs::read(&path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
		}
	}
	Ok(())
}

/// dist/de.felitendo.feishin.streamDeckPlugin, with the binaries for x86_64 and aarch64.
pub fn run() -> Result {
	let _ = fs::remove_dir_all(plugin_dir().join("bin"));
	build(&TARGETS.iter().map(|t| t.to_string()).collect::<Vec<_>>())?;
	let dist = root().join("dist");
	let _ = fs::remove_dir_all(&dist);
	fs::create_dir_all(&dist).map_err(|e| e.to_string())?;
	let file = dist.join("de.felitendo.feishin.streamDeckPlugin");
	let mut zip = zip::ZipWriter::new(fs::File::create(&file).map_err(|e| e.to_string())?);
	let dir = plugin_dir();
	let base = dir.parent().unwrap_or(&dir).to_path_buf();
	let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
	zip.add_directory(format!("{name}/"), SimpleFileOptions::default().unix_permissions(0o755)).map_err(|e| e.to_string())?;
	add_dir(&mut zip, &dir, &base)?;
	zip.finish().map_err(|e| e.to_string())?;
	println!("dist/de.felitendo.feishin.streamDeckPlugin (version {})", feishin_plugin::version());
	Ok(())
}
