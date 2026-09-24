# SpiderNet Nix flake — builds the Rust workspace in server/ and exposes
# one `spidernet` package containing all four binaries (sn-node, sn-exit,
# sn-fetch, sn-fair) in $out/bin, plus an experimental NixOS module.
#
# NO WARRANTY — SpiderNet is provided "as is" without warranty of any kind
# (AGPL-3.0, sections 15/16). Use is entirely at the operator's own
# responsibility.
#
# !!! NOT VERIFIED ON THIS MACHINE !!!
# Nix is not installed in the environment where this flake was written.
# `nix flake check` and `nix build` have NOT been run — treat everything
# below as best-effort and report build failures as issues.
#
# Usage (once Nix >= 2.19 is installed):
#   nix build .                       # -> result/bin/{sn-node,sn-exit,...}
#   nix shell .# -c sn-fair --capacity 100 --participant 1=contributing
#
# The nixpkgs input is pinned to the current stable NixOS release branch
# (nixos-26.05, released 2026-05-30; nixos-25.11 is already deprecated).

{
  description =
    "SpiderNet: pooled-bandwidth neighborhood network node software";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
  };

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          inherit (pkgs) lib;

          # Only the files cargo needs; keeps target/, docs and any other
          # clutter out of the Nix store hash.
          src = lib.cleanSourceWith {
            src = lib.cleanSource ./server;
            filter =
              path: type:
              # Prune cargo build dirs entirely.
              !(type == "directory" && lib.hasSuffix "/target" path)
              && (
                type == "directory"
                || lib.hasSuffix ".rs" path
                || lib.hasSuffix ".toml" path
                || lib.hasSuffix ".lock" path
              );
          };

          spidernet = pkgs.rustPlatform.buildRustPackage {
            pname = "spidernet";
            version = "0.1.0";

            inherit src;
            cargoLock.lockFile = ./server/Cargo.lock;

            # Integration tests talk to real servers; skip them when
            # packaging. Run `cargo test` inside server/ for development.
            doCheck = false;

            meta = {
              description =
                "SpiderNet node software (sn-node, sn-exit, sn-fetch, sn-fair)";
              longDescription = ''
                SpiderNet pools the bandwidth of household internet
                connections over a neighborhood mesh (Yggdrasil overlay).
                NO WARRANTY — AGPL-3.0, sections 15/16.
              '';
              license = lib.licenses.agpl3Only;
              mainProgram = "sn-node";
              platforms = lib.platforms.unix;
            };
          };
        in
        {
          inherit spidernet;
          default = spidernet;
        }
      );

      # ------------------------------------------------------------------
      # EXPERIMENTAL NixOS module — never built or activated anywhere.
      # Runs `sn-node daemon` (which manages the yggdrasil sidecar) as a
      # hardened systemd service.
      #
      # Usage example (in a NixOS configuration that imports this flake):
      #
      #   imports = [ spidernet.nixosModules.default ];
      #
      #   services.spidernet = {
      #     enable = true;
      #     configFile = "/etc/spidernet/node.toml";
      #     # stateDir defaults to "spidernet" -> /var/lib/spidernet,
      #     # which the example config uses as state_dir.
      #   };
      #
      # The config file must set state_dir = "/var/lib/spidernet" (or
      # leave the default and rely on WorkingDirectory below).
      # ------------------------------------------------------------------
      nixosModules.default =
        {
          config,
          lib,
          pkgs,
          ...
        }:
        let
          cfg = config.services.spidernet;
        in
        {
          options.services.spidernet = {
            enable = lib.mkEnableOption "SpiderNet node daemon (sn-node)";

            package = lib.mkOption {
              type = lib.types.package;
              # The flake package of the evaluating system; can be
              # overridden with any build of spidernet.
              default = self.packages.${pkgs.system}.default;
              defaultText = lib.literalExpression "spidernet.packages.\${pkgs.system}.default";
              description = "SpiderNet package to run (contains sn-node etc.).";
            };

            configFile = lib.mkOption {
              type = lib.types.path;
              description = ''
                Path to the node.toml config file read by
                `sn-node daemon --config`.
              '';
            };

            stateDir = lib.mkOption {
              type = lib.types.str;
              default = "spidernet";
              description = ''
                State directory (systemd StateDirectory), holding node.key
                and the generated yggdrasil.conf. Created as
                /var/lib/<stateDir>.
              '';
            };
          };

          config = lib.mkIf cfg.enable {
            systemd.services.spidernet = {
              description = "SpiderNet node (Yggdrasil overlay, optional exit)";
              wantedBy = [ "multi-user.target" ];
              after = [ "network-online.target" ];
              wants = [ "network-online.target" ];

              # The daemon spawns yggdrasil (manage = true by default).
              path = [ pkgs.yggdrasil ];

              serviceConfig = {
                ExecStart =
                  "${lib.getExe cfg.package} daemon --config ${cfg.configFile}";
                StateDirectory = cfg.stateDir;
                WorkingDirectory = "/var/lib/${cfg.stateDir}";

                # TUN setup requires CAP_NET_ADMIN; nothing more.
                AmbientCapabilities = [ "CAP_NET_ADMIN" ];
                CapabilityBoundingSet = [ "CAP_NET_ADMIN" ];
                DeviceAllow = [
                  "/dev/net/tun rw"
                  # Yggdrasil TUN devices are created at runtime.
                  "char-tun rwm"
                ];

                Restart = "on-failure";
                RestartSec = 5;
                NoNewPrivileges = true;
                ProtectSystem = "strict";
                ProtectHome = true;
                PrivateTmp = true;
                # State dir must stay writable.
                ReadWritePaths = [ "/var/lib/${cfg.stateDir}" ];
              };
            };
          };
        };
    };
}
