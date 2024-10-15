fn main() {
	println!("cargo::rustc-check-cfg=cfg(debug)");
	println!("cargo::rerun-if-changed=../migrations");

	if let Ok(profile) = std::env::var("PROFILE") {
		if profile == "debug" {
			println!("cargo::rustc-cfg=debug");
		}
	}
}
