use crate::Gateway;
use axum::Router;

mod dev;

pub fn router() -> Router<Gateway> {
	Router::new().nest("/dev", dev::router())
}
