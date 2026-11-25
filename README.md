# Kicom Tracker

A Braiins Deck widget, in Czech, showing Kicom's current wagers and recent results from
[AOX.cz](https://www.aox.cz/kicomtracker/).

- Deck package: `widget-jj_kicom-tracker` (maintainer `jj`, set in `packaging.json`)
- Widget UID: `0ca26df2-ec61-462f-839c-7e20a44e37bb`
- API: <https://nexus.braiinsforge.com/api/v1/data/kicomtracker/wagers>, AOX.cz's feed in snake_case,
  `schema_version` 1 only; portraits from `/api/v1/data/kicomtracker/avatar/<name>.jpg`

## Behaviour

Screen:

- one bet fills the screen; the title spans the full width, with status and deadline beneath
- left column: Kicom, conditions, stake; right column: opponents, status or resolution, indicators
- footer: Previous and Next buttons around the X/Y counter and "KICOM TRACKER data by AOX.cz"
- portraits load from Nexus and fall back to initials
- long text ends with an ellipsis at its line limit
- viewports from 317 × 238 to 1280 × 480; small ones keep title, deadline and participants

Rotation and data:

- bets rotate every 15 s (5–120 s); Previous/Next wrap around and restart the timer
- refresh every 5, 15, 30 or 60 minutes
- unresolved bets stay; confirmed results drop out after 0–30 days (default 3)
- a result's age counts from `closed_at`, else from when the widget first saw it closed;
  a restart does not reset it
- a deadline never decides a verdict, and a bet under review stays unresolved
- the last valid snapshot is saved and shown offline

## Install

- release: the `.nar` from the [`latest` release](../../releases/tag/latest), rebuilt from every `master` commit
- branch or pull request: the `kicom-tracker` artifact of its run on the [Actions](../../actions) page
- on the Deck: **Display Widgets → Install Widget**, then pick the file
- package server: the `latest` release is also a package index and Nix binary cache.
  As root on the Deck, register it once; the widget then appears among the installable widgets,
  and upgrades come with each release:

  ```sh
  base=https://github.com/BraiinsForge/widget-kicom-tracker/releases/download/latest
  key=downloads.braiinsforge.com-1:4XDOIc61MHtHeIOVrgNOfgzZHt4RCPfqWGDt5PwsLeU=
  bmc-nix-cli register-server --id kicom-tracker --optional \
    --index-url $base/nix-package-index.v1.json --index-public-key $key \
    --cache-url $base --cache-public-key $key
  ```

  The store path is content-addressed, so Nix checks it against its own hash and needs no key.
  The command requires one with `--cache-url`; the Deck's own key adds no trust.

## Build

The widget builds against the [Deck widget SDK](https://github.com/BraiinsForge/bmc-main) of the 26.10
release (`bmcpkgs/stable-26.10`, at the revision `Cargo.lock` records).

With Nix:

```sh
nix build            # result/kicom-tracker-<version>.nar and .nar.json
nix build .#captures # PNG captures on every supported target; not part of CI
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
3. merge to `master`; CI publishes the build

## Captures

- `capture/` drives bmc-main's `wasm-capture`
- its fixture replays `tests/fixtures/wagers-v1.json`; regenerate it with `./scripts/capture-fixture.py`
- captures render through software GL, so they verify neither a physical Deck nor its GPU

## License

GPL-3.0-or-later, see [`COPYING`](COPYING).
