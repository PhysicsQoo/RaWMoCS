# Python package release admission

Status: **candidate only / distribution not authorized**.

## Version and support contract

The package and binding are 0.3.0a1 / 0.3.0-alpha.1; the shared CLI is
explicitly pinned to 0.2.2 and the internal reference crate has its own
version. This is an intentional development identity, not a stable 0.3.0
release. A release must lock the intended version tuple and update the
existing source-release gate rather than reuse a 0.2.2 receipt.

The current binary qualification is CPython 3.12, manylinux_2_28 x86_64
and aarch64, and macOS x86_64 and arm64. No Windows Python or other
interpreter claim is made. Source metadata is not a test result; qualify
additional interpreters or narrow Requires-Python before publication.

## Candidate evidence

The existing Rust science workflow checks wheel metadata, ABI and
platform, native extension, RECORD coverage and SHA256, package source
identity and the extracted sdist. Candidate receipts explicitly deny
distribution and publication. These controls do not replace numerical,
failure, platform or independent witness checks. M1/M2 remain non-default,
and completed memory scouts are manual only.

## Redistribution gate (not passed)

The locked dependency graph contains rug 1.30.0 and gmp-mpfr-sys 1.7.1.
The latter ships GMP/MPFR source and is LGPL-3.0-or-later. GMP 6.3.0
offers LGPLv3/GPLv2 licensing; MPFR 4.2.2 is LGPLv3-or-later.
See the upstream [GMP copying conditions](https://gmplib.org/manual/Copying),
[MPFR manual](https://www.mpfr.org/mpfr-current/mpfr.html),
[gmp-mpfr-sys 1.7.1](https://docs.rs/crate/gmp-mpfr-sys/1.7.1),
and [LGPLv3](https://www.gnu.org/licenses/lgpl-3.0.html).
The project MIT license alone is not a binary redistribution clearance.

Before distributing wheels: inventory the resolved dependency licenses and
actual native linkage on each target, include required license/copyright
notices, provide the exact corresponding library source/build inputs and
a verified way to relink with modified libraries if required, and record
review of the resulting artifacts. Do not assume auditwheel or successful
installation establishes LGPL compliance. Review the distribution method
before publishing any binary or enabling PyPI upload.

## Release and publishing gate (not passed)

Retain the existing source release policy and exact-main evidence checks.
A versioned Python release policy must verify exact-main workflow/commit,
successful required jobs and tests, immutable artifact digests, explicit
version and platform inventory, distribution clearance, and reject
missing/duplicate/unexpected artifacts, wrong digests and skipped jobs.
Publish only the admitted bytes; never rebuild during publication.
Configure and verify the PyPI Trusted Publisher and protected environment
before enabling an OIDC upload. Do not create a release request from a
candidate receipt. S4 scalable WMC remains not achieved; changing its
version scope requires an explicit decision. QuPRS optional integration
is the next consumer admission; GPMC remains default.
