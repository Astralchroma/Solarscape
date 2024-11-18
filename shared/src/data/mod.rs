#[cfg(feature = "world")]
pub mod world;

use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};

#[cfg(feature = "backend")]
use sqlx::{encode::IsNull, error::BoxDynError, Database, Decode, Encode, Type, TypeInfo};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Id(u64);

#[cfg(feature = "backend")]
impl Id {
	pub fn new() -> Self {
		use std::{
			cell::Cell, cell::RefCell, sync::atomic::AtomicU8, sync::atomic::Ordering::Relaxed,
		};
		use time::{macros::datetime, OffsetDateTime};

		static THREAD_ID_COUNTER: AtomicU8 = AtomicU8::new(0);

		thread_local! {
			static THREAD_ID: Cell<u8> = {
				let thread_id = THREAD_ID_COUNTER.fetch_add(1, Relaxed);
				assert!(thread_id < u8::pow(2, 5));
				Cell::new(thread_id)
			};
			static COUNTER: RefCell<u16> = const { RefCell::new(0) };
		}

		const SOLARSCAPE_EPOCH: OffsetDateTime = datetime!(2024-01-01 00:00 UTC);

		let timestamp =
			((OffsetDateTime::now_utc() - SOLARSCAPE_EPOCH).whole_seconds() as u64) << 22;
		let thread_id = (THREAD_ID.get() as u64) << 12;
		let counter = COUNTER.with_borrow_mut(|counter| {
			let result = *counter;
			*counter += 1;
			if counter == &u16::pow(2, 12) {
				*counter = 0
			}
			result as u64
		});

		Id(timestamp | thread_id | counter)
	}
}

impl Display for Id {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.0)
	}
}

#[cfg(feature = "backend")]
impl<D: Database> Type<D> for Id
where
	i64: Type<D>,
{
	fn type_info() -> D::TypeInfo {
		<i64 as Type<D>>::type_info()
	}

	fn compatible(ty: &<D>::TypeInfo) -> bool {
		ty.type_compatible(&<i64 as Type<D>>::type_info())
	}
}

#[cfg(feature = "backend")]
impl<'r, D: Database> Decode<'r, D> for Id
where
	i64: Decode<'r, D>,
{
	fn decode(value: <D>::ValueRef<'r>) -> std::result::Result<Self, BoxDynError> {
		<i64 as Decode<D>>::decode(value).map(|value| Self(value as u64))
	}
}

#[cfg(feature = "backend")]
impl<'r, D: Database> Encode<'r, D> for Id
where
	i64: Encode<'r, D>,
{
	fn encode_by_ref(
		&self,
		buffer: &mut <D>::ArgumentBuffer<'r>,
	) -> std::result::Result<IsNull, BoxDynError> {
		<i64 as Encode<D>>::encode_by_ref(&(self.0 as i64), buffer)
	}
}
