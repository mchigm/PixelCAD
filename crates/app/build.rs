fn main() {
    // Debug info is required by `i-slint-backend-testing`'s `ElementHandle`
    // API (used by the headless GUI tests in `src/main.rs`). It has no
    // runtime cost for the shipped binary beyond a slightly larger compiled
    // artifact, so it is always enabled rather than gated on `cfg(test)`
    // (build scripts do not see the profile being tested).
    let config = slint_build::CompilerConfiguration::new().with_debug_info(true);
    slint_build::compile_with_config("ui/main.slint", config).unwrap();
}
