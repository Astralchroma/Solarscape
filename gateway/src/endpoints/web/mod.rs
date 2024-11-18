use crate::{
	types::{Email, InternalError, Username},
	Gateway, ARGON_2,
};
use argon2::{
	password_hash::{rand_core::OsRng, SaltString},
	PasswordHasher,
};
use axum::{
	debug_handler,
	extract::{Query, State},
	http::{HeaderMap, HeaderValue, StatusCode},
	response::{IntoResponse, Response},
	routing::get,
	Router,
};
use serde::Deserialize;
use solarscape_shared::data::Id;
use sqlx::{error::ErrorKind::UniqueViolation, query, Error::Database};
use thiserror::Error;

#[derive(Deserialize)]
struct CreateAccount {
	username: Username,
	email: Email,
	password: Box<str>,
}

#[debug_handler]
async fn create_account(
	State(Gateway { database, .. }): State<Gateway>,
	Query(CreateAccount {
		username,
		email,
		password,
	}): Query<CreateAccount>,
) -> Result<&'static str, CreateAccountError> {
	let salt = SaltString::generate(&mut OsRng);
	let password = ARGON_2
		.hash_password(password.as_bytes(), &salt)?
		.to_string();
	let id = Id::new();

	let mut transaction = database.begin().await?;

	query!("INSERT INTO inventories(id) VALUES ($1)", id as _)
		.execute(&mut *transaction)
		.await?;

	let result = query!(
		"INSERT INTO players(id, username, email, password) VALUES ($1, $2, $3, $4)",
		id as _,
		username as _,
		email as _,
		password
	)
	.execute(&mut *transaction)
	.await;

	return match result {
		Ok(_) => {
			transaction.commit().await?;
			Ok(r#"<p style="color:green">Account Created!</p>"#)
		}
		Err(error) => Err(match error {
			Database(error) if matches!(error.kind(), UniqueViolation) => {
				CreateAccountError::AccountExists
			}
			_ => error.into(),
		}),
	};
}

#[derive(Debug, Error)]
enum CreateAccountError {
	#[error("Account Exists!")]
	AccountExists,

	#[error(transparent)]
	Internal(#[from] anyhow::Error),
}

impl<E: InternalError> From<E> for CreateAccountError {
	fn from(value: E) -> Self {
		Self::Internal(value.into())
	}
}

impl IntoResponse for CreateAccountError {
	fn into_response(self) -> Response {
		use log::error;

		match self {
			CreateAccountError::AccountExists => (
				StatusCode::CONFLICT,
				r#"<p style="color:red">Account Exists!</p>"#,
			),
			CreateAccountError::Internal(error) => {
				error!("{error}");
				(
					StatusCode::INTERNAL_SERVER_ERROR,
					r#"<p style="color:red">Internal / Unknown Error!</p>"#,
				)
			}
		}
		.into_response()
	}
}

// Probably a more sane way to serve static content, but it's just two files, who cares
#[debug_handler]
async fn root() -> impl IntoResponse {
	let mut html_header_map = HeaderMap::new();
	html_header_map.append(
		"Content-Type",
		HeaderValue::from_static("text/html;charset=utf-8"),
	);

	(html_header_map, include_str!("index.html"))
}

#[debug_handler]
async fn htmx() -> impl IntoResponse {
	let mut js_header_map = HeaderMap::new();
	js_header_map.append(
		"Content-Type",
		HeaderValue::from_static("text/javascript;charset=utf-8"),
	);

	(js_header_map, include_str!("htmx-2.0.2.min.js"))
}

pub fn router() -> Router<Gateway> {
	Router::new()
		.route("/index.html", get(root))
		.route("/htmx-2.0.2.min.js", get(htmx))
		.route("/create_account", get(create_account))
}
