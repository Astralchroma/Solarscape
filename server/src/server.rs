use crate::{connection::Connection, sector::Sector};
use anyhow::Result;
use log::info;
use std::{
	net::SocketAddr,
	slice::Iter,
	sync::{Arc, Weak},
	thread,
};
use tokio::{
	net::{TcpListener, TcpStream},
	sync::RwLock,
};

pub struct Server {
	sectors: Vec<Arc<Sector>>,
	connections: RwLock<Vec<Weak<Connection>>>,
}

impl Server {
	pub async fn run() -> Result<()> {
		let sectors = Sector::load_all()?;

		let server = Arc::new(Self {
			sectors: sectors.clone(),
			connections: RwLock::new(vec![]),
		});

		for sector in sectors.into_iter() {
			thread::Builder::new()
				.name(sector.display_name().to_string())
				.spawn(|| sector.run())?;
		}

		let listener = TcpListener::bind("[::]:23500").await?;
		info!("Listening on [::]:23500");

		loop {
			let (stream, address) = listener.accept().await?;
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

	#[allow(clippy::needless_lifetimes)]
	pub fn sectors<'a>(self: &'a Arc<Self>) -> Iter<'a, Arc<Sector>> {
		self.sectors.iter()
	}
}
