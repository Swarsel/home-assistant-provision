{
  description = "home-assistant-provision";

  inputs = {
    flake-parts = {
      inputs.nixpkgs-lib.follows = "nixpkgs";
      url = "github:hercules-ci/flake-parts";
    };

    git-hooks-nix = {
      inputs.nixpkgs.follows = "nixpkgs";
      url = "github:cachix/git-hooks.nix";
    };

    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    pedantix = {
      inputs.nixpkgs.follows = "nixpkgs";
      url = "github:Swarsel/pedantix";
    };

    treefmt-nix = {
      inputs.nixpkgs.follows = "nixpkgs";
      url = "github:numtide/treefmt-nix";
    };
  };

  outputs =
    inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      flake.overlays.default = final: _prev: {
        home-assistant-provision = final.callPackage ./nix/package.nix { };
      };
      imports = [
        inputs.treefmt-nix.flakeModule
        inputs.git-hooks-nix.flakeModule
        inputs.pedantix.flakeModules.default
      ];
      perSystem =
        {
          config,
          pkgs,
          self',
          ...
        }:
        {
          apps = rec {
            default = home-assistant-provision;
            home-assistant-provision = {
              program = self'.packages.home-assistant-provision;
              meta.description = self'.packages.home-assistant-provision.meta.description;
            };
          };
          checks.package = self'.packages.home-assistant-provision;
          devShells.default = pkgs.mkShell {
            inputsFrom = [
              self'.packages.home-assistant-provision
              config.treefmt.build.devShell
            ];
            packages = with pkgs; [
              cargo
              clippy
              rust-analyzer
              rustc
              rustfmt
            ];
            shellHook = config.pre-commit.installationScript;
          };
          packages = rec {
            default = home-assistant-provision;
            home-assistant-provision = pkgs.callPackage ./nix/package.nix { };
          };
          pre-commit.settings = {
            hooks = {
              clippy = {
                enable = true;
                packageOverrides.cargo = pkgs.symlinkJoin {
                  name = "cargo-with-cc";
                  paths = [
                    pkgs.cargo
                    pkgs.stdenv.cc
                  ];
                };
              };
              deadnix.enable = true;
              statix.enable = true;
              treefmt.enable = true;
            };
            settings.rust = {
              cargoManifestPath = "./Cargo.toml";
              check.cargoDeps = pkgs.rustPlatform.importCargoLock { lockFile = ./Cargo.lock; };
            };
          };
          treefmt = {
            programs = {
              pedantix.enable = true;
              rustfmt.enable = true;
              taplo.enable = true;
            };
            projectRootFile = "flake.nix";
          };
        };
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
    };
}
