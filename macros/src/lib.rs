use proc_macro::TokenStream;

#[proc_macro]
pub fn protocol_version(_: TokenStream) -> TokenStream {
	env!("CARGO_PKG_VERSION_MAJOR")
		.parse()
		.expect("TokenStream containing a valid version number")
}
