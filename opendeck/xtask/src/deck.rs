// The virtual deck through its HTTP API, and what the end-to-end tests need around it.
use std::future::Future;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::Result;

/// Runs async code on one thread, like the plugin.
pub fn local<F: Future>(future: F) -> F::Output {
	let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("tokio runtime");
	tokio::task::LocalSet::new().block_on(&runtime, future)
}

pub async fn sleep(ms: u64) {
	tokio::time::sleep(Duration::from_millis(ms)).await;
}

pub struct Deck {
	url: String,
}

impl Deck {
	pub fn new() -> Deck {
		Deck { url: std::env::var("VIRTUAL_DECK_URL").unwrap_or_else(|_| "http://127.0.0.1:57190".into()) }
	}

	/// On a thread of its own, so a long press can go on while the test looks at Feishin.
	pub async fn post(&self, path: &str) -> Result {
		let url = format!("{}{path}", self.url);
		tokio::task::spawn_blocking(move || ureq::post(&url).send_empty().map(|_| ()).map_err(|e| format!("{url}: {e}")))
			.await
			.map_err(|e| e.to_string())?
	}

	pub async fn press(&self, key: usize, ms: u64) -> Result {
		self.post(&format!("/api/key/{key}/press?ms={ms}")).await
	}

	pub async fn rotate(&self, dial: usize, ticks: i32) -> Result {
		self.post(&format!("/api/dial/{dial}/rotate?ticks={ticks}")).await
	}

	pub async fn push(&self, dial: usize) -> Result {
		self.post(&format!("/api/dial/{dial}/press")).await
	}

	pub async fn touch(&self, dial: usize) -> Result {
		self.post(&format!("/api/touch/{dial}")).await
	}

	async fn get(&self, path: &str) -> Result<Vec<u8>> {
		let url = format!("{}{path}", self.url);
		tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
			let mut response = ureq::get(&url).call().map_err(|e| format!("{url}: {e}"))?;
			response.body_mut().read_to_vec().map_err(|e| e.to_string())
		})
		.await
		.map_err(|e| e.to_string())?
	}

	pub async fn state(&self) -> Result<Value> {
		serde_json::from_slice(&self.get("/api/state").await?).map_err(|e| e.to_string())
	}

	/// The picture of a key as OpenDeck drew it (a data URL).
	pub async fn key(&self, key: usize) -> Result<String> {
		Ok(self.state().await?["keys"][key].as_str().unwrap_or_default().to_string())
	}

	pub async fn dial(&self, dial: usize) -> Result<String> {
		Ok(self.state().await?["dials"][dial].as_str().unwrap_or_default().to_string())
	}

	/// With SHOTS_DIR set: what the deck shows, as name.json. To compare two builds.
	pub async fn shot(&self, name: &str) -> Result {
		let Ok(dir) = std::env::var("SHOTS_DIR") else { return Ok(()) };
		crate::write(&std::path::Path::new(&dir).join(format!("{name}.json")), self.state().await?.to_string())
	}
}

/// Asks again until the answer is Some, for at most timeout ms.
pub async fn wait_for<T>(what: &str, timeout: u64, mut check: impl AsyncFnMut() -> Result<Option<T>>) -> Result<T> {
	let until = Instant::now() + Duration::from_millis(timeout);
	while Instant::now() < until {
		if let Some(value) = check().await? {
			return Ok(value);
		}
		sleep(100).await;
	}
	Err(format!("timeout waiting for {what}"))
}

pub fn check(condition: bool, message: impl Into<String>) -> Result {
	if condition { Ok(()) } else { Err(message.into()) }
}

pub struct Suite {
	pub filter: String,
	pub count: usize,
	pub failed: usize,
}

impl Suite {
	pub async fn test(&mut self, name: &str, test: impl AsyncFnOnce() -> Result) {
		if !self.filter.is_empty() && !name.contains(&self.filter) {
			return;
		}
		self.count += 1;
		let start = Instant::now();
		match test().await {
			Ok(()) => println!("ok   {name} ({} ms)", start.elapsed().as_millis()),
			Err(error) => {
				self.failed += 1;
				println!("FAIL {name}: {error}");
			}
		}
	}
}
