{ lib
, stdenv
, runCommand
, makeRustPlatform
, mkShell
, ii
, python3
, nix
, git
, src
, bmc-main
, wasm-capture
}:
let
  name = "kicom-tracker";
  manifest = lib.importJSON ../manifest.json;
  toolchain = ii.rust.toolchain;
  rustPlatform = makeRustPlatform { cargo = toolchain; rustc = toolchain; };
  python = python3.withPackages (ps: [ ps.jsonschema ]);

  # Built alone from bmc-main's lock: its workspace build would also compile
  # every other crate's dependencies and mesa, several gigabytes the widget never uses.
  wasm-assets = rustPlatform.buildRustPackage {
    pname = "bmc-wasm-assets";
    version = bmc-main.shortRev;
    src = bmc-main;
    cargoLock = {
      lockFile = "${bmc-main}/Cargo.lock";
      allowBuiltinFetchGit = true;
    };
    cargoBuildFlags = [ "-p" "bmc-wasm-assets" ];
    doCheck = false;
  };

  mkCargo = pname: { nativeBuildInputs ? [ ], buildPhase, installPhase ? "touch $out" }:
    stdenv.mkDerivation {
      inherit pname src buildPhase installPhase;
      version = manifest.version;
      cargoDeps = rustPlatform.importCargoLock {
        lockFile = ../Cargo.lock;
        # One hash covers every crate Cargo.lock takes from the same bmc-main revision.
        outputHashes."bmc-wasm-sdk-0.1.0" = "sha256-oR3SLPIkv9JMnQzlyHof4CfLMwnBhqnKxvbVK9Ryax8=";
      };
      nativeBuildInputs = [ rustPlatform.cargoSetupHook toolchain ] ++ nativeBuildInputs;
    };

  nar = mkCargo "${name}-nar" {
    nativeBuildInputs = [ python nix ];
    buildPhase = ''
      cargo build --locked --offline --release --target wasm32-unknown-unknown
      release=target/wasm32-unknown-unknown/release
      mkdir -p $out
      python3 scripts/package.py \
        --wasm $release/${lib.replaceStrings [ "-" ] [ "_" ] name}.wasm \
        --artifact-root $release/deps \
        --extractor ${wasm-assets}/bin/bmc-wasm-assets \
        --upstream ${bmc-main} \
        --upstream-revision ${bmc-main.rev} \
        --out $out

      # package.py writes the NAR itself; Nix must read back exactly what it would have dumped.
      artifact=$out/${name}-${manifest.version}.nar
      nix-store --restore restored < $artifact
      nix-store --dump restored | cmp - $artifact
    '';
    installPhase = "";
  };

  captures = runCommand "${name}-captures" { nativeBuildInputs = [ wasm-capture nix ]; } ''
    nix-store --restore package < ${nar}/${name}-${manifest.version}.nar
    widget=$(echo package/lib/bmc-widgets/*/${name})
    wasm-capture run "$widget/lib/wasm/${name}.wasm" \
      --asset-root "$widget/lib/assets" \
      --capture-dir ${src}/capture \
      --all-targets \
      --output $out
  '';
in
{
  inherit nar captures;

  fmt = mkCargo "${name}-fmt" { buildPhase = "cargo fmt -- --check"; };

  clippy = mkCargo "${name}-clippy" {
    buildPhase = "cargo clippy --locked --offline --target wasm32-unknown-unknown -- -D warnings";
  };

  tests = mkCargo "${name}-tests" { buildPhase = "cargo test --locked --offline --lib"; };

  devShell = mkShell { packages = [ toolchain python git ]; };
}
