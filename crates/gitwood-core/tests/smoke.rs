//! Cargo compiles the standalone library alongside this integration smoke test.

#[test]
fn core_crate_is_available() {
    assert_eq!(env!("CARGO_PKG_NAME"), "gitwood-core");
}
