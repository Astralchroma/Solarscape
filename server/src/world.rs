use crate::{connection::Connection, sector::Sector};
use anyhow::Result;
use std::sync::Arc;

pub struct World {
	pub sectors: Vec<Arc<Sector>>,
}

impl World {
	pub fn new() -> Result<Arc<World>> {
		let sectors = Sector::load_all()?;

		Ok(Arc::new(Self {
			sectors: sectors.clone(),
		}))
	}

	pub fn sync(&self, connection: &Arc<Connection>) {
		self.sectors.iter().for_each(|sector| sector.sync(connection))
	}
}
