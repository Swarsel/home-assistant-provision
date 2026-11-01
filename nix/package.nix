{ lib, rustPlatform }:
let
  manifest = (lib.importTOML ../Cargo.toml).package;
in
rustPlatform.buildRustPackage {
  inherit (manifest) version;
  cargoLock.lockFile = ../Cargo.lock;
  pname = "home-assistant-provision";
  src = lib.fileset.toSource {
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../src
    ];
    root = ../.;
  };
  meta = {
    inherit (manifest) description;
    license = lib.licenses.mit;
    mainProgram = "home-assistant-provision";
  };
}
