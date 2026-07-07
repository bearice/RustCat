{
  description = "RustCat — an animated tray cat whose speed tracks CPU usage (Linux/KDE port)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    crane = {
      url = "github:ipetkov/crane";
    };
  };

  outputs = { self, nixpkgs, flake-utils, crane }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        # crane's toolchain derived from nixpkgs rust.
        craneLib = crane.mkLib pkgs;

        # Only Rust source matters for reproducibility; assets are read by
        # build.rs and embedded, so filtering to just the crate tree is fine.
        src = craneLib.path ./.;

        # trayicon's Linux/KDE backend is pure Rust (zbus + ico), so the build
        # needs no native GUI libraries. Git is required by build.rs to extract
        # the short commit hash shown in the About dialog.
        nativeBuildInputs = with pkgs; [
          git
          makeWrapper
        ];

        # Runtime helper tools so dialogs / system monitor work out of the box
        # on a KDE Plasma system. They are optional — the app degrades
        # gracefully if a tool is missing.
        runtimeDeps = with pkgs.kdePackages; [
          kdialog
          plasma-systemmonitor
        ];

        # nixpkgs' rustc-1.95 ships a broken *default* target spec: omitting the
        # target makes cargo refuse to build proc-macros ("target does not
        # support these crate types"). Explicitly setting CARGO_BUILD_TARGET
        # loads the proper target spec and the build succeeds. Harmless on
        # toolchains where the default already works.
        #
        # crane's buildDepsOnly runs `cargo check` and does not forward
        # `cargoBuildTarget` to that command, so we set CARGO_BUILD_TARGET as an
        # env var, which cargo reads directly (equivalent to --target).
        cargoBuildEnv = {
          CARGO_BUILD_TARGET = pkgs.stdenv.hostPlatform.config;
        };

        # Build dependencies only, then the real package, so that dependency
        # changes don't rebuild crates unnecessarily.
        cargoArtifacts = craneLib.buildDepsOnly {
          inherit src nativeBuildInputs;
          pname = "rustcat-deps";
          version = "0.0.0";
          env = cargoBuildEnv;
        };

        rustCat = craneLib.buildPackage {
          inherit src cargoArtifacts nativeBuildInputs;
          pname = "rustcat";
          version = "2.3.0";
          # No tests in this project.
          doCheck = false;
          env = cargoBuildEnv;

          postInstall = ''
            # Wrap so runtime helper tools are on PATH without polluting the
            # user's environment.
            wrapProgram $out/bin/rust_cat \
              --prefix PATH : ${pkgs.lib.makeBinPath runtimeDeps}

            # Desktop entry + icon for application launchers.
            install -Dm644 ${self}/assets/rustcat.desktop $out/share/applications/rustcat.desktop
            install -Dm644 ${self}/assets/appIcon.ico $out/share/icons/rustcat.ico
          '';
        };
      in
      {
        packages.default = rustCat;
        packages.rustcat = rustCat;

        devShells.default = pkgs.mkShell {
          inherit nativeBuildInputs;
          buildInputs = runtimeDeps ++ [
            pkgs.cargo
            pkgs.rustc
            pkgs.rust-analyzer
          ];
          RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
        };

        apps.default = {
          type = "app";
          program = "${rustCat}/bin/rust_cat";
        };
      });
}