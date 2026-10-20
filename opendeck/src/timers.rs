// Timers that call back once. The callback runs as its own task, so clearing a timer never stops a
// callback that already started.
use std::time::Duration;

use tokio::task::{AbortHandle, spawn_local};

#[derive(Debug)]
pub struct Timer(AbortHandle);

impl Timer {
	pub fn clear(&self) {
		self.0.abort();
	}
}

fn duration(ms: f64) -> Duration {
	// At least 1 ms.
	Duration::from_secs_f64(if ms.is_finite() && ms >= 1.0 { ms } else { 1.0 } / 1000.0)
}

pub fn set_timeout(ms: f64, callback: impl FnOnce() + 'static) -> Timer {
	let handle = spawn_local(async move {
		tokio::time::sleep(duration(ms)).await;
		// A task of its own: clear() after this point does not cancel the callback.
		spawn_local(async move { callback() });
	});
	Timer(handle.abort_handle())
}
