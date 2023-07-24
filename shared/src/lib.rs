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

/// Shorthand to be used in place of `..Default::default()`. This is a copy of Rust's "default_free_fn" unstable feature
/// because we don't want to use nightly, plus they seem to be removing it anyway.
#[must_use]
#[inline]
pub fn default<T: Default>() -> T {
	Default::default()
}
