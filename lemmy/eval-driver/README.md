Eval driver for Lemmy! For each controller, add `#[cfg_attr(feature = "{CONTROLLER-FEATURE-FLAG}", paralegal::analyze)]` and add the feature flag to the `Cargo.toml` in whatever crate you're analyzing. Then, follow the TODOs in `main.rs`. 

Then in `eval-driver`, run `cargo run` and enjoy.