// TODO: restore `test` here once the SDK is past 26.10;
// its JSON and date parsers exist only on wasm32, and it has no wasm test runner.
#[cfg(target_arch = "wasm32")]
mod feed;
#[cfg(target_arch = "wasm32")]
mod live;
#[cfg(any(target_arch = "wasm32", test))]
mod model;
#[cfg(target_arch = "wasm32")]
mod portraits;
