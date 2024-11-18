use crate::{
	client::{AnyState, State},
	world::Sector,
	ClArgs,
};
use chacha20poly1305::{aead::AeadMutInPlace, ChaCha20Poly1305, KeyInit};
use egui::{Align, Align2, Color32, Context, Layout, RichText, Separator, TextEdit, Vec2, Window};
use serde::Deserialize;
use serde_json::from_str;
use solarscape_shared::connection::Connection;
use tokio::{io::AsyncWriteExt, net::TcpStream, runtime::Handle, task::JoinHandle};

#[derive(Default)]
pub struct Login {
	email: String,
	password: String,

	error: String,
	login: Option<JoinHandle<Result<Sector, anyhow::Error>>>,
}

impl Login {
	#[cfg(debug)]
	pub fn from_cl_args(cl_args: &mut ClArgs) -> Self {
		match cl_args.authentication.take() {
			Some(authentication) => Self {
				login: Some(Handle::current().spawn(Self::login(
					cl_args.clone(),
					authentication.email.clone(),
					authentication.password.clone(),
				))),

				email: authentication.email,
				password: authentication.password,

				error: String::new(),
			},
			None => Self::default(),
		}
	}

	async fn login(
		cl_args: ClArgs,
		email: String,
		password: String,
	) -> Result<Sector, anyhow::Error> {
		let reqwest = reqwest::Client::new();

		let token = reqwest
			.get(cl_args.api_endpoint.to_string() + "/dev/token")
			.query(&[("email", email), ("password", password)])
			.send()
			.await?
			.text()
			.await?;

		let details = reqwest
			.get(cl_args.api_endpoint.to_string() + "/dev/connect")
			.header("Authorization", token)
			.send()
			.await?
			.text()
			.await?;

		#[derive(Deserialize)]
		struct ConnectionInfo {
			key: [u8; 32],
			address: String,
		}

		let details: ConnectionInfo = from_str(&details)?;

		let mut key = ChaCha20Poly1305::new_from_slice(&details.key).unwrap(); // For some reason, anyhow can't convert this
		let mut stream = TcpStream::connect(details.address).await?;
		let mut version_data = vec![0; 4];
		key.encrypt_in_place(&[0; 12].into(), b"", &mut version_data)
			.unwrap(); // Anyhow also can't convert this
		stream.write_u16_le(version_data.len() as u16).await?;
		stream.write_all(&version_data).await?;
		stream.flush().await?;
		let connection = Connection::new(stream, key);

		Ok(Sector::new(connection).await)
	}
}

impl State for Login {
	fn tick(&mut self) -> Option<AnyState> {
		if let Some(handle) = &mut self.login {
			if handle.is_finished() {
				match Handle::current().block_on(handle).unwrap() {
					Ok(sector) => return Some(AnyState::Sector(sector)),
					Err(error) => self.error = error.to_string(),
				}

				self.login = None;
			}
		}

		None
	}

	fn draw_ui(&mut self, cl_args: &ClArgs, context: &Context) {
		Window::new("Login")
			.anchor(Align2::CENTER_CENTER, (0.0, 0.0))
			.resizable(false)
			.collapsible(false)
			.auto_sized()
			.max_width(400.0)
			.enabled(self.login.is_none())
			.show(context, |window| {
				if !self.error.is_empty() {
					window.label(
						RichText::new(format!("Error: {}\n", &self.error)).color(Color32::RED),
					);
				}

				window.label("Email");
				window.add(Separator::default().spacing(4.0));
				window.add(
					TextEdit::singleline(&mut self.email)
						.desired_width(f32::INFINITY)
						.hint_text("name@example.com"),
				);
				window.label("");

				window.label("Password");
				window.add(Separator::default().spacing(4.0));
				window.add(
					TextEdit::singleline(&mut self.password)
						.desired_width(f32::INFINITY)
						.hint_text("correct horse battery staple")
						.password(true),
				);
				window.label("");

				window.allocate_ui_with_layout(
					Vec2 {
						x: window.min_rect().width(),
						y: 0.0,
					},
					Layout::left_to_right(Align::Center),
					|layout| {
						if self.login.is_some() {
							layout.spinner();
							layout.label("Connecting...");
						}

						layout.with_layout(Layout::right_to_left(Align::Center), |layout| {
							if layout.button("Login").clicked() {
								self.login = Some(Handle::current().spawn(Self::login(
									cl_args.clone(),
									self.email.clone(),
									self.password.clone(),
								)));
							}

							layout.hyperlink_to(
								"Create Account",
								"https://solarscape.astralchroma.dev/create_account",
							);
						});
					},
				);
			});
	}
}
