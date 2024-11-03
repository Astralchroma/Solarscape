use crate::{types::InternalError, types::Token, Gateway};
use axum::response::{IntoResponse, Response};
use axum::{async_trait, extract::FromRequestParts, http::request::Parts, http::StatusCode};
use solarscape_shared::types::Id;
use sqlx::{query, query_scalar};
use thiserror::Error;

#[derive(Clone, Copy)]
pub struct Authenticated(pub Id);

#[async_trait]
impl FromRequestParts<Gateway> for Authenticated {
	type Rejection = AuthenticationError;

	async fn from_request_parts(
		parts: &mut Parts,
		Gateway { database, .. }: &Gateway,
	) -> Result<Self, Self::Rejection> {
		let token: Token = parts
			.headers
			.get("Authorization")
			.map(|value| value.to_str())
			.ok_or(AuthenticationError::Unauthorized)?
			.map_err(|_| AuthenticationError::Unauthorized)?
			.into();

		let id: Id = query_scalar!(
			r#"SELECT player_id AS "id: Id" FROM tokens WHERE token = $1 AND valid = true"#,
			token as _
		)
		.fetch_one(database)
		.await?
		.ok_or(AuthenticationError::Unauthorized)?;

		query!("UPDATE tokens SET used = DEFAULT WHERE token = $1", token as _)
			.execute(database)
			.await?;

		Ok(Self(id))
	}
}

#[derive(Debug, Error)]
pub enum AuthenticationError {
	#[error("Unauthorized")]
	Unauthorized,

	#[error(transparent)]
	Internal(#[from] anyhow::Error),
}

impl<E: InternalError> From<E> for AuthenticationError {
	fn from(value: E) -> Self {
		Self::Internal(value.into())
	}
}

impl IntoResponse for AuthenticationError {
	fn into_response(self) -> Response {
		use log::error;

		match self {
			AuthenticationError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized"),
			AuthenticationError::Internal(error) => {
				error!("{error}");
				(StatusCode::INTERNAL_SERVER_ERROR, "Internal Error")
			}
		}
		.into_response()
	}
}
