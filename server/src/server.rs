use crate::connection::Connection;
use anyhow::Result;
use std::sync::Arc;
use tokio::net::TcpListener;

pub struct Server;

impl Server {
	pub async fn await_connections(self: Arc<Self>) -> Result<()> {
		let listener = TcpListener::bind("[::]:23500").await?;
		println!("Listening on [::]:23500");

		loop {
			let (socket, address) = listener.accept().await?;
			tokio::spawn(Connection::new(socket, address));
		}
	}
}
