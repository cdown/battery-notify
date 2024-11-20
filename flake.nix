{
  description = "Simple rust project";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    naersk.url = "github:nix-community/naersk/master";
    utils.url = "github:numtide/flake-utils";
  };

  outputs =
    inputs@{ self
    , nixpkgs
    , utils
    , naersk
    , ...
    }: utils.lib.eachDefaultSystem (system:
    let
      pkgs = import nixpkgs { inherit system; };

      buildInputs = with pkgs; [ ]; # dependencies here

      mkRustProject = (conf: ((pkgs.callPackage naersk { }).buildPackage ({
        src = ./.;
        inherit buildInputs;
      } // conf)));
    in
    {
      packages.default = mkRustProject {};
      packages.clippy = mkRustProject { mode = "clippy"; };
      packages.check = mkRustProject { mode = "check"; };

      devShell = with pkgs; mkShell {
        buildInputs = buildInputs ++ [
          rustc
          cargo
          rust-analyzer
          rustfmt
          clippy
        ];
        RUST_SRC_PATH = "${rustPlatform.rustLibSrc}";
      };
    });
}
