use crate::{types::Email, types::Id, types::Username, ARGON_2};
use argon2::{password_hash::rand_core::OsRng, password_hash::SaltString, PasswordHasher};
use axum::{
	debug_handler,
	extract::{Query, State},
	http::{HeaderMap, HeaderValue, StatusCode},
	response::{IntoResponse, Response},
};
use log::error;
use serde::Deserialize;
use sqlx::{error::ErrorKind, query, PgPool};

type RequestResult<T, E = T> = Result<T, Error<E>>;

pub enum Error<E: IntoResponse> {
	Endpoint(E),
	Internal(anyhow::Error),
}

impl<E: IntoResponse> IntoResponse for Error<E> {
	fn into_response(self) -> Response {
		match self {
			Error::Endpoint(error) => error.into_response(),
			Error::Internal(internal_error) => {
				error!("{internal_error}");
				(StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
			}
		}
	}
}

impl<E: IntoResponse, A: Into<anyhow::Error>> From<A> for Error<E> {
	fn from(value: A) -> Self {
		Self::Internal(value.into())
	}
}

#[derive(Deserialize)]
pub struct CreateAccount {
	username: Username,
	email: Email,
	password: Box<str>,
}

#[debug_handler]
pub async fn get_create_account(
	State(database): State<PgPool>,
	Query(CreateAccount {
		username,
		email,
		password,
	}): Query<CreateAccount>,
) -> RequestResult<impl IntoResponse> {
	let salt = SaltString::generate(&mut OsRng);
	let password = ARGON_2.hash_password(password.as_bytes(), &salt)?.to_string();

	let id = Id::new();
	let result = query!(
		"INSERT INTO players VALUES ($1, $2, $3, $4)",
		id as _,
		username as _,
		email as _,
		password
	)
	.execute(&database)
	.await;

	return match result {
		Ok(_) => Ok(r#"<p id=message style="color: green">Account Created!</p>"#),
		Err(error) => match error {
			sqlx::Error::Database(error) if matches!(error.kind(), ErrorKind::UniqueViolation) => {
				Ok(r#"<p id=message style="color: red">Account Already Exists!</p>"#)
			}
			_ => Err(error)?,
		},
	};
}

// Probably a more sane way to serve static content, but it's just two files, who cares
#[debug_handler]
pub async fn get_root() -> impl IntoResponse {
	let mut html_header_map = HeaderMap::new();
	html_header_map.append("Content-Type", HeaderValue::from_static("text/html;charset=utf-8"));

	(html_header_map, include_str!("index.html"))
}

#[debug_handler]
pub async fn get_htmx() -> impl IntoResponse {
	let mut js_header_map = HeaderMap::new();
	js_header_map.append(
		"Content-Type",
		HeaderValue::from_static("text/javascript;charset=utf-8"),
	);

	(js_header_map, include_str!("htmx-2.0.2.min.js"))
}
