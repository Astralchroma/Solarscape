#![deny(clippy::unwrap_used)]

use log::LevelFilter::Info;
use std::io::Result;
use tokio::runtime::Runtime;

pub mod io;
pub mod protocol;
pub mod world;

/// Initializes the logger and returns the tokio runtime used for async / await and input / output.
pub fn shared_main() -> Result<Runtime> {
	env_logger::builder()
		.filter_level(Info)
		.format_module_path(false)
		.format_target(false)
		.init();

	tokio::runtime::Builder::new_multi_thread().enable_all().build()
}
