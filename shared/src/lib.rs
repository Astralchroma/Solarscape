#![deny(clippy::unwrap_used)]

use log::LevelFilter::Info;
use std::{io::Result, time::Duration};
use tokio::runtime::Runtime;

pub mod chunk;
pub mod connection;
pub mod protocol;

pub const TICKS_PER_SECOND: u32 = 30;
pub const TICK_DURATION: Duration = Duration::from_nanos(1_000_000_000 / TICKS_PER_SECOND as u64);

/// Initializes the logger and returns the tokio runtime used for async / await and input / output.
pub fn shared_main() -> Result<Runtime> {
	env_logger::builder()
		.filter_level(Info)
		.format_module_path(false)
		.format_target(false)
		.init();

	tokio::runtime::Builder::new_multi_thread().enable_io().build()
}
