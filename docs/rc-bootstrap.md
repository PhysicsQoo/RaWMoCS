# RC bootstrap for the single rawmocs public crate

The initial registry publication is `rawmocs 0.4.0-rc.1`, not stable 0.4.0.
This prerelease is permanent in the registry even if yanked later.
Stable Rust, CLI and Python release versions and releases/request.json are unchanged.
The pinned base is exact-main 273f45b83c55b28871350c32ba5070bf77928688, CI 46114420248.
The RC package must differ from the admitted stable crate only in Cargo package
version and corresponding root lock entry. No solver, dependency, Python or
other package upload is authorized by RC modes.

In source-release.yml on main:
- rc-preflight: read-only package and tests, no token.
- rc-bootstrap: reproduce exact RC bytes, use the crates-io environment secret
  CARGO_REGISTRY_TOKEN only for the first upload, validate registry SHA256 and
  an independent registry consumer. No stable or GitHub release.
- rc-trusted: after revoking/removing the bootstrap token and configuring the
  crate's Trusted Publisher (PhysicsQoo/RaWMoCS, source-release.yml, crates-io),
  obtain an OIDC token and verify RC bytes, no second upload.

Stable 0.4.0 remains blocked until a separate stable admission request and
explicit trusted publication support for a previously registered RC crate.

## Stable publication recovery and future freeze

The stable 0.4.0 crate was published and registry checksum verified in run 46123201552. GitHub draft creation returned HTTP 403 twice. The source revision and crate bytes must remain unchanged. A maintainer may create only tag `v0.4.0` at `273f45b83c55b28871350c32ba5070bf77928688` after confirming no tag or release exists. The publisher accepts this exact tag only for the pinned 0.4.0 registry checksum and uses `--verify-tag`. Never force, move or delete a tag. Once the registry is MATCH, the registry upload job is skipped; explicit trusted dispatch or the main publication marker resumes GitHub only. No new crate upload or permanent credential is required.

For subsequent releases, finish release engineering before exact-main source admission. After admission modify only the release request and notes. Any workflow tree differences block a fresh registry upload before credentials and again immediately before upload; new GitHub release creation also checks consistency. A differing workflow tree requires a new admission, never a retag of already published bytes. The orphan-tag exception is restricted to the single immutable 0.4.0 identity. Existing verified drafts and published releases retain their previous strict checks.
