use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
// "Message" like clippy wants would conflict with the clientbound equivalent, would rather have ServerboundMessage
// instead of serverbound::Message
#[allow(clippy::module_name_repetitions)]
pub enum ServerboundMessage {}
