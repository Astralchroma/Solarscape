use crate::{connection::Connection, sector::Sector};
use anyhow::Result;
use log::{info, warn};
use std::slice::Iter;
use std::{
	env, fs,
	net::SocketAddr,
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
		let mut sectors = vec![];

		let mut sectors_path = env::current_dir()?;
		sectors_path.push("sectors");

		for path in fs::read_dir(sectors_path)? {
			let path = path?;
			let file_name = path.file_name();
			let key = file_name.to_string_lossy();

			if key.starts_with('.') {
				warn!("{key} is hidden, skipping.");
				continue;
			}

			if !path.metadata()?.is_dir() {
				warn!("{key} is not a directory, skipping.");
				continue;
			}

			sectors.push(Sector::load(&key)?);

			info!("Sector \"{key}\" Loaded")
		}

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
