# Stable release automation

Use the single stable admission and OIDC publication path. Run the read-only Stable package admission workflow on release/vX.Y.Z (or dispatch on main), pin the exact revision, run ID, crate/source artifact IDs and ZIP SHA256 digests in stable-request.json, and provide releases/vX.Y.Z.md. Do not substitute rebuilt artifacts without matching the admitted bytes. Only the request may change after admission.

The source-release workflow runs on main with explicit [publish-stable] intent or stable-trusted dispatch. stable-preflight is read-only. A short-lived OIDC token is used only in the registry job; it reproduces the exact admitted crate, verifies registry download bytes and stops on conflicts. The publication job verifies the registry, exact Git tag, CI-created draft, all assets, and final release identity. No RC bootstrap or version-specific recovery path remains. Historical versions remain available at their tags; their old workflows are not current publication procedures.

WASM is a target of the same Rust solver. A WASM-bearing release requires separately verified same-revision artifacts and source/license admission in the stable release contract; no temporary CI artifact is a public release.
