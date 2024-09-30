{ lib, pkgs, ... }: {
	languages.rust = {
		enable = true;
		channel = "stable";
	};

	services.postgres = {
		enable = true;
		package = pkgs.postgresql_16;
		initialDatabases = [{ name = "solarscape"; }];
		listen_addresses = "127.0.0.1";
	};

	packages = with pkgs; [ openssl pkg-config sqlx-cli ];
	env.LD_LIBRARY_PATH = lib.makeLibraryPath (with pkgs; [ libxkbcommon vulkan-loader wayland ]);
}
