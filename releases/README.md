# Stable release automation

Use the single stable admission and OIDC publication path. Run the read-only Stable package admission workflow on release/vX.Y.Z (or dispatch on main), pin the exact revision, run ID, crate/source artifact IDs and ZIP SHA256 digests in stable-request.json, and provide releases/vX.Y.Z.md. Do not substitute rebuilt artifacts without matching the admitted bytes. Only the request may change after admission.

The source-release workflow runs on main with explicit [publish-stable] intent or stable-trusted dispatch. stable-preflight is read-only. A short-lived OIDC token is used only in the registry job; it reproduces the exact admitted crate, verifies registry download bytes and stops on conflicts. The publication job verifies the registry, exact Git tag, CI-created draft, all assets, and final release identity. No RC bootstrap or version-specific recovery path remains. Historical versions remain available at their tags; their old workflows are not current publication procedures.

WASM is a target of the same Rust solver. A WASM-bearing release requires separately verified same-revision artifacts and source/license admission in the stable release contract; no temporary CI artifact is a public release.

## Same-version WASM assets

When the source contains the WASM build recipe, the release request must include a wasm object with run, id, digest and review_sha256. The run must be a successful manual Rust science wasm admission on the exact release revision (main or release/vX.Y.Z), and the artifact is rawmocs-wasm-candidate. Its second clean build must have replayed from vendored corresponding source offline. Add the review JSON at releases/vX.Y.Z-wasm-review.json after admission; this is an explicit human decision, not an automated legal assertion. It must include status APPROVED_FOR_REDISTRIBUTION, version, revision, candidate_digest, nonempty review_reference and scope binary-corresponding-source-licenses. Only this review and the immutable request may differ from the admitted source. Never create an approval receipt merely because CI passed.

The existing stable publisher validates the exact artifact and produces rawmocs-X.Y.Z-wasm.zip, rawmocs-X.Y.Z-wasm-source.zip, wasm-provenance.json, wasm-review.json and wasm-SHA256SUMS. The normal crate/source assets remain unchanged. The Playground pin is set only after official release readback, not against a transient Actions artifact.
