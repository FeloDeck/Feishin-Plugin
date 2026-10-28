// End-to-end test: press keys on the virtual deck (through OpenDeck) and check what a fake Feishin
// gets. Needs OpenDeck with the virtual deck, the plugin and the profile of the test (cargo xtask
// profile test/e2e/layout.json, OpenDeck stopped meanwhile). Port 4333 must be free: quit Feishin.
//
//   cargo xtask e2e [test name filter]
//
// With SHOTS_DIR set, it writes what the deck shows at a few points, to compare two builds.
use std::rc::Rc;

use serde_json::{Value, json};

use crate::Result;
use crate::deck::{Deck, Suite, check, local, sleep, wait_for};
use crate::fake::{Fake, PASSWORD, songs};

/// The commands Feishin gets in this time, at most n.
async fn commands(fake: &Fake, n: usize, timeout: u64) -> Vec<Value> {
	let mut got = Vec::new();
	let _ = wait_for("commands", timeout, async || {
		got.extend(fake.take());
		Ok((got.len() >= n).then_some(()))
	})
	.await;
	got
}

async fn expect(fake: &Fake, expected: &[Value]) -> Result {
	let got = commands(fake, expected.len(), 3000).await;
	// Numbers can come as 40 or 40.0.
	let same = got.len() == expected.len()
		&& got
			.iter()
			.zip(expected)
			.all(|(g, e)| g["event"] == e["event"] && ["volume", "position"].iter().all(|k| g[k].as_f64() == e[k].as_f64()));
	check(same, format!("Feishin got {got:?}, not {expected:?}"))
}

/// Waits until the key shows something else than before, and returns it.
async fn changed(deck: &Deck, key: usize, before: &str) -> Result<String> {
	wait_for(&format!("key {key} to change"), 3000, async || Ok(Some(deck.key(key).await?).filter(|now| now != before))).await
}

async fn shows(deck: &Deck, key: usize, image: &str) -> Result {
	wait_for(&format!("key {key} to show it again"), 3000, async || Ok((deck.key(key).await? == image).then_some(()))).await
}

async fn dial_changed(deck: &Deck, dial: usize, before: &str) -> Result<String> {
	wait_for(&format!("dial {dial} to change"), 3000, async || Ok(Some(deck.dial(dial).await?).filter(|now| now != before))).await
}

/// Back to the player of the start, as each test begins with it.
async fn reset(fake: &Fake) {
	fake.reset();
	sleep(800).await;
	fake.take();
}

async fn suite(s: &mut Suite, fake: &Rc<Fake>, deck: &Deck) {
	s.test("keys show the player", async || {
		sleep(500).await;
		deck.shot("start").await?;
		check(!deck.key(0).await?.is_empty(), "key 0 is empty")?;
		check(deck.key(7).await? != deck.key(11).await?, "the volume does not show")?;
		check(deck.key(0).await? != deck.key(14).await?, "the time does not show")?;
		check(deck.key(0).await? != deck.key(10).await?, "the cover does not show")
	})
	.await;

	s.test("play/pause shows the cover of the song", async || {
		let first = deck.key(10).await?;
		let before = fake.covers();
		fake.set("song", songs()[1].clone());
		let second = changed(deck, 10, &first).await?;
		deck.shot("cover").await?;
		// No cover: the icon with the time.
		fake.set("song", songs()[2].clone());
		changed(deck, 10, &second).await?;
		reset(fake).await;
		shows(deck, 10, &first).await?;
		// The plugin keeps only the last cover, so it asks for the first one again.
		let asked = fake.covers() - before;
		check(asked == 2, format!("asked for {asked} covers, not 2"))
	})
	.await;

	s.test("play and pause", async || {
		let paused = deck.key(0).await?;
		deck.press(0, 100).await?;
		expect(fake, &[json!({ "event": "play" })]).await?;
		changed(deck, 0, &paused).await?;
		deck.press(0, 100).await?;
		expect(fake, &[json!({ "event": "pause" })]).await?;
		reset(fake).await;
		shows(deck, 0, &paused).await
	})
	.await;

	s.test("next and previous", async || {
		let track = deck.dial(1).await?;
		deck.press(2, 100).await?;
		expect(fake, &[json!({ "event": "next" })]).await?;
		dial_changed(deck, 1, &track).await?;
		deck.press(1, 100).await?;
		expect(fake, &[json!({ "event": "previous" })]).await?;
		check(fake.player()["song"]["name"] == "First Song", "not back at the first song")?;
		reset(fake).await;
		wait_for("the track", 3000, async || Ok((deck.dial(1).await? == track).then_some(()))).await
	})
	.await;

	s.test("shuffle", async || {
		let off = deck.key(3).await?;
		deck.press(3, 100).await?;
		expect(fake, &[json!({ "event": "shuffle" })]).await?;
		changed(deck, 3, &off).await?;
		deck.press(3, 100).await?;
		expect(fake, &[json!({ "event": "shuffle" })]).await?;
		shows(deck, 3, &off).await
	})
	.await;

	s.test("repeat goes through its modes", async || {
		let none = deck.key(4).await?;
		deck.press(4, 100).await?;
		let all = changed(deck, 4, &none).await?;
		deck.press(4, 100).await?;
		let one = changed(deck, 4, &all).await?;
		check(one != none, "repeat one looks like repeat off")?;
		deck.press(4, 100).await?;
		shows(deck, 4, &none).await?;
		expect(fake, &[json!({ "event": "repeat" }), json!({ "event": "repeat" }), json!({ "event": "repeat" })]).await
	})
	.await;

	s.test("volume keys", async || {
		let up = deck.key(7).await?;
		deck.press(7, 100).await?;
		expect(fake, &[json!({ "event": "volume", "volume": 55 })]).await?;
		changed(deck, 7, &up).await?;
		deck.press(5, 100).await?;
		expect(fake, &[json!({ "event": "volume", "volume": 50 })]).await?;
		shows(deck, 7, &up).await?;
		deck.press(12, 100).await?;
		expect(fake, &[json!({ "event": "volume", "volume": 40 })]).await?;
		reset(fake).await;
		Ok(())
	})
	.await;

	s.test("holding a volume key keeps going", async || {
		deck.press(7, 1000).await?;
		let got = commands(fake, 10, 1500).await;
		let volumes: Vec<f64> = got.iter().filter_map(|c| c["volume"].as_f64()).collect();
		reset(fake).await;
		check(volumes.len() >= 4, format!("only {volumes:?}"))?;
		check(volumes.windows(2).all(|w| w[1] == w[0] + 5.0), format!("not in steps of 5: {volumes:?}"))
	})
	.await;

	s.test("mute and unmute", async || {
		let on = deck.key(6).await?;
		deck.press(6, 100).await?;
		expect(fake, &[json!({ "event": "volume", "volume": 0 })]).await?;
		changed(deck, 6, &on).await?;
		deck.press(6, 100).await?;
		expect(fake, &[json!({ "event": "volume", "volume": 50 })]).await?;
		shows(deck, 6, &on).await
	})
	.await;

	s.test("seek keys", async || {
		deck.press(9, 100).await?;
		expect(fake, &[json!({ "event": "position", "position": 40 })]).await?;
		deck.press(8, 100).await?;
		expect(fake, &[json!({ "event": "position", "position": 30 })]).await?;
		deck.press(13, 100).await?;
		expect(fake, &[json!({ "event": "position", "position": 60 })]).await?;
		reset(fake).await;
		Ok(())
	})
	.await;

	s.test("volume dial", async || {
		let before = deck.dial(0).await?;
		deck.rotate(0, 2).await?;
		expect(fake, &[json!({ "event": "volume", "volume": 54 })]).await?;
		dial_changed(deck, 0, &before).await?;
		deck.rotate(2, -1).await?;
		expect(fake, &[json!({ "event": "volume", "volume": 49 })]).await?;
		deck.push(0).await?;
		expect(fake, &[json!({ "event": "volume", "volume": 0 })]).await?;
		deck.touch(0).await?;
		expect(fake, &[json!({ "event": "volume", "volume": 49 })]).await?;
		reset(fake).await;
		Ok(())
	})
	.await;

	s.test("seek dial", async || {
		deck.rotate(1, 2).await?;
		expect(fake, &[json!({ "event": "position", "position": 40 })]).await?;
		deck.rotate(3, -1).await?;
		expect(fake, &[json!({ "event": "position", "position": 30 })]).await?;
		deck.push(1).await?;
		expect(fake, &[json!({ "event": "play" })]).await?;
		deck.touch(1).await?;
		expect(fake, &[json!({ "event": "pause" })]).await?;
		reset(fake).await;
		Ok(())
	})
	.await;

	let mut offline = String::new();
	s.test("keys say when Feishin is gone", async || {
		let next = deck.key(2).await?;
		fake.stop();
		offline = changed(deck, 2, &next).await?;
		sleep(300).await;
		let state = deck.state().await?;
		let keys: Vec<&str> = state["keys"].as_array().map(|k| k.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
		check(keys.iter().all(|k| *k == offline), "not every key shows it")?;
		deck.shot("offline").await?;
		fake.start().await?;
		// The plugin tries again every 5 seconds at most.
		wait_for("the plugin to connect", 8000, async || Ok((fake.clients() > 0).then_some(()))).await?;
		shows(deck, 2, &next).await
	})
	.await;

	s.test("keys say when the password is wrong", async || {
		let next = deck.key(2).await?;
		fake.set_password("other");
		let password = changed(deck, 2, &next).await?;
		check(password != offline, "looks like offline")?;
		deck.shot("password").await?;
		fake.set_password(PASSWORD);
		// A press tries again right away, instead of in 15 seconds.
		deck.press(2, 100).await?;
		shows(deck, 2, &next).await?;
		check(fake.take().is_empty(), "the press went to Feishin")
	})
	.await;
}

pub fn run(args: &[String]) -> Result {
	let filter = args.first().cloned().unwrap_or_default();
	local(async move {
		let fake = Fake::new();
		fake.start().await?;
		let deck = Deck::new();
		wait_for("the plugin to connect (is it on the deck?)", 15000, async || Ok((fake.clients() > 0).then_some(()))).await?;
		let mut s = Suite { filter, count: 0, failed: 0 };
		suite(&mut s, &fake, &deck).await;
		fake.stop();
		println!("\n{}/{} passed", s.count - s.failed, s.count);
		if s.failed > 0 { Err(format!("{} failed", s.failed)) } else { Ok(()) }
	})
}
