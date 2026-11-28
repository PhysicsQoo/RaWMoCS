# First rawmocs crates.io publication (v0.4.0)

Only `rawmocs` is public. Never publish `rawmocs-core`, `rawmocs-cli` or
`rawmocs-python` to crates.io.

1. Merge the validated candidate to main, with explicit `[release-v040]`
   intent or manually dispatch the existing Rust crate rehearsal on main.
2. Pin the exact main revision, successful admission run, all three artifact
   IDs/digests and source/sdist/crate SHA256 in releases/request.json using
   qualification `v040-single-crate-source-only`. Do not reuse a branch run.
3. Verify the existing source-release workflow read-only admission. Create
   GitHub environment `crates-io`, restrict it to main and require maintainer
   approval where supported. Obtain a short-lived crates.io token allowing
   creation of the new crate. Store it only in the environment secret
   `CARGO_REGISTRY_TOKEN`, never in a chat, commit, or log.
4. Explicitly dispatch source-release.yml on main with registry_mode=bootstrap.
   It refuses nonmatching repackaged bytes, conflicting names or missing evidence,
   uploads only the single package, and checks registry index/download SHA256.
   The GitHub Release is gated on the matching registry checksum.
5. After first publication, revoke the token and remove the environment secret.
   In https://crates.io/crates/rawmocs/settings configure Trusted Publishing:
   owner PhysicsQoo, repository RaWMoCS, workflow source-release.yml,
   environment crates-io. Dispatch registry_mode=trusted to verify OIDC and
   registry bytes without republishing v0.4.0. A later version must extend the
   versioned release admission; the v0.4.0 gate cannot authorize other versions.

The source-only policy does not publish linked wheels or upload to PyPI.
