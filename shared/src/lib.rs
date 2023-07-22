#![deny(clippy::unwrap_used)]

use log::LevelFilter::Trace;

pub mod io;
pub mod protocol;
pub mod world;

pub fn setup_logging() {
	env_logger::builder()
		.filter_level(Trace)
		.format_module_path(false)
		.format_target(false)
		.init();
}
