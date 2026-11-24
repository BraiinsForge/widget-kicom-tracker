# Deck widget workflow

- The repository is the one widget: a Rust cdylib with `manifest.json` at the root.
- The SDK is a Git dependency on bmc-main's `bmcpkgs/stable-26.10` branch. The flake input must
  point at the same revision as `Cargo.lock`; `scripts/package.py` refuses to package otherwise.
- Build with `nix build` or `./scripts/build.sh`; check with `nix flake check` or `./scripts/check.sh`.
- `scripts/package.py` is the one packaging path for both. It follows upstream's
  `docs/uploading-wasm-widgets.md` and uses the launcher template verbatim.
- Always include the release version in the widget's display name. Increase the version for every
  delivered update while keeping its UID stable.
- The `nix build` captures render the packaged widget through software GL. They do not verify GPU
  output or installation on a physical Deck.
