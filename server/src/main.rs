#![deny(clippy::unwrap_used)]

mod chunk;
mod connection;
mod sector;
mod voxject;
mod world;

pub use chunk::*;
pub use connection::*;
pub use sector::*;
pub use voxject::*;
pub use world::*;

use crate::world::World;
use anyhow::Result;
use solarscape_shared::setup_logging;
use std::{env, fs};

fn main() -> Result<()> {
	setup_logging();

	let mut cargo = env::current_dir()?;
	cargo.push("Cargo.toml");

	// if Cargo.toml exists, assume we are running in a development environment.
	if cargo.exists() {
		let mut data = env::current_dir()?;
		data.push("server");
		data.push("run");

		fs::create_dir_all(data.clone())?;
		env::set_current_dir(data)?;
	}

	let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;

	runtime.block_on(World::run())
}
