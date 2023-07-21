use crate::{Connection, Sector};
use anyhow::Result;
use log::info;
use std::{
	net::SocketAddr,
	sync::{Arc, Weak},
};
use tokio::{
	net::{TcpListener, TcpStream},
	sync::RwLock,
};

pub struct World {
	pub sectors: Vec<Arc<Sector>>,
	connections: RwLock<Vec<Weak<Connection>>>,
}

impl World {
	pub async fn run() -> Result<()> {
		let sectors = Sector::load_all()?;

		let server = Arc::new(Self {
			sectors: sectors.clone(),
			connections: RwLock::new(vec![]),
		});

		let socket = TcpListener::bind("[::]:23500").await?;
		info!("Listening on [::]:23500");

		loop {
			let (stream, address) = socket.accept().await?;
			tokio::spawn(server.clone().accept_connection(stream, address));
		}
	}

	async fn accept_connection(self: Arc<Self>, stream: TcpStream, address: SocketAddr) {
		if let Some(connection) = Connection::accept(self.clone(), stream, address).await {
			let mut connections = self.connections.write().await;
			connections.push(Arc::downgrade(&connection));
			connections.retain(|connection| connection.strong_count() != 0)
		}
	}
}
