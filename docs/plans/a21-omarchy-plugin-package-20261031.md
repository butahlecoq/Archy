# Package the Omarchy plugin with Archy

## Goal

Make the reviewed Arch package carry the standalone Omarchy plugin and its local installer. An Omarchy user can copy the package-owned plugin into their home through the existing preview and apply commands, then enable it with Omarchy's separate plugin command.

## Scope

- Add the plugin files, installer scripts, and user-editable templates to `packaging/arch/files.txt` under `/usr/lib/archy/omarchy/`.
- Extend the package staging test to prove that the allowlist produces the complete standalone plugin root and keeps its files at mode `0644`.
- Document the packaged install and uninstall commands. Keep the install preview read-only and plugin enablement explicit.
- Do not add a package hook, edit user configuration, install the package on the host, or enable the plugin.

## Risks and rationale

The installer defaults to a `plugin/` directory beside its own file. Staging the installer at `/usr/lib/archy/omarchy/install.py` and the plugin at `/usr/lib/archy/omarchy/plugin/` preserves that existing contract. The package stage already accepts files under `/usr/lib/archy/`; no new dependency or package hook is needed. The files remain package-owned, while the user's installed plugin copy remains user-owned and removable through the existing transaction.

The main risk is an incomplete source allowlist that produces a plugin manifest with missing QML modules or templates. The staging test will compare the complete source inventory and inspect staged bytes. Actual package installation and visible Omarchy behavior remain separate target gates.

## Verification

- Run the focused package staging tests and Omarchy install tests.
- Run the packaging test suite, Python syntax checks, `git diff --check`, and the repository's required CI after review and publication.
- On the current Omarchy host, validate the staged plugin root with `omarchy plugin validate`; do not apply installation during this change.

## Results

- The package staging test failed before the allowlist change because all 13 Omarchy payload paths were absent. After the change, `PYTHONDONTWRITEBYTECODE=1 python3 -B -m unittest tests.packaging.test_package.StageTests.test_staged_source_package_contains_complete_omarchy_plugin -v` passed. It staged the real source allowlist with fixture binaries and notice inputs, then checked the complete Omarchy file set, bytes, and modes. This does not build an Arch package.
- `PYTHONDONTWRITEBYTECODE=1 python3 -B -m unittest tests.omarchy.integration.test_install -v` passed 15 tests. Python AST checks and `git diff --check` passed.
- On Omarchy 4.0.6, the staged plugin passed `omarchy plugin validate`. The staged `install.py` preview returned `action: install` and `applied: false` with a temporary `HOME`; no plugin or ownership state was created. An independent reviewer also applied and removed the staged plugin through a disposable `HOME` and confirmed preview, apply, and uninstall behavior.
- `PYTHONDONTWRITEBYTECODE=1 python3 -B -m unittest tests.packaging.test_package -v` ran 23 tests but could not complete because this host has no `cargo`. The remaining failures and error report `FileNotFoundError: cargo` from existing notice-coverage paths; the new staged-plugin test passed. Fresh PR CI must cover the full module with Cargo available.
- Two independent Sol-medium reviewers found no issues. One reproduced the staged install and uninstall flow with a disposable home. Neither reviewer claims a built package or installed target acceptance.
