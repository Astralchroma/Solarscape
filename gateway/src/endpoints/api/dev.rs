use crate::{types::Email, types::InternalError, types::Token, ARGON_2};
use argon2::{password_hash::Error as ArgonError, PasswordHash, PasswordVerifier};
use axum::response::{IntoResponse, Response};
use axum::{debug_handler, extract::Query, extract::State, http::StatusCode, routing::get, Router};
use serde::Deserialize;
use sqlx::{query, query_scalar, PgPool};
use thiserror::Error;

#[derive(Deserialize)]
struct GetToken {
	email: Email,
	password: Box<str>,
}

#[debug_handler]
async fn token(
	State(database): State<PgPool>,
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
				(StatusCode::INTERNAL_SERVER_ERROR, "Internal Error")
			}
		}
		.into_response()
	}
}

pub fn router() -> Router<PgPool> {
	Router::new().route("/token", get(token))
}
