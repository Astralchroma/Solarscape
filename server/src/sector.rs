use crate::{connection::Connection, object::Object};
use anyhow::Result;
use log::info;
use serde::Deserialize;
use solarscape_shared::protocol::Clientbound;
use std::{
	collections::HashMap,
	env,
	fs::{self, DirEntry, File},
	io::Read,
	sync::{atomic::AtomicU32, Arc},
};
use tokio::sync::RwLock;

pub struct Sector {
	pub sector_id: usize,
	pub name: Box<str>,
	pub display_name: Box<str>,
	pub object_id_counter: AtomicU32,
	pub objects: RwLock<HashMap<u32, Arc<Object>>>,
}

impl Sector {
	pub fn load_all() -> Result<Vec<Arc<Sector>>> {
		let mut sectors = vec![];

		let mut sectors_path = env::current_dir()?;
		sectors_path.push("sectors");

		for path in fs::read_dir(sectors_path)? {
			if let Some(sector) = Sector::load(path?, sectors.len())? {
				sectors.push(sector);
			}
		}

		info!(
			"Loaded {} Sectors: {:?}",
			sectors.len(),
			sectors.iter().map(|sector| &sector.display_name).collect::<Vec<_>>()
		);

		Ok(sectors)
	}

	fn load(path: DirEntry, sector_id: usize) -> Result<Option<Arc<Sector>>> {
		let file_name = path.file_name();
		let name = file_name.to_string_lossy();

		if name.starts_with('.') || !path.metadata()?.is_dir() {
			return Ok(None);
		}

		let mut config_path = path.path();
		config_path.push("sector.conf");

		let mut file = File::open(config_path)?;
		let mut bytes = vec![];
		file.read_to_end(&mut bytes)?;
		let string = String::from_utf8(bytes)?;

		let configuration: SectorConfig = hocon::de::from_str(string.as_str())?;

		let sector = Arc::new(Sector {
			sector_id,
			name: name.into(),
			display_name: configuration.display_name,
			object_id_counter: AtomicU32::new(0),
			objects: RwLock::new(HashMap::new()),
		});

		let object = Object::sphere(&sector);

		sector.objects.blocking_write().insert(object.object_id, object);

		Ok(Some(sector))
	}

	pub fn sync(&self, connection: &Arc<Connection>) {
		connection.send(Clientbound::SyncSector {
			name: self.name.clone(),
			display_name: self.display_name.clone(),
		})
	}

	pub async fn subscribe(&self, connection: &Arc<Connection>) {
		connection.send(Clientbound::ActiveSector {
			sector_id: self.sector_id,
		});
		for object in self.objects.read().await.values() {
			object.subscribe(connection).await
		}
	}
}

#[derive(Deserialize)]
struct SectorConfig {
	pub display_name: Box<str>,
}
