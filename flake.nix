{
  description = "Solo: Rust / GPUI development and runtime environment";

  inputs = {
    # Keep Intel macOS available alongside Apple Silicon and the headless Linux core.
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-26.05-darwin";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { nixpkgs, rust-overlay, ... }:
    let
      systems = [
        "aarch64-darwin"
        "x86_64-darwin"
        "aarch64-linux"
        "x86_64-linux"
      ];
      forEachSystem =
        f:
        nixpkgs.lib.genAttrs systems (
          system:
          f (
            import nixpkgs {
              inherit system;
              overlays = [ rust-overlay.overlays.default ];
            }
          )
        );
    in
    {
      devShells = forEachSystem (
        pkgs:
        let
          rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          mkDevShell =
            extraPackages:
            pkgs.mkShell {
              packages = [
                rustToolchain
                pkgs.gitMinimal
                pkgs.ripgrep
                pkgs.pkg-config
                pkgs.nixfmt
              ]
              ++ extraPackages;

              # Bindgen needs the same libclang, C headers and SDK as the Nix linker.
              nativeBuildInputs = pkgs.lib.optionals pkgs.stdenv.hostPlatform.isDarwin [
                pkgs.rustPlatform.bindgenHook
              ];
              buildInputs = pkgs.lib.optionals pkgs.stdenv.hostPlatform.isDarwin [
                pkgs.apple-sdk
                pkgs.libiconv
              ];

              # cargo-fmt / cargo-clippy must not ask a host rustup to download a second toolchain.
              RUSTUP_TOOLCHAIN = "${rustToolchain}";
            };
        in
        rec {
          # Solo のモデル接続は Rust ライブラリを利用する。
          default = mkDevShell [ ];
          subscription = default;
          # CI と疑似シナリオも同じ toolchain を使う。
          minimal = mkDevShell [ ];
        }
      );

      formatter = forEachSystem (pkgs: pkgs.nixfmt);
    };
}
