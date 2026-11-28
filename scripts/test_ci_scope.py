import unittest
from pathlib import Path
from ci_scope import classify, wasm_recipe_changed, playground_changed

def row(path, status="M", old="100644", new="100644"):
    return (f":{old} {new} {'a'*40} {'b'*40} {status}\0{path}\0").encode()

class ScopeTests(unittest.TestCase):
    def test_docs(self):
        r = classify(row("README.md"))
        self.assertEqual(r["scope"], "DOCS")
        self.assertFalse(r["python_contract"])
    def test_site_only_does_not_request_native(self):
        for p in ("playground/release.json", "playground/index.html", "playground/app.js",
                  "playground/style.css", "scripts/playground/assemble.py",
                  "scripts/playground/smoke.py", "scripts/playground/test_assemble.py"):
            r = classify(row(p))
            self.assertEqual(r["scope"], "SITE")
            self.assertFalse(r["native_contract"])
            self.assertFalse(r["python_contract"])
            self.assertTrue(playground_changed(r))
        for p in ("README.md", "playground/README.md"):
            self.assertFalse(classify(row(p))["native_contract"])

    def test_mixed_or_nonregular_site_requires_native(self):
        for raw in (row("playground/app.js") + row("crates/rawmocs-core/src/lib.rs"),
                    row("playground/unknown.js"),
                    row("playground/app.js", "T", "100644", "120000"),
                    row("playground/app.js", "D", "100644", "000000"),
                    row("scripts/playground/unknown.py")):
            r = classify(raw)
            self.assertTrue(r["native_contract"])
            self.assertTrue(r["python_contract"])
            self.assertEqual(r["scope"], "CORE_OR_UNKNOWN")

    def test_package(self):
        r = classify(row("pyproject.toml"))
        self.assertEqual(r["scope"], "PACKAGE")
        self.assertTrue(r["python_contract"])
        self.assertTrue(r["native_contract"])
        self.assertFalse(r["release_authorized"])
    def test_binding(self):
        r = classify(row("crates/rawmocs-python/src/lib.rs"))
        self.assertEqual(r["scope"], "BINDING")
        self.assertTrue(r["full_science_required_before_release"])
    def test_core_and_unknown(self):
        for p in ("Cargo.toml", "crates/rawmocs-cli/Cargo.lock", "crates/rawmocs-cli/src/lib.rs",
                  "scripts/release_source.py", "docs/unknown.md", "python/unknown.py",
                  ".github/workflows/rust-science.yml", "releases/request.json"):
            self.assertEqual(classify(row(p))["scope"], "CORE_OR_UNKNOWN")
    def test_mixed(self):
        r = classify(row("pyproject.toml")+row("crates/rawmocs-cli/src/main.rs"))
        self.assertEqual(r["scope"], "CORE_OR_UNKNOWN")
    def test_modes(self):
        for old, new, status in (("100644","100755","M"), ("100644","120000","T"),
                                 ("100644","000000","D")):
            self.assertEqual(classify(row("pyproject.toml", status, old, new))["scope"], "CORE_OR_UNKNOWN")
    def test_malformed(self):
        for raw in (row("README.md")[:-1], row("README.md")*2, b"bad\0x\0"):
            with self.assertRaises(ValueError):
                classify(raw)
    def test_wasm_recipe_gate(self):
        for p in ('scripts/wasm/build.py', '.github/actions/wasm-candidate/action.yml', 'scripts/stage_single_rust_crate.py', '.github/workflows/rust-science.yml', 'scripts/ci_scope.py', 'crates/rawmocs-cli/src/general.rs', 'crates/rawmocs-core/src/lib.rs', 'packaging/rawmocs/Cargo.lock'):
            self.assertTrue(wasm_recipe_changed(classify(row(p))))
        for p in ('README.md', 'playground/app.js', 'crates/rawmocs-python/src/lib.rs'):
            self.assertFalse(wasm_recipe_changed(classify(row(p))))
        self.assertTrue(wasm_recipe_changed({'reason':'unknown diff','paths':{}}))

    def test_playground_change_gate(self):
        for p in ("playground/release.json", "playground/index.html", "playground/style.css",
                  "playground/app.js", "scripts/playground/assemble.py", "scripts/playground/smoke.py",
                  "scripts/wasm_release.py", "scripts/release_source.py",
                  "scripts/ci_scope.py", ".github/workflows/rust-science.yml"):
            self.assertTrue(playground_changed(classify(row(p))), p)
        for p in ("README.md", "playground/README.md", "crates/rawmocs-cli/src/general.rs",
                  "packaging/rawmocs/Cargo.lock", "releases/stable-request.json"):
            self.assertFalse(playground_changed(classify(row(p))), p)
        self.assertTrue(playground_changed({"reason": "unknown diff", "paths": {}}))

    def test_empty_no_authority(self):
        self.assertFalse(classify(b"")["release_authorized"])


class ManualLongCI(unittest.TestCase):
    root = Path(__file__).resolve().parents[1] / ".github" / "workflows"

    def job_guard(self, workflow, job):
        lines = (self.root / workflow).read_text().splitlines()
        start = lines.index("  " + job + ":") + 1
        end = next((i for i in range(start, len(lines))
                    if lines[i].startswith("  ") and not lines[i].startswith("    ")
                    and lines[i].endswith(":")), len(lines))
        return next(l.strip() for l in lines[start:end] if l.startswith("    if: "))

    def test_science_long_jobs_dispatch_only(self):
        for name in ("release-native-transport", "release-native-paired",
                     "python-portability", "python-manylinux", "a0-science"):
            self.assertEqual(self.job_guard("rust-science.yml", name),
                             "if: github.event_name == 'workflow_dispatch' && inputs.suite == 'full'")

    def test_wasm_full_admission_not_routine_push(self):
        guard = self.job_guard('rust-science.yml', 'wasm-contract')
        self.assertIn("github.event_name == 'workflow_dispatch' && inputs.suite == 'wasm'", guard)
        self.assertIn("github.event_name == 'pull_request' && needs.core.outputs.wasm_contract == 'true'", guard)
        self.assertNotIn("github.event_name == 'push'", guard)

    def test_wasm_routine_and_admission_separation(self):
        source = (self.root / "rust-science.yml").read_text()
        self.assertIn("mode: ${{ github.event_name == 'pull_request' && 'routine' || 'admission' }}", source)
        self.assertIn("'rawmocs-wasm-routine' || 'rawmocs-wasm-candidate'", source)
        admission = (self.root / "stable-package-admission.yml").read_text()
        self.assertNotIn("mode: routine", admission)
        self.assertIn("name: rawmocs-wasm-candidate", admission)
        action = (self.root.parent / "actions/wasm-candidate/action.yml").read_text()
        self.assertIn("default: admission", action)
        self.assertEqual(action.count("if: inputs.mode == 'routine'"), 2)
        self.assertNotIn("restore-keys:", action)
        self.assertIn("path: ${{ runner.temp }}/rawmocs-gmp-cache", action)

    def test_playground_smoke_is_scoped_main_or_manual(self):
        guard = self.job_guard("rust-science.yml", "playground-site")
        self.assertIn("github.event_name == 'workflow_dispatch' && inputs.suite == 'playground'", guard)
        self.assertIn("github.event_name == 'push' && github.ref == 'refs/heads/main' && needs.core.outputs.playground_contract == 'true'", guard)
        self.assertNotIn("github.event_name == 'pull_request'", guard)

    def test_pages_deployment_explicit_main_and_smoke_only(self):
        workflow = "rust-science.yml"
        guard = self.job_guard(workflow, "playground-deploy")
        self.assertIn("github.event_name == 'workflow_dispatch' && inputs.suite == 'playground-deploy'", guard)
        self.assertIn("github.repository == 'PhysicsQoo/RaWMoCS' && github.ref == 'refs/heads/main'", guard)
        self.assertNotIn("github.event_name == 'push'", guard)
        self.assertNotIn("github.event_name == 'pull_request'", guard)
        source = (self.root / workflow).read_text()
        site, deployment = source.split("\n  playground-deploy:\n", 1)
        self.assertIn("needs: playground-site", deployment)
        self.assertIn("pages: write", deployment)
        self.assertIn("id-token: write", deployment)
        self.assertIn("enablement: false", deployment)
        self.assertIn("actions/upload-pages-artifact@v3", site)
        self.assertIn("actions/deploy-pages@v4", deployment)
        self.assertNotIn("pages: write", site)
        self.assertNotIn("id-token: write", site)

    def test_native_jobs_share_exact_routing(self):
        source = (self.root / "rust-science.yml").read_text()
        self.assertIn("native_required: ${{ steps.native-route.outputs.required }}", source)
        self.assertIn("steps.scope.outputs.native_contract == 'true'", source)
        self.assertIn("inputs.suite != 'playground' && inputs.suite != 'playground-deploy'", source)
        for job in ("release-cli", "portability"):
            self.assertIn("needs.core.outputs.native_required == 'true'", self.job_guard("rust-science.yml", job))
        core = source.split("\n  a0-science:\n", 1)[0]
        self.assertEqual(core.count("if: steps.native-route.outputs.required == 'true'"), 5)
        self.assertIn("python3 scripts/test_ci_scope.py", core)
        self.assertIn("python3 scripts/test_release_source.py", core)

    def test_no_push_title_full_intent(self):
        for filename in ("rust-science.yml", "codspeed.yml"):
            source = (self.root / filename).read_text()
            for marker in ("[release-science]", "[parallel-full]", "[full-science]", "[performance]"):
                self.assertNotIn(marker, source)
            if filename == "rust-science.yml":
                self.assertIn("default: fast", source)

    def test_standalone_codspeed_auto_comparison_manual(self):
        source = (self.root / "codspeed.yml").read_text()
        own, research = source.split("\n  codspeed:\n", 1)
        self.assertIn("  pull_request:", own)
        self.assertIn("  push:", own)
        self.assertIn("  workflow_dispatch:", own)
        self.assertIn("default: standalone", own)
        self.assertIn("github.event_name != 'workflow_dispatch' || inputs.suite == 'standalone'", own)
        self.assertIn("github.event.pull_request.head.repo.full_name == github.repository", own)
        self.assertIn("cargo codspeed run --bench solver", own)
        self.assertNotIn("quprs_direct_cnf", own)
        self.assertNotIn("repository: PhysicsQoo/", own)
        self.assertIn("github.event_name == 'workflow_dispatch' && inputs.suite == 'comparison'", research)
        self.assertIn("cargo codspeed run --bench solver --bench quprs_direct_cnf", research)
        self.assertIn("Verify simulated output, precision and plan signature", research)
        self.assertEqual(own.count("uses: CodSpeedHQ/action@v5"), 1)
        self.assertEqual(research.count("uses: CodSpeedHQ/action@v5"), 1)
        self.assertNotIn("continue-on-error: true", source)

    def test_direct_cnf_comparison_dispatch_only(self):
        source = (self.root / "quprs-direct-cnf-paired.yml").read_text()
        triggers = source.split("on:\n", 1)[1].split("\npermissions:", 1)[0]
        self.assertEqual(triggers.strip(), "workflow_dispatch:")

    def test_manual_separate_concurrency(self):
        for filename, suffix in (("rust-science.yml", "inputs.suite || 'routine'"),
                                 ("codspeed.yml", "'manual' || 'routine'")):
            source = (self.root / filename).read_text()
            group = next(l for l in source.splitlines() if l.startswith("  group: "))
            self.assertIn("github.event_name == 'workflow_dispatch' && "+suffix, group)

if __name__ == "__main__":
    unittest.main()
