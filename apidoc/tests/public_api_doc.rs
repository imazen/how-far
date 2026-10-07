//! Public-API surface snapshots for the PARENT workspace (docs/public-api/).
//! Shared implementation + format docs: the `zenutils-apidoc` crate.
//!
//! Published crates only — the `examples/how-far-*` members are internal
//! (`publish = false`) and carry no snapshot.
#[test]
fn public_api_surface_docs_are_current() {
    zenutils_apidoc::ApiDoc::new()
        .workspace_dir("..")
        .crates(["how-far", "how-far-along", "how-far-really", "how-far-interpolate"])
        .run();
}
