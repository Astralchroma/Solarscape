use crate::connection::Connection;
use anyhow::Result;
use std::{
	net::SocketAddr,
	sync::{Arc, Weak},
};
use tokio::{
	net::{TcpListener, TcpStream},
	sync::RwLock,
};

pub struct Server {
	connections: RwLock<Vec<Weak<Connection>>>,
}

impl Server {
	pub fn new() -> Arc<Self> {
		Arc::new(Self {
			connections: RwLock::new(vec![]),
		})
	}

	pub async fn await_connections(self: Arc<Self>) -> Result<()> {
		let listener = TcpListener::bind("[::]:23500").await?;
		println!("Listening on [::]:23500");

		loop {
			let (stream, address) = listener.accept().await?;
			tokio::spawn(self.clone().accept_connection(stream, address));
		}
	}

	async fn accept_connection(self: Arc<Self>, stream: TcpStream, address: SocketAddr) {
		if let Some(connection) = Connection::accept(stream, address).await {
			let mut connections = self.connections.write().await;
			connections.push(Arc::downgrade(&connection))
		}
	}
}
