# Source release workflow

A reviewed change to `releases/request.json` on main explicitly requests publication. The workflow verifies the nominated immutable solver revision, full Science and CodSpeed runs on that revision, required job/step conclusions, artifact provenance, archive SHA256 and installation receipt. PRs perform read-only preflight; only main push or main workflow_dispatch can publish. A draft is created first, then promoted after asset readback. No solver binaries are redistributed.

The first request targets the already admitted solver commit, not the later CI-only commit. Future releases update the version, exact admitted revision, run IDs, artifact ID/digests and notes. When the scientific gate or source package version changes, update the admission script and existing CI accordingly. Do not use old runs for a new solver revision. A failed or partial publication is not silently overwritten: inspect the existing tag/draft and assets before any explicit recovery. Manual dispatch retries a failed preflight with the same request; it is not a permission bypass.

The release request is not a scientific result; pinned CI evidence is. Do not add private tracking IDs.
