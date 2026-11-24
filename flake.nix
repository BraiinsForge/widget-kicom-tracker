{
  description = "Kicom Tracker, a Braiins Deck widget";

  # The manifest schema, the launcher template and the extractor come from this pin,
  # which must match Cargo.lock's SDK revision.
  inputs.bmc-main.url = "git+https://github.com/BraiinsForge/bmc-main?ref=bmcpkgs/stable-26.10&shallow=1";

  outputs = { self, bmc-main }:
    let
      lib = bmc-main.inputs.nixpkgs.lib;
      forAllSystems = lib.genAttrs [ "x86_64-linux" "aarch64-linux" ];
      perSystem = system:
        let
          # bmc-main's own package set, so the Rust toolchain is the one the SDK is built with.
          pkgs = bmc-main.legacyPackages.${system}.pkgs;
        in
        pkgs.callPackage ./nix/widget.nix {
          src = self;
          inherit bmc-main;
        };
    in
    {
      packages = forAllSystems (system: {
        default = (perSystem system).nar;
        inherit (perSystem system) nar;
      });

      checks = forAllSystems (system: {
        inherit (perSystem system) nar clippy fmt tests;
      });

      devShells = forAllSystems (system: {
        default = (perSystem system).devShell;
      });
    };
}
