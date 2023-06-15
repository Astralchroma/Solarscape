pub mod connection;
pub mod server;

use crate::server::Server;
use anyhow::Result;
use std::{panic, process::exit, sync::Arc};

#[tokio::main]
async fn main() -> Result<()> {
	// If there is a panic we should always exit immediately, tokio won't do this for us.
	let default_panic = panic::take_hook();
	panic::set_hook(Box::new(move |info| {
		default_panic(info);
		exit(1);
	}));

	Arc::new(Server {}).await_connections().await
}
