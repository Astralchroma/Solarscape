use crate::sector::Data;
use nalgebra::{vector, zero, Vector3};
use solarscape_shared::data::world::{ChunkCoordinates, Material};

pub type Generator = fn(&ChunkCoordinates) -> Data;

pub fn sphere_chunk_data(
	coordinates: &ChunkCoordinates,
	radius: f32,
	material_map: impl Fn(f32) -> Material,
) -> Data {
	let mut data = Data::default();
	let level_radius = radius / f32::powi(2.0, *coordinates.level as i32);
	let chunk_origin_level_coordinates =
		coordinates.cast() * f32::powi(16.0, *coordinates.level as i32 + 1);

	for x in 0..16 {
		for y in 0..16 {
			for z in 0..16 {
				let index = x << 8 | y << 4 | z;
				let level_coordinates =
					chunk_origin_level_coordinates + vector![x as f32, y as f32, z as f32];
				let distance = level_coordinates.metric_distance(&zero::<Vector3<_>>()) - 32.0;
				data.densities[index] = level_radius - distance;
				data.materials[index] = material_map(distance);
			}
		}
	}

	data
}

pub fn sphere_generator(coordinates: &ChunkCoordinates) -> Data {
	sphere_chunk_data(coordinates, 32.0, |distance| {
		if distance >= 32.0 {
			Material::Nothing
		} else if distance >= 30.0 {
			Material::Ground
		} else if distance >= 16.0 {
			Material::Stone
		} else {
			Material::Corium
		}
	})
}
