use crate::sector::Sector;
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
}
