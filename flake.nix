{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-24.11";
  };

  outputs =
    { self
    , nixpkgs
    , ...
    }@inputs:
    let
      supportedSystems = [ "aarch64-darwin" "x86_64-darwin" "x86_64-linux" "aarch64-linux" ];
      forAllSystems = f: nixpkgs.lib.genAttrs supportedSystems (system: f system);
    in
    {
      devShell = forAllSystems (system: inputs.self.devShells.${system}.default);
      devShells = forAllSystems
        (system:
          let
            pkgs = import nixpkgs {
              inherit system;
            };
          in
          {
            default = pkgs.mkShell {
              name = "gh-event-forwarder";
              nativeBuildInputs = with pkgs; [
                nix # so in --pure mode we actually find the "correct" nix
                bash
                nix-prefetch-git
                rustc
                cargo
                clippy
                rustfmt
                pkg-config
                git
              ];
              buildInputs = with pkgs; [
              ] ++ lib.optionals stdenv.isDarwin [ darwin.Security libiconv ];

              postHook = ''
                checkPhase() (
                    cd "${builtins.toString ./.}/ofborg"
                    set -x
                    cargo fmt
                    git diff --exit-code
                    cargofmtexit=$?

                    cargo clippy
                    cargoclippyexit=$?

                    cargo build && cargo test
                    cargotestexit=$?

                    sum=$((cargofmtexit + cargoclippyexit + cargotestexit))
                    exit $sum
                )
              '';

              RUSTFLAGS = "-D warnings";
              RUST_BACKTRACE = "1";
              RUST_LOG = "ofborg=debug";
              NIX_PATH = "nixpkgs=${pkgs.path}";
            };
          });

      packages = forAllSystems (system:
        let
          pkgs = import nixpkgs {
            inherit system;
          };

          pkg = pkgs.rustPlatform.buildRustPackage {
            name = "ofborg";
            src = pkgs.nix-gitignore.gitignoreSource [ ] ./.;

            nativeBuildInputs = with pkgs; [
              pkg-config
              pkgs.rustPackages.clippy
            ];

            buildInputs = with pkgs; [
            ] ++ lib.optionals pkgs.stdenv.isDarwin (with pkgs; [
              darwin.apple_sdk.frameworks.Security
              darwin.apple_sdk.frameworks.CoreFoundation
            ]);

            preBuild = ''
              cargo clippy
            '';

            doCheck = false; # Tests require access to a /nix/ and a nix daemon
            checkInputs = with pkgs; [
              nix
            ];

            cargoLock = {
              lockFile = ./Cargo.lock;
              outputHashes = {
                "hubcaps-0.6.2" = "sha256-yyHOCxUsehvtYfttRY4T9TDrJhSKGpJRa/SX3Sd1TNc=";
              };
            };
          };

        in
        {
          inherit pkg;

          ofborg.rs = pkgs.runCommand "ofborg-rs-symlink-compat" { src = pkg; } ''
            mkdir -p $out/bin
            for f in $(find $src -type f); do
              bn=$(basename "$f")
              ln -s "$f" "$out/bin/$bn"

              # Rust 1.n? or Cargo  starting outputting bins with dashes
              # instead of underscores ... breaking all the callers.
              if echo "$bn" | grep -q "-"; then
                ln -s "$f" "$out/bin/$(echo "$bn" | tr '-' '_')"
              fi
            done

            test -e $out/bin/builder
            test -e $out/bin/github_comment_filter
            test -e $out/bin/github_comment_poster
            test -e $out/bin/github_webhook_receiver
            test -e $out/bin/log_message_collector
            test -e $out/bin/evaluation_filter
          '';
        });

      hydraJobs = {
        buildRs = forAllSystems (system: self.packages.${system}.ofborg.rs);
      };
    };
}
