// Client for the remote control server of Feishin (Settings → Window → Remote). One connection for
// all keys. It keeps the player state and tells the keys what changed.
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::Duration;

use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio::task::{AbortHandle, spawn_local};
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

use crate::cover::shrink_cover;
use crate::emitter::Emitter;
use crate::timers::{Timer, set_timeout};

/// What the settings page tells the user to set up in Feishin.
pub const DEFAULT_HOST: &str = "localhost";
pub const DEFAULT_PORT: &str = "4333";
pub const DEFAULT_USERNAME: &str = "feishin";
pub const DEFAULT_PASSWORD: &str = "streamdeck";

// Feishin sends the position about every 200 ms while playing, and echoes volume changes. Right after
// a change of our own, these can still have the old value, so they are ignored for a moment. Then
// pressing again (or turning a dial) does not jump back.
const SETTLE: Duration = Duration::from_millis(700);
const RECONNECT_DELAYS_MS: [f64; 4] = [1000.0, 2000.0, 3000.0, 5000.0];
const UNAUTHORIZED_RETRY_MS: f64 = 15000.0;
const SETTINGS_CHANGED_RETRY_MS: f64 = 500.0;
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
/// Feishin does not answer when the song has no cover (any more). Ask again after this.
const COVER_TIMEOUT: Duration = Duration::from_secs(15);

// Close codes of the remote control server of Feishin (src/main/features/core/remote/index.ts).
const CLOSE_SERVER_SHUTDOWN: u16 = 4000;
const CLOSE_SETTINGS_CHANGED: u16 = 4002;
const CLOSE_AUTH_FAILED: u16 = 4004;
/// The socket ended without a close frame.
const CLOSE_ABNORMAL: u16 = 1006;

/// How the connection to Feishin is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
	/// Trying to connect.
	Connecting,
	/// Logged in, the player state comes in.
	Connected,
	/// Nothing answers at host:port (Feishin closed, remote control off, wrong port).
	Unreachable,
	/// Feishin does not take the username or password.
	Unauthorized,
	/// Anything else, like another program on the port. See the detail.
	Error,
}

impl Status {
	/// The name the settings page knows.
	pub fn name(self) -> &'static str {
		match self {
			Status::Connecting => "connecting",
			Status::Connected => "connected",
			Status::Unreachable => "unreachable",
			Status::Unauthorized => "unauthorized",
			Status::Error => "error",
		}
	}
}

#[derive(Clone, Debug, PartialEq)]
pub struct Connection {
	pub status: Status,
	/// host:port the plugin tries to reach.
	pub address: String,
	/// Why, for Status::Error.
	pub detail: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Playback {
	Playing,
	Paused,
	#[default]
	Stopped,
}

impl Playback {
	/// Older Feishin versions sent upper case, or a boolean.
	fn parse(value: &Value) -> Option<Playback> {
		if let Some(playing) = value.as_bool() {
			return Some(if playing { Playback::Playing } else { Playback::Paused });
		}
		match value.as_str()?.to_lowercase().as_str() {
			"playing" => Some(Playback::Playing),
			"paused" => Some(Playback::Paused),
			"stopped" => Some(Playback::Stopped),
			_ => None,
		}
	}
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Repeat {
	#[default]
	None,
	All,
	One,
}

impl Repeat {
	fn parse(value: &Value) -> Option<Repeat> {
		match value.as_str()?.to_lowercase().as_str() {
			"none" => Some(Repeat::None),
			"all" => Some(Repeat::All),
			"one" => Some(Repeat::One),
			_ => None,
		}
	}

	/// The name in Feishin, also the name of its picture.
	pub fn name(self) -> &'static str {
		match self {
			Repeat::None => "none",
			Repeat::All => "all",
			Repeat::One => "one",
		}
	}
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Song {
	pub name: String,
	pub artist: String,
	/// In seconds.
	pub duration: f64,
	/// Where Feishin loads the cover from, "" for none. The songs of an album share it.
	pub cover: String,
}

impl Song {
	fn parse(value: &Value) -> Option<Song> {
		if !value.is_object() {
			return None;
		}
		Some(Song {
			name: value["name"].as_str().unwrap_or_default().to_string(),
			artist: value["artistName"].as_str().unwrap_or_default().to_string(),
			// Feishin sends milliseconds.
			duration: value["duration"].as_f64().map_or(0.0, |ms| ms / 1000.0),
			cover: value["imageUrl"].as_str().unwrap_or_default().to_string(),
		})
	}
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerState {
	pub playback: Playback,
	pub repeat: Repeat,
	pub shuffle: bool,
	/// 0 to 100. None while not known: Feishin only says its volume once it changed.
	pub volume: Option<f64>,
	pub song: Option<Song>,
}

/// What changed in the player state. Position: a seek, or time passing. Cover: a cover came in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
	Playback,
	Repeat,
	Shuffle,
	Volume,
	Song,
	Position,
	Cover,
}

/// Where and how to log in.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
	pub host: String,
	pub port: String,
	pub username: String,
	pub password: String,
}

impl Settings {
	/// Empty values take the defaults.
	pub fn new(host: &str, port: &str, username: &str, password: &str) -> Settings {
		let or = |value: &str, fallback: &str| if value.is_empty() { fallback.to_string() } else { value.to_string() };
		Settings {
			host: or(crate::js::trim(host), DEFAULT_HOST),
			port: or(crate::js::trim(port), DEFAULT_PORT),
			username: or(username, DEFAULT_USERNAME),
			password: or(password, DEFAULT_PASSWORD),
		}
	}

	fn address(&self) -> String {
		format!("{}:{}", self.host, self.port)
	}

	fn url(&self) -> String {
		// IPv6 addresses go in brackets.
		let host = if self.host.contains(':') && !self.host.starts_with('[') { format!("[{}]", self.host) } else { self.host.clone() };
		format!("ws://{host}:{}", self.port)
	}
}

impl Default for Settings {
	fn default() -> Settings {
		Settings::new("", "", "", "")
	}
}

struct Socket {
	serial: u64,
	tx: mpsc::UnboundedSender<Message>,
	task: AbortHandle,
}

/// Why the last try failed, before the socket closed.
#[derive(Clone, Debug)]
enum Failure {
	/// Nobody there: refused, no route, no such host, timed out.
	Unreachable,
	Other(String),
}

pub struct Client {
	settings: RefCell<Settings>,
	socket: RefCell<Option<Socket>>,
	serial: Cell<u64>,
	retry_timer: RefCell<Option<Timer>>,
	attempt: Cell<usize>,
	started: Cell<bool>,
	failure: RefCell<Option<Failure>>,
	connection: RefCell<Connection>,
	state: RefCell<PlayerState>,
	position: Cell<f64>,
	position_at: Cell<Instant>,
	seek_settles: Cell<Instant>,
	volume_settles: Cell<Instant>,
	volume_before_mute: Cell<Option<f64>>,
	/// The cover last loaded, by where Feishin loads it from. None in it: it could not be loaded.
	cover: RefCell<Option<(String, Option<String>)>>,
	/// Feishin answers a cover request without saying which cover it is, so only one is asked for
	/// at a time: where from, and when.
	cover_request: RefCell<Option<(String, Instant)>>,
	me: Weak<Client>,
	pub on_connection: Emitter<Connection>,
	pub on_state: Emitter<[Change]>,
	/// For tests: the messages that would go to Feishin.
	#[cfg(test)]
	pub sent: RefCell<Option<Vec<Value>>>,
}

impl Client {
	pub fn new() -> Rc<Client> {
		let settings = Settings::default();
		let address = settings.address();
		let now = Instant::now();
		Rc::new_cyclic(|me| Client {
			settings: RefCell::new(settings),
			socket: RefCell::new(None),
			serial: Cell::new(0),
			retry_timer: RefCell::new(None),
			attempt: Cell::new(0),
			started: Cell::new(false),
			failure: RefCell::new(None),
			connection: RefCell::new(Connection { status: Status::Connecting, address, detail: String::new() }),
			state: RefCell::new(PlayerState::default()),
			position: Cell::new(0.0),
			position_at: Cell::new(now),
			seek_settles: Cell::new(now),
			volume_settles: Cell::new(now),
			volume_before_mute: Cell::new(None),
			cover: RefCell::new(None),
			cover_request: RefCell::new(None),
			me: me.clone(),
			on_connection: Emitter::default(),
			on_state: Emitter::default(),
			#[cfg(test)]
			sent: RefCell::new(None),
		})
	}

	pub fn connection(&self) -> Connection {
		self.connection.borrow().clone()
	}

	pub fn connected(&self) -> bool {
		self.connection.borrow().status == Status::Connected
	}

	pub fn state(&self) -> PlayerState {
		self.state.borrow().clone()
	}

	/// A volume seen before, while Feishin did not say its own yet. Feishin starts with the volume it
	/// had, so this is right most of the time.
	pub fn assume_volume(&self, volume: f64) {
		if self.state.borrow().volume.is_none() && volume.is_finite() {
			self.update(|s| s.volume = Some(volume.clamp(0.0, 100.0)), false);
		}
	}

	/// Where the track is, in seconds. Counts on by itself while playing.
	pub fn position(&self) -> f64 {
		let state = self.state.borrow();
		let elapsed = if state.playback == Playback::Playing { self.position_at.get().elapsed().as_secs_f64() } else { 0.0 };
		let position = self.position.get() + elapsed;
		match &state.song {
			Some(song) => position.min(song.duration),
			None => position,
		}
	}

	/// The cover of the current song as a small data URL for the keys. None while there is none or
	/// it is still loading. Asks Feishin for it the first time: Feishin loads it from the music server
	/// ("proxy").
	pub fn cover(&self) -> Option<String> {
		let url = self.state.borrow().song.as_ref().map(|s| s.cover.clone()).filter(|url| !url.is_empty())?;
		if let Some((loaded, image)) = self.cover.borrow().as_ref()
			&& *loaded == url
		{
			return image.clone();
		}
		let waiting = self.cover_request.borrow().as_ref().is_some_and(|(_, at)| at.elapsed() <= COVER_TIMEOUT);
		if !waiting && self.send(json!({ "event": "proxy" })) {
			*self.cover_request.borrow_mut() = Some((url, Instant::now()));
		}
		None
	}

	/// New connection settings. Connects again when something changed.
	pub fn configure(&self, settings: Settings) {
		let changed = *self.settings.borrow() != settings;
		*self.settings.borrow_mut() = settings;
		if changed || !self.started.get() {
			self.started.set(true);
			self.reconnect();
		}
	}

	/// Drops the connection (if any) and connects again right away.
	pub fn reconnect(&self) {
		// Clients of tests stay as they are.
		#[cfg(test)]
		if self.sent.borrow().is_some() {
			return;
		}
		self.attempt.set(0);
		self.teardown();
		self.connect(true);
	}

	/// Connects right away while waiting for the next try. For when a key is used.
	pub fn wake(&self) {
		if self.started.get() && self.socket.borrow().is_none() {
			self.reconnect();
		}
	}

	// ----- commands. false: not connected, nothing was sent. -----

	pub fn toggle_play_pause(&self) -> bool {
		let playing = self.state.borrow().playback == Playback::Playing;
		if !self.send(json!({ "event": if playing { "pause" } else { "play" } })) {
			return false;
		}
		// Feishin answers with a playback event. Show it right away, so the key reacts at once. Count
		// from here, so the time does not jump ahead by the time spent paused.
		self.set_position(self.position());
		self.update(|s| s.playback = if playing { Playback::Paused } else { Playback::Playing }, false);
		true
	}

	pub fn next(&self) -> bool {
		self.send(json!({ "event": "next" }))
	}

	pub fn previous(&self) -> bool {
		self.send(json!({ "event": "previous" }))
	}

	pub fn toggle_shuffle(&self) -> bool {
		self.send(json!({ "event": "shuffle" }))
	}

	pub fn toggle_repeat(&self) -> bool {
		self.send(json!({ "event": "repeat" }))
	}

	/// 0 to 100.
	pub fn set_volume(&self, volume: f64) -> bool {
		let value = crate::js::round(volume.clamp(0.0, 100.0));
		if !self.send(json!({ "event": "volume", "volume": value as i64 })) {
			return false;
		}
		self.volume_settles.set(Instant::now() + SETTLE);
		self.update(|s| s.volume = Some(value), false);
		true
	}

	/// Up or down from the volume now. Fails while the volume is not known, rather than guess and
	/// suddenly get much louder or quieter.
	pub fn change_volume(&self, delta: f64) -> bool {
		let volume = self.state.borrow().volume;
		volume.is_some_and(|volume| self.set_volume(volume + delta))
	}

	/// The remote control of Feishin cannot mute: muting sets the volume to 0, unmuting sets the volume
	/// from before.
	pub fn toggle_mute(&self) -> bool {
		let volume = self.state.borrow().volume;
		if volume != Some(0.0) {
			if !self.set_volume(0.0) {
				return false;
			}
			self.volume_before_mute.set(volume);
			return true;
		}
		self.set_volume(self.volume_before_mute.get().unwrap_or(50.0))
	}

	/// Forward (positive) or back (negative), in seconds.
	pub fn seek_by(&self, seconds: f64) -> bool {
		let Some(duration) = self.state.borrow().song.as_ref().map(|s| s.duration) else { return false };
		// Stop just before the end, so seeking forward never skips to the next track by accident.
		let position = (self.position() + seconds).clamp(0.0, (duration - 1.0).max(0.0));
		if !self.send(json!({ "event": "position", "position": position })) {
			return false;
		}
		self.set_position(position);
		self.seek_settles.set(Instant::now() + SETTLE);
		self.on_state.emit(&[Change::Position]);
		true
	}

	// ----- the connection -----

	/// announce: show "connecting". Tries in the background keep showing the last failure, so the
	/// status does not flicker every few seconds while Feishin is closed.
	fn connect(&self, announce: bool) {
		self.clear_retry();
		*self.failure.borrow_mut() = None;
		*self.cover_request.borrow_mut() = None;
		let settings = self.settings.borrow().clone();
		if announce || self.connected() {
			self.set_connection(Status::Connecting, "");
		}
		let request = match settings.url().into_client_request() {
			Ok(request) => request,
			Err(error) => {
				// A host or port that cannot be in a URL, like "local host" or port "abc".
				self.set_connection(Status::Error, &error.to_string());
				self.schedule_retry(RECONNECT_DELAYS_MS[RECONNECT_DELAYS_MS.len() - 1]);
				return;
			}
		};
		let Some(me) = self.me.upgrade() else { return };
		let serial = self.serial.get() + 1;
		self.serial.set(serial);
		let (tx, rx) = mpsc::unbounded_channel();
		// Feishin skips the login when it has no username and password. It ignores this then.
		let login = base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", settings.username, settings.password));
		let _ = tx.send(Message::Text(json!({ "event": "authenticate", "header": format!("Basic {login}") }).to_string().into()));
		let task = spawn_local(run_socket(me, serial, request, rx));
		*self.socket.borrow_mut() = Some(Socket { serial, tx, task: task.abort_handle() });
	}

	fn is_current(&self, serial: u64) -> bool {
		self.socket.borrow().as_ref().is_some_and(|s| s.serial == serial)
	}

	fn closed(&self, serial: u64, code: u16) {
		if !self.is_current(serial) {
			return;
		}
		*self.socket.borrow_mut() = None;
		if code == CLOSE_AUTH_FAILED {
			self.set_connection(Status::Unauthorized, "");
			self.schedule_retry(UNAUTHORIZED_RETRY_MS);
			return;
		}
		if code == CLOSE_SETTINGS_CHANGED {
			// The remote settings changed in Feishin (like a new password): try again, fresh.
			self.attempt.set(0);
			self.set_connection(Status::Connecting, "");
			self.schedule_retry(SETTINGS_CHANGED_RETRY_MS);
			return;
		}
		let failure = self.failure.borrow().clone();
		if code == CLOSE_SERVER_SHUTDOWN || !self.connected() || matches!(failure, Some(Failure::Unreachable)) {
			match failure {
				Some(Failure::Other(detail)) if code != CLOSE_SERVER_SHUTDOWN => self.set_connection(Status::Error, &detail),
				_ => self.set_connection(Status::Unreachable, ""),
			}
		} else {
			// The connection dropped. Connect again without a fuss.
			self.set_connection(Status::Connecting, "");
		}
		let attempt = self.attempt.get();
		self.attempt.set(attempt + 1);
		self.schedule_retry(RECONNECT_DELAYS_MS[attempt.min(RECONNECT_DELAYS_MS.len() - 1)]);
	}

	fn schedule_retry(&self, delay: f64) {
		self.clear_retry();
		let me = self.me.clone();
		let timer = set_timeout(delay, move || {
			if let Some(me) = me.upgrade() {
				me.connect(false);
			}
		});
		*self.retry_timer.borrow_mut() = Some(timer);
	}

	fn clear_retry(&self) {
		if let Some(timer) = self.retry_timer.borrow_mut().take() {
			timer.clear();
		}
	}

	fn teardown(&self) {
		self.clear_retry();
		if let Some(socket) = self.socket.borrow_mut().take() {
			socket.task.abort();
		}
	}

	fn send(&self, message: Value) -> bool {
		#[cfg(test)]
		if let Some(sent) = self.sent.borrow_mut().as_mut() {
			sent.push(message);
			return true;
		}
		if !self.connected() {
			return false;
		}
		match self.socket.borrow().as_ref() {
			Some(socket) => socket.tx.send(Message::Text(message.to_string().into())).is_ok(),
			None => false,
		}
	}

	fn set_connection(&self, status: Status, detail: &str) {
		let next = Connection { status, address: self.settings.borrow().address(), detail: detail.to_string() };
		if *self.connection.borrow() != next {
			*self.connection.borrow_mut() = next.clone();
			self.on_connection.emit(&next);
		}
	}

	// ----- the player state -----

	/// A message from Feishin, like {"event": "volume", "data": 40}.
	pub fn on_message(&self, message: &Value) {
		let data = &message["data"];
		match message["event"].as_str().unwrap_or_default() {
			// Comes once logged in. The state goes in before the connection says "connected", so keys
			// never draw the state from before a reconnect as connected.
			"state" => {
				self.attempt.set(0);
				if let Some(position) = data["position"].as_f64() {
					self.set_position(position);
				}
				self.update(
					|s| {
						s.playback = Playback::parse(&data["status"]).unwrap_or(s.playback);
						s.repeat = Repeat::parse(&data["repeat"]).unwrap_or(s.repeat);
						s.shuffle = data["shuffle"].as_bool().unwrap_or(s.shuffle);
						// Missing until the volume changed since Feishin started: keep what we know.
						s.volume = data["volume"].as_f64().or(s.volume);
						s.song = Song::parse(&data["song"]);
					},
					true,
				);
				self.set_connection(Status::Connected, "");
			}
			"playback" => {
				if let Some(playback) = Playback::parse(data) {
					// Start counting from here, so pausing keeps the time played since the last update.
					self.set_position(self.position());
					self.update(|s| s.playback = playback, false);
				}
			}
			"song" => {
				self.set_position(0.0);
				self.update(|s| s.song = Song::parse(data), true);
			}
			"position" => {
				if let Some(position) = data.as_f64()
					&& Instant::now() >= self.seek_settles.get()
				{
					self.set_position(position);
					self.on_state.emit(&[Change::Position]);
				}
			}
			"volume" => {
				if let Some(volume) = data.as_f64()
					&& Instant::now() >= self.volume_settles.get()
				{
					self.update(|s| s.volume = Some(volume), false);
				}
			}
			"shuffle" => {
				if let Some(shuffle) = data.as_bool() {
					self.update(|s| s.shuffle = shuffle, false);
				}
			}
			"repeat" => {
				if let Some(repeat) = Repeat::parse(data) {
					self.update(|s| s.repeat = repeat, false);
				}
			}
			"proxy" => self.cover_loaded(data.as_str().and_then(shrink_cover)),
			// Feishin could not load the cover. The plugin sends nothing else that Feishin answers
			// with an error.
			"error" => self.cover_loaded(None),
			_ => {}
		}
	}

	fn cover_loaded(&self, image: Option<String>) {
		let Some((url, _)) = self.cover_request.borrow_mut().take() else { return };
		*self.cover.borrow_mut() = Some((url, image));
		// Also when the song changed meanwhile: the keys then ask for the new cover.
		self.on_state.emit(&[Change::Cover]);
	}

	/// Changes the state and tells the keys what changed. position: tell them about the position too.
	fn update(&self, change: impl FnOnce(&mut PlayerState), position: bool) {
		let before = self.state();
		change(&mut self.state.borrow_mut());
		let after = self.state();
		let mut changes = Vec::new();
		if position {
			changes.push(Change::Position);
		}
		for (changed, what) in [
			(before.playback != after.playback, Change::Playback),
			(before.repeat != after.repeat, Change::Repeat),
			(before.shuffle != after.shuffle, Change::Shuffle),
			(before.volume != after.volume, Change::Volume),
			(before.song != after.song, Change::Song),
		] {
			if changed {
				changes.push(what);
			}
		}
		if !changes.is_empty() {
			self.on_state.emit(&changes);
		}
	}

	fn set_position(&self, position: f64) {
		self.position.set(position);
		self.position_at.set(Instant::now());
	}
}

/// Why connecting failed: is anyone there at all?
fn failure(error: &tokio_tungstenite::tungstenite::Error) -> Failure {
	use std::io::ErrorKind;
	use tokio_tungstenite::tungstenite::Error as E;
	match error {
		E::Io(e)
			if matches!(
				e.kind(),
				ErrorKind::ConnectionRefused
					| ErrorKind::ConnectionReset
					| ErrorKind::HostUnreachable
					| ErrorKind::NetworkUnreachable
					| ErrorKind::NotFound
					| ErrorKind::TimedOut
			) || e.to_string().contains("failed to lookup address") =>
		{
			Failure::Unreachable
		}
		other => Failure::Other(other.to_string()),
	}
}

async fn run_socket(
	me: Rc<Client>,
	serial: u64,
	request: tokio_tungstenite::tungstenite::handshake::client::Request,
	mut rx: mpsc::UnboundedReceiver<Message>,
) {
	let stream = match tokio::time::timeout(HANDSHAKE_TIMEOUT, tokio_tungstenite::connect_async(request)).await {
		Ok(Ok((stream, _))) => stream,
		Ok(Err(error)) => {
			*me.failure.borrow_mut() = Some(failure(&error));
			me.closed(serial, CLOSE_ABNORMAL);
			return;
		}
		Err(_) => {
			*me.failure.borrow_mut() = Some(Failure::Unreachable);
			me.closed(serial, CLOSE_ABNORMAL);
			return;
		}
	};
	if !me.is_current(serial) {
		return;
	}
	let (mut sink, mut stream) = stream.split();
	let writer = spawn_local(async move {
		while let Some(message) = rx.recv().await {
			if sink.send(message).await.is_err() {
				break;
			}
		}
	});
	let mut code = CLOSE_ABNORMAL;
	while let Some(message) = stream.next().await {
		if !me.is_current(serial) {
			writer.abort();
			return;
		}
		match message {
			Ok(Message::Text(text)) => {
				// Anything that is not JSON is ignored.
				if let Ok(message) = serde_json::from_str::<Value>(&text) {
					me.on_message(&message);
				}
			}
			Ok(Message::Close(frame)) => {
				code = frame.map_or(CLOSE_ABNORMAL, |f| f.code.into());
				break;
			}
			Ok(_) => {}
			Err(error) => {
				*me.failure.borrow_mut() = Some(failure(&error));
				break;
			}
		}
	}
	writer.abort();
	me.closed(serial, code);
}

#[cfg(test)]
pub(crate) mod testing {
	use super::*;

	/// A client that is connected and only writes down what it sends.
	pub fn connected(state: Value) -> Rc<Client> {
		let client = Client::new();
		*client.sent.borrow_mut() = Some(Vec::new());
		client.on_message(&json!({ "event": "state", "data": state }));
		client
	}

	pub fn sent(client: &Client) -> Vec<Value> {
		client.sent.borrow().clone().unwrap_or_default()
	}
}

#[cfg(test)]
mod tests {
	use super::testing::*;
	use super::*;

	fn song(duration_ms: f64) -> Value {
		json!({ "name": "Song", "artistName": "Artist", "duration": duration_ms })
	}

	#[test]
	fn the_first_state_connects_and_fills_in_the_player() {
		let client = connected(json!({ "status": "PLAYING", "repeat": "all", "shuffle": true, "volume": 40, "song": song(180000.0) }));
		assert_eq!(client.connection().status, Status::Connected);
		let state = client.state();
		assert_eq!((state.playback, state.repeat, state.shuffle, state.volume), (Playback::Playing, Repeat::All, true, Some(40.0)));
		assert_eq!(state.song, Some(Song { name: "Song".into(), artist: "Artist".into(), duration: 180.0, cover: String::new() }));
		// Old Feishin versions send a boolean.
		client.on_message(&json!({ "event": "playback", "data": false }));
		assert_eq!(client.state().playback, Playback::Paused);
	}

	#[test]
	fn the_volume_stays_unknown_until_feishin_says_it() {
		let client = connected(json!({ "status": "paused" }));
		assert_eq!(client.state().volume, None);
		assert!(!client.change_volume(5.0), "no guessing");
		client.assume_volume(30.0);
		assert!(client.change_volume(5.0));
		assert_eq!(sent(&client), [json!({ "event": "volume", "volume": 35 })]);
	}

	#[tokio::test(start_paused = true)]
	async fn an_old_volume_from_feishin_does_not_jump_back_right_after_a_change() {
		let client = connected(json!({ "volume": 50 }));
		client.set_volume(60.0);
		client.on_message(&json!({ "event": "volume", "data": 50 }));
		assert_eq!(client.state().volume, Some(60.0));
		tokio::time::advance(SETTLE).await;
		client.on_message(&json!({ "event": "volume", "data": 55 }));
		assert_eq!(client.state().volume, Some(55.0));
	}

	#[test]
	fn mute_sets_0_and_unmute_the_volume_from_before() {
		let client = connected(json!({ "volume": 70 }));
		client.toggle_mute();
		assert_eq!(client.state().volume, Some(0.0));
		client.toggle_mute();
		assert_eq!(client.state().volume, Some(70.0));
		let volumes: Vec<Value> = sent(&client).iter().map(|m| m["volume"].clone()).collect();
		assert_eq!(volumes, [json!(0), json!(70)]);
	}

	#[tokio::test(start_paused = true)]
	async fn seeking_stays_in_the_track_and_the_position_counts_on_while_playing() {
		let client = connected(json!({ "status": "playing", "position": 10, "song": song(60000.0) }));
		tokio::time::advance(Duration::from_secs(2)).await;
		assert_eq!(client.position(), 12.0);
		client.seek_by(100.0);
		assert_eq!(sent(&client).last(), Some(&json!({ "event": "position", "position": 59.0 })));
		client.seek_by(-100.0);
		assert_eq!(client.position(), 0.0);
		// Nothing to seek in without a song.
		client.on_message(&json!({ "event": "song", "data": null }));
		assert!(!client.seek_by(5.0));
	}

	#[tokio::test(start_paused = true)]
	async fn the_time_goes_on_from_where_it_paused() {
		let client = connected(json!({ "status": "paused", "position": 30, "song": song(60000.0) }));
		tokio::time::advance(Duration::from_secs(20)).await;
		client.toggle_play_pause();
		assert_eq!(client.position(), 30.0);
		tokio::time::advance(Duration::from_secs(2)).await;
		client.toggle_play_pause();
		tokio::time::advance(Duration::from_secs(5)).await;
		assert_eq!(client.position(), 32.0);
	}

	#[test]
	fn empty_settings_take_the_defaults() {
		assert_eq!(Settings::new(" ", "", "", ""), Settings::default());
		assert_eq!(Settings::default().url(), "ws://localhost:4333");
		assert_eq!(Settings::new("::1", "4334", "u", "p").url(), "ws://[::1]:4334");
	}

	#[tokio::test(start_paused = true)]
	async fn covers_are_asked_for_one_at_a_time_and_kept() {
		let first = json!({ "name": "First", "duration": 1000, "imageUrl": "http://music/cover/1" });
		let client = connected(json!({ "song": first }));
		let seen = Rc::new(Cell::new(0));
		let into = seen.clone();
		client.on_state.on(move |changes| into.set(into.get() + usize::from(changes.contains(&Change::Cover))));
		assert_eq!(client.cover(), None);
		assert_eq!(client.cover(), None);
		assert_eq!(sent(&client), [json!({ "event": "proxy" })], "one request at a time");
		client.on_message(&json!({ "event": "proxy", "data": crate::cover::testing::png(4, 4) }));
		assert_eq!(seen.get(), 1);
		let cover = client.cover().unwrap_or_default();
		assert!(cover.starts_with("data:image/jpeg;base64,"), "{cover}");
		// Another song of the album: the same cover, not asked for again.
		client.on_message(&json!({ "event": "song", "data": { "name": "Second", "imageUrl": "http://music/cover/1" } }));
		assert_eq!(client.cover(), Some(cover));
		assert_eq!(sent(&client).len(), 1);
		// Feishin could not load it.
		client.on_message(&json!({ "event": "song", "data": { "name": "Third", "imageUrl": "http://music/cover/3" } }));
		assert_eq!(client.cover(), None);
		client.on_message(&json!({ "event": "error", "data": "404" }));
		assert_eq!((client.cover(), sent(&client).len()), (None, 2));
		// No answer: asked again later.
		client.on_message(&json!({ "event": "song", "data": { "name": "Fourth", "imageUrl": "http://music/cover/4" } }));
		client.cover();
		tokio::time::advance(COVER_TIMEOUT).await;
		client.cover();
		assert_eq!(sent(&client).len(), 3);
		tokio::time::advance(Duration::from_millis(1)).await;
		client.cover();
		assert_eq!(sent(&client).len(), 4);
		// No cover at all: nothing to ask for.
		client.on_message(&json!({ "event": "song", "data": { "name": "Fifth" } }));
		assert_eq!(client.cover(), None);
	}

	#[test]
	fn changes_say_what_changed() {
		let client = connected(json!({ "status": "paused", "shuffle": false }));
		let seen = Rc::new(RefCell::new(Vec::new()));
		let into = seen.clone();
		client.on_state.on(move |changes| into.borrow_mut().push(changes.to_vec()));
		client.on_message(&json!({ "event": "shuffle", "data": true }));
		client.on_message(&json!({ "event": "shuffle", "data": true }));
		client.on_message(&json!({ "event": "repeat", "data": "ONE" }));
		assert_eq!(*seen.borrow(), [vec![Change::Shuffle], vec![Change::Repeat]]);
	}
}
