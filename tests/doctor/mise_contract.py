"""Project-local mise contract; run with python3 tests/doctor/mise_contract.py."""

from pathlib import Path
import os
import shutil
import subprocess
import tempfile
import unittest

try:
    import tomllib
except ImportError as exc:
    raise SystemExit("Python 3.11+ (stdlib tomllib) is required for this contract") from exc


ROOT = Path(__file__).resolve().parents[2]


class MiseContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.config = tomllib.loads((ROOT / ".mise.toml").read_text())
        cls.toolchain = tomllib.loads((ROOT / "rust-toolchain.toml").read_text())

    def test_project_rust_matches_exact_toolchain(self):
        self.assertEqual(self.config["tools"], {"rust": "1.99.1"})
        self.assertEqual(self.config["tools"]["rust"], self.toolchain["toolchain"]["channel"])

    def test_no_automatic_install_or_environment_hooks(self):
        self.assertEqual(set(self.config), {"tools", "settings", "env", "tasks"})
        self.assertEqual(self.config["settings"], {"auto_install": False})
        self.assertIs(self.config["settings"]["auto_install"], False)
        self.assertEqual(self.config["env"], {"RUSTUP_AUTO_INSTALL": "0"})
        for key in ("hooks", "venv", "python", "pipx"):
            self.assertNotIn(key, self.config)
        self.assertNotIn("global", self.config)
        self.assertNotIn("lazy", self.config["tools"])

    def test_preflight_checks_effective_rust_and_cargo_versions(self):
        tasks = self.config["tasks"]
        check = (ROOT / "scripts/check-rust-toolchain.sh").read_text()
        self.assertIn("rustc --version", check)
        self.assertIn("cargo --version", check)
        self.assertLess(check.index("export RUSTUP_AUTO_INSTALL=0"), check.index("rustc --version"))
        for name in ("fmt", "lint", "test:core", "build"):
            self.assertNotIn("depends", tasks[name])
            self.assertEqual(tasks[name]["run"][0], "sh scripts/check-rust-toolchain.sh")
        self.assertIn("1.99.1", check)
        self.assertIn("exit 1", check)

    def test_preflight_rejects_effective_version_overrides(self):
        script = ROOT / "scripts/check-rust-toolchain.sh"
        with tempfile.TemporaryDirectory() as directory:
            binary_dir = Path(directory)
            for name in ("rustc", "cargo"):
                stub = binary_dir / name
                stub.write_text('#!/bin/sh\nprintf "%s %s (test)\\n" "' + name + '" "${FAKE_' + name.upper() + '_VERSION}"\n')
                stub.chmod(0o755)
            env = dict(os.environ, PATH=str(binary_dir) + os.pathsep + os.environ.get("PATH", ""))
            env.update(FAKE_RUSTC_VERSION="1.99.1", FAKE_CARGO_VERSION="1.99.1")
            self.assertEqual(subprocess.run(["/bin/sh", str(script)], env=env, capture_output=True).returncode, 0)
            for variable in ("FAKE_RUSTC_VERSION", "FAKE_CARGO_VERSION"):
                changed = dict(env, **{variable: "1.98.0"})
                result = subprocess.run(["/bin/sh", str(script)], env=changed, capture_output=True)
                self.assertEqual(result.returncode, 1, result.stderr)
                self.assertIn(b"expected", result.stderr)

    def test_missing_rustup_toolchain_does_not_create_runtime_files(self):
        rustup = shutil.which("rustup")
        if rustup is None:
            self.skipTest("rustup proxy is not installed")
        proxy_dir = Path(rustup).parent
        if not all((proxy_dir / name).exists() for name in ("rustc", "cargo")):
            self.skipTest("rustup rustc/cargo proxies are not installed")
        script = ROOT / "scripts/check-rust-toolchain.sh"
        for proxied in ("rustc", "cargo"):
            with self.subTest(proxied=proxied), tempfile.TemporaryDirectory() as directory:
                fixture = Path(directory)
                rustup_home = fixture / "rustup"
                rustup_home.mkdir()
                (rustup_home / "settings.toml").write_text('version = "12"\nprofile = "minimal"\n\n[overrides]\n')
                cargo_home = fixture / "cargo-home"
                cargo_home.mkdir()
                binary_dir = fixture / "bin"
                binary_dir.mkdir()
                (binary_dir / proxied).symlink_to(proxy_dir / proxied)
                other = "cargo" if proxied == "rustc" else "rustc"
                stub = binary_dir / other
                stub.write_text(f'#!/bin/sh\nprintf "{other} 1.99.1 (test)\\n"\n')
                stub.chmod(0o755)
                before = sorted(str(path.relative_to(fixture)) for path in fixture.rglob("*"))
                env = dict(
                    os.environ,
                    HOME=directory,
                    PATH=str(binary_dir) + os.pathsep + "/usr/bin:/bin",
                    CARGO_HOME=str(cargo_home),
                    RUSTUP_HOME=str(rustup_home),
                    RUSTUP_TOOLCHAIN="1.95.0",
                    RUSTUP_AUTO_INSTALL="1",
                    RUSTUP_DIST_SERVER=(fixture / "denied-dist").as_uri(),
                    RUSTUP_UPDATE_ROOT=(fixture / "denied-update").as_uri(),
                )
                result = subprocess.run(["/bin/sh", str(script)], env=env, capture_output=True, text=True, timeout=10)
                after = sorted(str(path.relative_to(fixture)) for path in fixture.rglob("*"))
                self.assertNotEqual(result.returncode, 0)
                self.assertNotIn("downloading", result.stderr.lower())
                self.assertEqual(after, before)

    def test_core_tasks_are_exact_and_do_not_call_workers(self):
        tasks = self.config["tasks"]
        expected = {
            "fmt": "cargo fmt --all -- --check",
            "lint": "cargo clippy --workspace --all-targets --locked -- -D warnings",
            "test:core": "cargo test --workspace --locked",
            "build": "cargo build -p archy-cli --release --locked",
        }
        for name, command in expected.items():
            self.assertEqual(tasks[name]["run"], ["sh scripts/check-rust-toolchain.sh", command])
            self.assertNotIn("ifc", str(tasks[name]).lower())
            self.assertNotIn("blender", str(tasks[name]).lower())
        self.assertNotIn("setup:ifc", tasks)
        self.assertNotIn("test:ifc", tasks)
        self.assertEqual(set(tasks), set(expected))

    def test_documented_invocations_set_no_install_before_mise(self):
        install = (ROOT / "docs/omarchy/install.md").read_text()
        for name in ("fmt", "lint", "test:core", "build"):
            self.assertIn(f"`MISE_AUTO_INSTALL=false RUSTUP_AUTO_INSTALL=0 mise run {name}`", install)
        self.assertIn("MISE_AUTO_INSTALL=true", install)
        self.assertIn("RUSTUP_AUTO_INSTALL=1", install)
        self.assertIn("--skip-deps", install)

    def test_documented_prefix_overrides_inherited_install_setting_at_startup(self):
        install = (ROOT / "docs/omarchy/install.md").read_text()
        with tempfile.TemporaryDirectory() as directory:
            stub = Path(directory) / "mise"
            stub.write_text('#!/bin/sh\nprintf "%s %s\\n" "$MISE_AUTO_INSTALL" "$RUSTUP_AUTO_INSTALL"\n')
            stub.chmod(0o755)
            env = dict(os.environ, PATH=directory + os.pathsep + os.environ.get("PATH", ""), MISE_AUTO_INSTALL="true", RUSTUP_AUTO_INSTALL="1")
            for name in ("fmt", "lint", "test:core", "build"):
                command = f"MISE_AUTO_INSTALL=false RUSTUP_AUTO_INSTALL=0 mise run {name}"
                self.assertIn(f"`{command}`", install)
                result = subprocess.run(["/bin/sh", "-c", command], env=env, capture_output=True, text=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout.strip(), "false 0")

    def test_config_has_no_global_writes_or_install_commands(self):
        raw = (ROOT / ".mise.toml").read_text().lower()
        for forbidden in ("mise use", "mise install", "mise trust", "pip install", "uv sync", "cargo install", "~/.config", "~/.bashrc", "~/.zshrc"):
            self.assertNotIn(forbidden, raw)


if __name__ == "__main__":
    unittest.main()
