use std::ops::{Deref, DerefMut};

pub struct Radians(pub f32);

impl Deref for Radians {
	type Target = f32;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl DerefMut for Radians {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl From<Degrees> for Radians {
	fn from(value: Degrees) -> Self {
		Self(f32::to_radians(*value))
	}
}

pub struct Degrees(pub f32);

impl Deref for Degrees {
	type Target = f32;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl DerefMut for Degrees {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl From<Radians> for Degrees {
	fn from(value: Radians) -> Self {
		Self(f32::to_degrees(*value))
	}
}
