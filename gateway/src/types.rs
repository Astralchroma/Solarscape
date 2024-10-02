use argon2::password_hash::rand_core::{OsRng, RngCore};
use axum::response::{IntoResponse, Response};
use email_address::{EmailAddress, Options};
use serde::{de::Unexpected, Deserialize, Deserializer};
use sqlx::{encode::IsNull, error::BoxDynError, Database, Decode, Encode, Type, TypeInfo};
use std::{cell::Cell, cell::RefCell, sync::atomic::AtomicU8, sync::atomic::Ordering::Relaxed};
use time::{macros::datetime, OffsetDateTime};

pub trait InternalError: Into<anyhow::Error> {}

impl InternalError for sqlx::Error {}
impl InternalError for argon2::password_hash::Error {}

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

/// Represents a valid Username which may or may not be registered.
#[derive(Type)]
#[sqlx(transparent)]
pub struct Username(Box<str>);

impl<'d> Deserialize<'d> for Username {
	fn deserialize<D: Deserializer<'d>>(deserializer: D) -> std::result::Result<Self, D::Error> {
		let username = Box::<str>::deserialize(deserializer)?;

		// For simple checks it can often be easier to handwrite the validation rather then pull in a regex library
		if username.len() == 0 || username.len() > 32 {
			return Err(serde::de::Error::invalid_length(
				username.len(),
				&"length between 1..=32",
			));
		}

		for character in username.chars() {
			match character {
				'0'..='9' | 'A'..='Z' | 'a'..='z' | '_' => continue,
				character => {
					return Err(serde::de::Error::invalid_value(
						Unexpected::Char(character),
						&"0-9A-Za-z_",
					))
				}
			}
		}

		Ok(Self(username))
	}
}

/// Represents a valid Email Address which may or may not be verified or in use.
pub struct Email(EmailAddress);

impl<'d> Deserialize<'d> for Email {
	fn deserialize<D: Deserializer<'d>>(deserializer: D) -> std::result::Result<Self, D::Error> {
		let address = Box::<str>::deserialize(deserializer)?;

		const EMAIL_OPTIONS: Options = Options {
			minimum_sub_domains: 2,     // Disallows `example`, but allows `example.com`
			allow_domain_literal: true, // If for some reasons you want to use an IP address... go ahead I guess lmao
			allow_display_text: false,  // We're not Git, we don't want `Astralchroma <astralchroma@proton.me>`
		};

		return match EmailAddress::parse_with_options(&address, EMAIL_OPTIONS) {
			Ok(address) => Ok(Email(address)),
			Err(error) => Err(serde::de::Error::custom(error.to_string())),
		};
	}
}

impl<D: Database> Type<D> for Email
where
	Box<str>: Type<D>,
{
	fn type_info() -> D::TypeInfo {
		<Box<str> as Type<D>>::type_info()
	}

	fn compatible(ty: &<D>::TypeInfo) -> bool {
		ty.type_compatible(&<Box<str> as Type<D>>::type_info())
	}
}

impl<'r, D: Database> Decode<'r, D> for Email
where
	Box<str>: Decode<'r, D>,
{
	fn decode(value: <D>::ValueRef<'r>) -> std::result::Result<Self, BoxDynError> {
		<Box<str> as Decode<D>>::decode(value).map(|address| Self(EmailAddress::new_unchecked(address)))
	}
}

impl<'r, D: Database> Encode<'r, D> for Email
where
	Box<str>: Encode<'r, D>,
{
	fn encode_by_ref(&self, buffer: &mut <D>::ArgumentBuffer<'r>) -> std::result::Result<IsNull, BoxDynError> {
		<Box<str> as Encode<D>>::encode_by_ref(&self.0.email().into_boxed_str(), buffer)
	}
}

#[derive(Type)]
#[sqlx(transparent)]
pub struct Token([u8; 16]);

impl Token {
	pub fn new() -> Self {
		let mut token = Token([0; 16]);
		OsRng.fill_bytes(token.0.as_mut_slice());
		token
	}
}

impl IntoResponse for Token {
	fn into_response(self) -> Response {
		const LOOKUP: [char; 16] = [
			'0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
		];

		let mut response = String::with_capacity(32);
		for byte in self.0 {
			response.push(LOOKUP[(byte >> 4) as usize]);
			response.push(LOOKUP[(byte & 0xF) as usize]);
		}

		response.into_response()
	}
}
