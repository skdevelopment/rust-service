//! Regenerates `src/bindings.ts` from the Rust command signatures.
//!
//! CI runs this and fails if the committed bindings are stale
//! (`git diff --exit-code src/bindings.ts`).

#[test]
fn export_bindings() {
    rustservice_lib::export_bindings();
}
