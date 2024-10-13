use crate::{extractors::Authenticated, types::Email, types::InternalError, types::Token, Gateway, ARGON_2};
use argon2::{password_hash::Error as ArgonError, PasswordHash, PasswordVerifier};
use axum::response::{IntoResponse, Response};
use axum::{debug_handler, extract::Query, extract::State, http::StatusCode, routing::get, Json, Router};
use chacha20poly1305::{aead::OsRng, ChaCha20Poly1305, KeyInit};
use serde::{Deserialize, Serialize};
use solarscape_backend_types::messages::AllowConnection;
use sqlx::{query, query_scalar};
use thiserror::Error;

#[derive(Deserialize)]
struct GetToken {
	email: Email,
	password: Box<str>,
}

#[debug_handler]
async fn token(
	State(Gateway { database, .. }): State<Gateway>,
	Query(GetToken { email, password }): Query<GetToken>,
) -> Result<Token, GetTokenError> {
	let mut transaction = database.begin().await?;

	let player = query!("SELECT id, phc_password FROM players WHERE email = $1", email as _)
		.fetch_optional(&mut *transaction)
		.await?
		.ok_or(GetTokenError::AccountDoesNotExist)?;

	let result = ARGON_2.verify_password(password.as_bytes(), &PasswordHash::new(&player.phc_password)?);

	match result {
		Ok(_) => {}
		Err(error) => {
			return Err(match error {
				ArgonError::Password => GetTokenError::IncorrectPassword,
				error => error.into(),
			})
		}
	}

	// The chance of a token collision is extremely unlikely, so we won't
	// bother coming up with a fancy scheme for always unique tokens
	let token = loop {
		let token = Token::new();

		let exists = query_scalar!(
			"SELECT EXISTS (SELECT 1 FROM tokens WHERE token = $1) AS \"exists!\"",
			token as _
		)
		.fetch_one(&mut *transaction)
		.await?;

		match exists {
			true => continue,
			false => break token,
		}
	};

	query!("INSERT INTO tokens VALUES ($1, $2)", token as _, player.id)
		.execute(&mut *transaction)
		.await?;

	transaction.commit().await?;

	Ok(token)
}

#[derive(Debug, Error)]
enum GetTokenError {
	#[error("Account does not exist")]
	AccountDoesNotExist,

	#[error("Incorrect Password")]
	IncorrectPassword,

	#[error(transparent)]
	Internal(#[from] anyhow::Error),
}

impl<E: InternalError> From<E> for GetTokenError {
	fn from(value: E) -> Self {
		Self::Internal(value.into())
	}
}

impl IntoResponse for GetTokenError {
	fn into_response(self) -> Response {
		use log::error;

		match self {
			GetTokenError::AccountDoesNotExist => (StatusCode::NOT_FOUND, "Account does not exist"),
			GetTokenError::IncorrectPassword => (StatusCode::UNAUTHORIZED, "Incorrect Password"),
			GetTokenError::Internal(error) => {
				error!("{error}");
				(StatusCode::INTERNAL_SERVER_ERROR, "Internal / Unknown Error")
			}
		}
		.into_response()
	}
}

#[debug_handler]
async fn connect(
	State(Gateway { database, cl_args }): State<Gateway>,
	Authenticated(id): Authenticated,
) -> Result<Json<ConnectionInfo>, ConnectError> {
	// Generate Encryption Key
	let key = ChaCha20Poly1305::generate_key(&mut OsRng);

	// Send Key to Sector Server through Channel
	// Currently, sector servers just create a channel with the same name as the sector
	// This is fine for now, but will need to be improved when we implement proper support for multiple sectors
	let allow_connection = AllowConnection { id, key: key.into() };
	let message = serde_json::to_string(&allow_connection).unwrap();
	query!(
		"SELECT pg_notify(channel, message) FROM (VALUES ($1, $2)) notifies(channel, message)",
		cl_args.sector,
		message,
	)
	.execute(&database)
	.await?;

	// Respond with Connection Info
	Ok(Json(ConnectionInfo {
		key: key.into(),
		address: cl_args.sector_address.clone(),
	}))
}

#[derive(Serialize)]
struct ConnectionInfo {
	key: [u8; 32],
	address: String,
}

#[derive(Debug, Error)]
enum ConnectError {
	#[error(transparent)]
	Internal(#[from] anyhow::Error),
}

impl<E: InternalError> From<E> for ConnectError {
	fn from(value: E) -> Self {
		Self::Internal(value.into())
	}
}

impl IntoResponse for ConnectError {
	fn into_response(self) -> Response {
		use log::error;

		match self {
			ConnectError::Internal(error) => {
				error!("{error}");
				(StatusCode::INTERNAL_SERVER_ERROR, "Internal / Unknown Error")
			}
		}
		.into_response()
	}
}

pub fn router() -> Router<Gateway> {
	Router::new()
		.route("/token", get(token))
		.route("/connect", get(connect))
}
