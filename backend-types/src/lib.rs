pub mod messages {
	use crate::types::Id;
	use serde::{Deserialize, Serialize};

	#[derive(Deserialize, Serialize)]
	pub struct AllowConnection {
		pub id: Id,
		pub key: [u8; 32],
	}
}

pub mod types {
	use serde::{Deserialize, Serialize};
	use sqlx::{encode::IsNull, error::BoxDynError, Database, Decode, Encode, Type, TypeInfo};
	use std::{cell::Cell, cell::RefCell, sync::atomic::AtomicU8, sync::atomic::Ordering::Relaxed};
	use time::{macros::datetime, OffsetDateTime};

	#[derive(Clone, Copy, Deserialize, Serialize)]
	pub struct Id(u64);

	impl Id {
		pub fn new() -> Self {
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

			let timestamp = ((OffsetDateTime::now_utc() - SOLARSCAPE_EPOCH).whole_seconds() as u64) << 22;
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

	impl<'r, D: Database> Decode<'r, D> for Id
	where
		i64: Decode<'r, D>,
	{
		fn decode(value: <D>::ValueRef<'r>) -> std::result::Result<Self, BoxDynError> {
			<i64 as Decode<D>>::decode(value).map(|value| Self(value as u64))
		}
	}

	impl<'r, D: Database> Encode<'r, D> for Id
	where
		i64: Encode<'r, D>,
	{
		fn encode_by_ref(&self, buffer: &mut <D>::ArgumentBuffer<'r>) -> std::result::Result<IsNull, BoxDynError> {
			<i64 as Encode<D>>::encode_by_ref(&(self.0 as i64), buffer)
		}
	}
}
