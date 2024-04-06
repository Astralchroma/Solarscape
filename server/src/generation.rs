// The future plan for world generation is to have a FFI plugin system and a library for writing world generation
// plugins, for now though, we'll just do some basic world generation in here.
//
// When that happens, we'll probably start by just refactoring what's here into the library.
//
// Also should be noted that this module isn't really very well planned ahead, so current capabilities are likely
// inadequate for any complicated generation.

use crate::world::Chunk;
use log::warn;
use nalgebra::{vector, zero, Vector3};
use solarscape_shared::types::GridCoordinates;
use tokio::sync::mpsc::UnboundedSender as Sender;

type GeneratorFunction = (dyn (Fn(GridCoordinates) -> Chunk) + Send + Sync);

// We plan to do some FFI stuff and put generators in an external library which gets loaded at runtime, so because of
// this we make this a wrapper struct and not a trait. For convenience it also must be Send, Sync, and Clone so we can
// share it between threads, the implications of that are up to generators to handle, although they shouldn't have any
// shared mutable state anyway so this shouldn't cause any issues.
#[derive(Clone, Copy)]
pub struct Generator(&'static GeneratorFunction);

impl Generator {
	pub fn new(generator_function: &'static GeneratorFunction) -> Self {
		Self(generator_function)
	}

	pub fn generate(&self, grid_coordinates: GridCoordinates, completion_channel: &Sender<Chunk>) {
		let generator = *self;
		let completion_channel = completion_channel.clone();
		rayon::spawn_fifo(move || {
			let chunk = generator.0(grid_coordinates);
			if completion_channel.send(chunk).is_err() {
				// TODO: Maybe we should have a "server stopping" flag
				warn!("Failed to return completed chunk ({grid_coordinates}), either the server is stopping or something broke");
			}
		})
	}
}

// While Chunk is defined in world.rs, the later chunk generation library will likely have it's own [Chunk] later, so
// we'll keep all the generation specific functions here.
impl Chunk {
	pub fn sphere(mut self, radius: f32) -> Self {
		let level_radius = radius / f32::powi(2.0, self.grid_coordinates.level as i32);
		let chunk_origin_level_coordinates = self.grid_coordinates.coordinates.cast() * 16.0;

		for x in 0..16 {
			for y in 0..16 {
				for z in 0..16 {
					let level_coordinates = chunk_origin_level_coordinates + vector![x as f32, y as f32, z as f32];
					let distance = level_coordinates.metric_distance(&zero::<Vector3<_>>());
					self.densities[x << 8 | y << 4 | z] = (256.0 * (level_radius - distance)) as u8;
				}
			}
		}

		self
	}
}

pub fn sphere_generator(grid_coordinates: GridCoordinates) -> Chunk {
	Chunk::new(grid_coordinates).sphere(8.0)
}
