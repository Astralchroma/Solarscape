use axum::Router;
use sqlx::PgPool;

mod dev;

pub fn router() -> Router<PgPool> {
	Router::new().nest("/dev", dev::router())
}
