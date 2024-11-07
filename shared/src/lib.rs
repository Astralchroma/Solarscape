#[cfg(feature = "world")]
pub mod connection;

pub mod data;

#[cfg(feature = "world")]
pub mod structure;

pub mod message {
	#[cfg(feature = "backend")]
	pub mod backend;

	#[cfg(feature = "world")]
	pub mod clientbound;

	#[cfg(feature = "world")]
	pub mod serverbound;
}

#[cfg(feature = "world")]
pub mod triangulation_table;

use std::{hash::BuildHasher, hash::Hasher};

#[derive(Clone, Copy, Default)]
pub struct ShiftHasherBuilder<const E: usize>;

impl<const E: usize> BuildHasher for ShiftHasherBuilder<E> {
	type Hasher = ShiftHasher<E>;

	fn build_hasher(&self) -> Self::Hasher {
		ShiftHasher(0)
	}
}

/// Rust's default [Hasher] implementation is designed to be resistant to HashDoS attacks at the cost of performance,
/// this is fine for most use cases, however in instances where many [HashMap](std::collections::HashMap) lookups are
/// required, this performance cost can be extreme.
///
/// [ShiftHasher] is a [Hasher] implementation that works by simply shifting parts of the hashed values into the final
/// hash, in a similar way to how some other languages (like Java) do it.
///
/// This shouldn't be used as a default across the codebase, only in performance critical situations where the
/// viability of HashDoS attacks is low. More efficient algorithms that mitigate the use of a
/// [HashMap](std::collections::HashMap) entirely should be preferred if reasonable.
pub struct ShiftHasher<const E: usize>(u64);

impl<const E: usize> ShiftHasher<E> {
	const BITS_PER_ELEMENT: usize = 64 / E;
	const MASK: u64 = !(u64::MAX << Self::BITS_PER_ELEMENT);
}

impl<const E: usize> Hasher for ShiftHasher<E> {
	fn finish(&self) -> u64 {
		self.0
	}

	fn write(&mut self, _: &[u8]) {
		unimplemented!();
	}

	fn write_u8(&mut self, i: u8) {
		self.write_u64(i as u64);
	}

	fn write_u16(&mut self, i: u16) {
		self.write_u64(i as u64);
	}

	fn write_u32(&mut self, i: u32) {
		self.write_u64(i as u64);
	}

	fn write_u64(&mut self, i: u64) {
		self.0 <<= Self::BITS_PER_ELEMENT;
		self.0 |= i & Self::MASK;
	}

	fn write_u128(&mut self, i: u128) {
		self.write_u64(i as u64);
	}

	fn write_usize(&mut self, i: usize) {
		self.write_u64(i as u64);
	}

	fn write_i8(&mut self, i: i8) {
		self.write_u64(i as u64);
	}

	fn write_i16(&mut self, i: i16) {
		self.write_u64(i as u64);
	}

	fn write_i32(&mut self, i: i32) {
		self.write_u64(i as u64);
	}

	fn write_i64(&mut self, i: i64) {
		self.write_u64(i as u64);
	}

	fn write_i128(&mut self, i: i128) {
		self.write_u64(i as u64);
	}

	fn write_isize(&mut self, i: isize) {
		self.write_usize(i as usize)
	}
}
