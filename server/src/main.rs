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

use crate::World;
use anyhow::Result;
use log::error;
use solarscape_shared::setup_logging;
use std::{env, fs};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<()> {
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

	let token = CancellationToken::new();

	let server_token = token.clone();
	tokio::spawn(async move {
		let result = World::run().await;
		server_token.cancel();
		if let Err(error) = result {
			error!("{error:?}");
		}
	});

	token.cancelled_owned().await;
	Ok(())
}
