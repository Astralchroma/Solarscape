#![warn(clippy::pedantic, clippy::nursery)]

pub mod messages;

use chrono::{Local, SubsecRound};
use log::{Log, Metadata, Record};

// Existing logger implementations are overly complicated for our needs currently, so we'll just write a dummy logger
pub struct StdLogger;

impl Log for StdLogger {
	fn enabled(&self, _: &Metadata) -> bool {
		true
	}

	fn log(&self, record: &Record) {
		// hi please stop dumping useless crap in the logs, thanks
		if record
			.module_path()
			.map_or(true, |path| !path.starts_with("solarscape"))
		{
			return;
		}

		let timestamp = Local::now().round_subsecs(0).to_rfc3339();
		println!("{timestamp} {:5}: {}", record.level(), record.args());
	}

	fn flush(&self) {}
}
