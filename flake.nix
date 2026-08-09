{
  description = "netchecker — cross-platform internet reachability checker";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
        # Read the version from Cargo.toml so this never drifts out of sync.
        cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "netchecker";
          version = cargoToml.package.version;
          src = ./.;

          # Use the committed lockfile; no vendor hash to maintain.
          cargoLock.lockFile = ./Cargo.lock;

          # rustls (ring) needs no OpenSSL. macOS needs the system frameworks
          # that netdev / SystemConfiguration link against.
          buildInputs = pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.darwin.apple_sdk.frameworks.Security
            pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
          ];

          meta = with pkgs.lib; {
            description = "Cross-platform internet reachability checker";
            homepage = "https://github.com/pourmand1376/netchecker";
            license = licenses.mit;
            mainProgram = "netchecker";
          };
        };

        # `nix run` / `nix run github:pourmand1376/netchecker`
        apps.default = flake-utils.lib.mkApp {
          drv = self.packages.${system}.default;
        };
      }
    );
}
