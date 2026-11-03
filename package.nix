# TODO: move this to nixpkgs
# This file aims to be a replacement for the nixpkgs derivation.

{
  buildFeatures ? [ ],
  buildNoDefaultFeatures ? false,
  buildPackages,
  fetchFromGitHub,
  lib,
  rustPlatform,
}:

rustPlatform.buildRustPackage {
  inherit buildNoDefaultFeatures buildFeatures;

  pname = "mime-rs";
  version = "0.1.0";
  cargoHash = "";

  src = fetchFromGitHub {
    owner = "pimalaya";
    repo = "mime";
    rev = "v0.1.0";
    hash = "";
  };

  meta = {
    description = "MIME message parser, validator, editor and builder library for Rust";
    homepage = "https://github.com/pimalaya/mime";
    changelog = "https://github.com/pimalaya/mime/blob/master/CHANGELOG.md";
    license = [
      lib.licenses.mit
      lib.licenses.asl20
    ];
    maintainers = with lib.maintainers; [ soywod ];
  };
}
