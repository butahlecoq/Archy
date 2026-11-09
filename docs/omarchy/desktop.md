# A22 local Omarchy desktop integration

The standalone plugin root is `integrations/omarchy/plugin/` in a source checkout, with `manifest.json` at that root. The core Arch package recipe stages the same root at `/usr/lib/archy/omarchy/plugin/` and places its installer scripts beside it. The Archy monorepo root is not a valid argument to `omarchy plugin add`. The installer writes only `~/.config/omarchy/plugins/archy.status/` and its ownership record under `~/.local/state/archy/omarchy/`. It does not edit `/usr/share/omarchy`, `shell.json`, menus, bindings or hooks, and it does not enable a plugin without the user's separate action.

From a source checkout on a reviewed Omarchy target, preview first:

```sh
python3 integrations/omarchy/install.py
omarchy plugin validate integrations/omarchy/plugin
python3 integrations/omarchy/install.py --apply
omarchy-shell shell rescanPlugins
omarchy plugin enable archy.status
```

After installing a reviewed Arch package that contains the plugin, use a host Python 3 interpreter to run the packaged commands from any directory:

```sh
python3 /usr/lib/archy/omarchy/install.py
omarchy plugin validate /usr/lib/archy/omarchy/plugin
python3 /usr/lib/archy/omarchy/install.py --apply
omarchy-shell shell rescanPlugins
omarchy plugin enable archy.status
```

The packaged installer reads its sibling `plugin/` directory by default. The package-owned files remain under `/usr/lib/archy/omarchy/`; the install copies the plugin into the user's Omarchy config and records ownership. The package does not enable the plugin. Preview removal with `python3 /usr/lib/archy/omarchy/uninstall.py`, then use `--apply` to remove an unchanged user-owned copy.

`install.py` and `uninstall.py` default to a read-only JSON preview. The install refuses an occupied destination without its ownership state, symlinks/special files, changed owned files, a different source inventory, and upgrades that need a separate review. A same-content repeat is idempotent. `python3 integrations/omarchy/uninstall.py --apply` moves an unchanged owned plugin tree to an exclusive backup under `~/.local/state/archy/omarchy/`, then removes its ownership record. An edited plugin remains untouched for manual recovery. To turn off the live panel, explicitly run `omarchy plugin disable archy.status` and `omarchy-shell shell rescanPlugins` on the target. Package removal does not remove this user-owned plugin.

The transaction uses directory handles, no-follow opens and no-replace renames. A replacement at the final install name cannot redirect writes into another directory. Uninstall checks that the moved directory is the inventoried inode before removing the ownership record and restores a different tree when the destination is free. Linux has no atomic conditional rename by inode, so a concurrent writer can still move entries in the gap before the inode check, prevent restoration by occupying the name, or replace the destination after the last check. In that case the installer refuses and preserves state where it can. Inspect the destination and `removed-*` backups before retrying. Run install and uninstall without another process modifying the plugin and state directories.

Optional menu, binding and theme/post-update hook examples are under `integrations/omarchy/templates/` in source and `/usr/lib/archy/omarchy/templates/` in the package recipe. Review and merge them into user-owned Omarchy config only after checking IDs, key chords and filenames; the installer refuses to change these shared files. The menu action and binding use a fixed `omarchy-shell shell toggle archy.status '{}'` command. The hook only asks an already loaded panel to refresh. It does not enable, install, start a run or approve anything. Do not copy a template over an existing config file.

In the panel, enter an absolute local `project.yaml`, `.yml` or `.json`; the plugin does not scan home or save a recent project. Pick a verified accepted run, then pick one of its verified artifacts. The plugin uses fixed argv to `/usr/bin/archy desktop status` and `/usr/bin/archy desktop open`. The CLI re-verifies the accepted run and artifact bytes before copying them to a private runtime directory and spawning `/usr/bin/xdg-open` for a report/PNG or `/usr/bin/blender --factory-startup --disable-autoexec` for a `.blend`. The CLI observes the viewer for 250 ms and reports an immediate nonzero exit with a bounded stderr diagnostic. A receipt says `handed_to_viewer`; it does not claim the viewer displayed it or that a render/IFC was produced. Failures after that window remain outside the receipt. A missing or unverified preview/scene has no open button and the CLI refuses direct attempts. Runtime copies remain in `XDG_RUNTIME_DIR/archy-open-*` for the session, so a viewer can consume them after the CLI exits.

Current accepted core runs contain reports, variants and evaluations. Render/preview, Blender scene and IFC publication remain dependent on the A08/A12/A16/A14 integrations and their target verification. The panel has no generation, worker command, approval or regulatory authority. The generic Quickshell 0.3.2 / Qt 6.12 container tests on WSL exercise transport only. Omarchy 4.0.6 plugin discovery, enable/disable, theme, Lua binding, actual viewer behavior and clean target installation remain `OM-A22-01`/`OM-A21-05` deferred gates.
