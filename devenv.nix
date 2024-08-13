{ lib, pkgs, ... }: {
	languages.rust = {
		enable = true;
		channel = "stable";
	};

	env.LD_LIBRARY_PATH = lib.makeLibraryPath (with pkgs; [
		vulkan-loader
	]);
}
