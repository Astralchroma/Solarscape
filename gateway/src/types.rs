use crate::{to_bytes, to_string};
use argon2::password_hash::rand_core::{OsRng, RngCore};
use axum::response::{IntoResponse, Response};
use email_address::{EmailAddress, Options};
use serde::{de::Unexpected, Deserialize, Deserializer};
use sqlx::{encode::IsNull, error::BoxDynError, Database, Decode, Encode, Type, TypeInfo};

pub trait InternalError: Into<anyhow::Error> {}

impl InternalError for sqlx::Error {}
impl InternalError for argon2::password_hash::Error {}

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
		to_string(self.0.as_slice()).into_response()
	}
}

// More Jank™️
impl From<&str> for Token {
	fn from(value: &str) -> Self {
		let mut bytes = to_bytes(value);
		while bytes.len() < 16 {
			bytes.push(0)
		}
		Self(bytes.first_chunk().unwrap().clone())
	}
}
