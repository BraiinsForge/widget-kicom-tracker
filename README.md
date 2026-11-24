# Kicom Tracker

A Braiins Deck widget, in Czech, showing Kicom's current wagers and recent results from
[AOX.cz](https://www.aox.cz/kicomtracker/).

- Deck package: `widget-jj_kicom-tracker` (maintainer `jj`, set in `packaging.json`)
- Widget UID: `0ca26df2-ec61-462f-839c-7e20a44e37bb`
- API: <https://www.aox.cz/kicomtracker/api/v1/wagers.json>
  ([schema](https://www.aox.cz/kicomtracker/api/v1/schema.json)), `schemaVersion` 1 only

## Behaviour

Screen:

- one bet fills the screen; the title spans the full width, with status and deadline beneath
- left column: Kicom, conditions, stake; right column: opponents, status or resolution, indicators
- footer: Previous and Next buttons around the X/Y counter and "KICOM TRACKER data by AOX.cz"
- portraits load from the tracker and fall back to initials
- long text ends with an ellipsis at its line limit
- viewports from 317 × 238 to 1280 × 480; small ones keep title, deadline and participants

Rotation and data:

- bets rotate every 15 s (5–120 s); Previous/Next wrap around and restart the timer
- refresh every 5, 15, 30 or 60 minutes
- unresolved bets stay; confirmed results drop out after 0–30 days (default 3)
- a result's age counts from `closedAt`, else from when the widget first saw it closed;
  a restart does not reset it
- a deadline never decides a verdict, and a bet under review stays unresolved
- the last valid snapshot is saved and shown offline

## Install

On the Deck, open **Display Widgets → Install Widget** and pick the built `.nar`.

## Build

The widget builds against the [Deck widget SDK](https://github.com/BraiinsForge/bmc-main) of the 26.10
release (`bmcpkgs/stable-26.10`, at the revision `Cargo.lock` records).

With Nix:

```sh
nix build            # result/kicom-tracker-<version>.nar and .nar.json
nix flake check      # formatting, Clippy, tests and the NAR
```

Without Nix (Git, `rustup`, `python3` with `jsonschema`):

```sh
./scripts/build.sh   # target/nar/kicom-tracker-<version>.nar and .nar.json
./scripts/check.sh   # formatting, Clippy and tests
```

Updating the SDK:

1. change the branch in `Cargo.toml` and `flake.nix` if needed
2. `cargo update -p bmc-wasm-sdk` and `nix flake update bmc-main`

Packaging fails while the two locks name different revisions.

## Releasing an update

1. raise `version` in `manifest.json` and `Cargo.toml`, and the display name's ` v<version>` suffix
2. keep the `uid`
3. build and install the new `.nar`

## License

GPL-3.0-or-later, see [`COPYING`](COPYING).
