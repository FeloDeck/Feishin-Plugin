// A fake Feishin: its remote control server, with a player that only keeps its state. For the
// end-to-end tests, and to try the keys without Feishin (cargo xtask fake-feishin).
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use image::{ImageFormat, Rgba, RgbaImage};
use serde_json::{Value, json};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tokio::task::{AbortHandle, spawn_local};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;

use crate::Result;
use crate::deck::local;

pub const PORT: u16 = 4333;
const USERNAME: &str = "feishin";
pub const PASSWORD: &str = "streamdeck";
// Close codes of Feishin.
const SERVER_SHUTDOWN: u16 = 4000;
const SETTINGS_CHANGED: u16 = 4002;
const AUTH_FAILED: u16 = 4004;

/// The third song has no cover.
pub fn songs() -> [Value; 3] {
	[
		json!({ "id": "1", "name": "First Song", "artistName": "Test Artist", "duration": 200_000, "imageUrl": "http://fake/cover/1", "userFavorite": false }),
		json!({ "id": "2", "name": "Second Song", "artistName": "Other Artist", "duration": 3_725_000, "imageUrl": "http://fake/cover/2", "userFavorite": true }),
		json!({ "id": "3", "name": "Third Song", "artistName": "Test Artist", "duration": 95_000, "userFavorite": false }),
	]
}

/// A cover as Feishin sends it (base64): 600 x 600 like on a music server, warm for the first song,
/// cool for the others, with a ring in the middle.
fn cover(url: &str) -> String {
	let [r, g, b] = if url.ends_with('1') { [235.0, 95.0, 45.0] } else { [40.0, 150.0, 225.0] };
	let image = RgbaImage::from_fn(600, 600, |x, y| {
		let (x, y) = (f64::from(x), f64::from(y));
		if ((x - 300.0).hypot(y - 300.0) - 200.0).abs() < 14.0 {
			return Rgba([255, 255, 255, 255]);
		}
		let light = 0.35 + 0.65 * (1200.0 - x - y) / 1200.0;
		Rgba([(r * light) as u8, (g * light) as u8, (b * light) as u8, 255])
	});
	let mut png = std::io::Cursor::new(Vec::new());
	let _ = image.write_to(&mut png, ImageFormat::Png);
	base64::engine::general_purpose::STANDARD.encode(png.into_inner())
}

pub struct Fake {
	/// Like the state message of Feishin: status, repeat, shuffle, volume, song, position.
	player: RefCell<Value>,
	/// The commands the plugin sent, oldest first. Without the requests for covers.
	got: RefCell<Vec<Value>>,
	/// How many covers the plugin asked for.
	covers: Cell<usize>,
	password: RefCell<String>,
	clients: RefCell<Vec<UnboundedSender<Message>>>,
	server: RefCell<Option<AbortHandle>>,
	/// Prints the commands.
	verbose: Cell<bool>,
}

fn close(code: u16) -> Message {
	Message::Close(Some(CloseFrame { code: code.into(), reason: "".into() }))
}

impl Fake {
	pub fn new() -> Rc<Fake> {
		Rc::new(Fake {
			player: RefCell::new(Fake::start_state()),
			got: RefCell::default(),
			covers: Cell::new(0),
			password: RefCell::new(PASSWORD.into()),
			clients: RefCell::default(),
			server: RefCell::default(),
			verbose: Cell::new(false),
		})
	}

	/// Paused, so the time on the keys stays put.
	pub fn start_state() -> Value {
		json!({ "status": "paused", "repeat": "none", "shuffle": false, "volume": 50, "song": songs()[0], "position": 30 })
	}

	pub async fn start(self: &Rc<Self>) -> Result {
		let listener = TcpListener::bind(("127.0.0.1", PORT)).await.map_err(|e| format!("port {PORT}: {e}"))?;
		let me = self.clone();
		let task = spawn_local(async move {
			while let Ok((stream, _)) = listener.accept().await {
				spawn_local(serve(me.clone(), stream));
			}
		});
		*self.server.borrow_mut() = Some(task.abort_handle());
		Ok(())
	}

	/// Like quitting Feishin.
	pub fn stop(&self) {
		if let Some(server) = self.server.borrow_mut().take() {
			server.abort();
		}
		self.close_all(SERVER_SHUTDOWN);
	}

	/// Like a new password in the remote settings of Feishin: it closes the connections.
	pub fn set_password(&self, password: &str) {
		*self.password.borrow_mut() = password.into();
		self.close_all(SETTINGS_CHANGED);
	}

	fn close_all(&self, code: u16) {
		for client in self.clients.borrow_mut().drain(..) {
			let _ = client.send(close(code));
		}
	}

	pub fn clients(&self) -> usize {
		self.clients.borrow().len()
	}

	pub fn covers(&self) -> usize {
		self.covers.get()
	}

	pub fn player(&self) -> Value {
		self.player.borrow().clone()
	}

	/// The player as at the start, and tells the plugin.
	pub fn reset(&self) {
		for (key, value) in Fake::start_state().as_object().cloned().unwrap_or_default() {
			let event = if key == "status" { "playback" } else { key.as_str() };
			self.set(event, value);
		}
	}

	/// The commands since the last call.
	pub fn take(&self) -> Vec<Value> {
		std::mem::take(&mut self.got.borrow_mut())
	}

	/// Changes the player like Feishin does, and tells the plugin.
	pub fn set(&self, event: &str, data: Value) {
		let key = if event == "playback" { "status" } else { event };
		self.player.borrow_mut()[key] = data.clone();
		if event == "song" {
			self.player.borrow_mut()["position"] = 0.into();
		}
		self.send(json!({ "event": event, "data": data }));
	}

	/// Marks the song as a favorite or not, like in Feishin, and tells the plugin.
	pub fn set_favorite(&self, favorite: bool) {
		let id = self.player()["song"]["id"].clone();
		self.player.borrow_mut()["song"]["userFavorite"] = favorite.into();
		self.send(json!({ "event": "favorite", "data": { "favorite": favorite, "id": id } }));
	}

	fn send(&self, message: Value) {
		let text = message.to_string();
		self.clients.borrow_mut().retain(|c| c.send(Message::Text(text.clone().into())).is_ok());
	}

	fn command(&self, command: Value) {
		if self.verbose.get() {
			println!("{command}");
		}
		let player = self.player();
		match command["event"].as_str().unwrap_or_default() {
			"play" => self.set("playback", "playing".into()),
			"pause" => self.set("playback", "paused".into()),
			event @ ("next" | "previous") => {
				let songs = songs();
				let now = songs.iter().position(|s| s["name"] == player["song"]["name"]).unwrap_or(0);
				let next = if event == "next" { now + 1 } else { now + songs.len() - 1 } % songs.len();
				self.set("song", songs[next].clone());
			}
			"shuffle" => self.set("shuffle", (!player["shuffle"].as_bool().unwrap_or(false)).into()),
			"repeat" => {
				let next = match player["repeat"].as_str() {
					Some("none") => "all",
					Some("all") => "one",
					_ => "none",
				};
				self.set("repeat", next.into());
			}
			// Only for the song that plays, like in Feishin.
			"favorite" => {
				if command["id"] == player["song"]["id"]
					&& let Some(favorite) = command["favorite"].as_bool()
				{
					self.set_favorite(favorite);
				}
			}
			"volume" => self.set("volume", command["volume"].clone()),
			"position" => self.set("position", command["position"].clone()),
			// Feishin loads the cover of the song and sends it back, or nothing without a cover. To
			// every client here, it only goes to the one that asked.
			"proxy" => {
				self.covers.set(self.covers.get() + 1);
				if let Some(url) = player["song"]["imageUrl"].as_str() {
					self.send(json!({ "event": "proxy", "data": cover(url) }));
				}
				return;
			}
			_ => {}
		}
		self.got.borrow_mut().push(command);
	}
}

async fn serve(fake: Rc<Fake>, stream: TcpStream) {
	let Ok(socket) = tokio_tungstenite::accept_async(stream).await else { return };
	let (mut sink, mut stream) = socket.split();
	let (tx, mut rx) = unbounded_channel::<Message>();
	spawn_local(async move {
		while let Some(message) = rx.recv().await {
			let last = matches!(message, Message::Close(_));
			if sink.send(message).await.is_err() || last {
				break;
			}
		}
	});
	// The first message logs in.
	let login = loop {
		match stream.next().await {
			Some(Ok(Message::Text(text))) => break serde_json::from_str::<Value>(&text).unwrap_or_default(),
			Some(Ok(_)) => {}
			_ => return,
		}
	};
	let header = format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(format!("{USERNAME}:{}", fake.password.borrow())));
	if login["event"] != "authenticate" || login["header"] != header.as_str() {
		let _ = tx.send(close(AUTH_FAILED));
		return;
	}
	let _ = tx.send(Message::Text(json!({ "event": "state", "data": fake.player() }).to_string().into()));
	fake.clients.borrow_mut().push(tx.clone());
	while let Some(Ok(message)) = stream.next().await {
		if let Message::Text(text) = message
			&& let Ok(command) = serde_json::from_str::<Value>(&text)
		{
			fake.command(command);
		}
	}
	fake.clients.borrow_mut().retain(|c| !c.same_channel(&tx));
}

/// cargo xtask fake-feishin
pub fn run() -> Result {
	local(async {
		let fake = Fake::new();
		fake.verbose.set(true);
		fake.start().await?;
		println!("Fake Feishin on ws://127.0.0.1:{PORT} (username {USERNAME}, password {PASSWORD}). Ctrl+C stops it.");
		std::future::pending::<()>().await;
		Ok(())
	})
}
