import hashlib
import json
import pathlib
import re
import unittest
from urllib.request import urlopen

ROOT = pathlib.Path(__file__).resolve().parents[3]
PLUGIN = ROOT / "integrations/omarchy/plugin"
FIXTURE = pathlib.Path(__file__).parent / "fixtures/upstream-v4.0.6.json"


def validate_manifest(root):
    data = json.loads((root / "manifest.json").read_text())
    assert set(data) == {"schemaVersion", "id", "name", "version", "kinds", "entryPoints"}
    assert data["schemaVersion"] == 1
    assert re.fullmatch(r"[a-z][a-z0-9-]*\.[a-z][a-z0-9-]*", data["id"])
    assert not data["id"].startswith("omarchy.")
    assert data["kinds"] == ["panel"]
    assert set(data["entryPoints"]) == {"panel"}
    entry = data["entryPoints"]["panel"]
    assert re.fullmatch(r"[A-Za-z][A-Za-z0-9_-]*\.qml", entry)
    assert (root / entry).is_file() and not (root / entry).is_symlink()
    for path in root.iterdir():
        assert path.is_file() and not path.is_symlink()
    return data


class PluginContract(unittest.TestCase):
    def test_manifest_and_inventory(self):
        self.assertEqual(validate_manifest(PLUGIN), {
            "schemaVersion": 1, "id": "archy.status", "name": "Archy", "version": "0.1.0",
            "kinds": ["panel"], "entryPoints": {"panel": "Main.qml"},
        })
        self.assertEqual({p.name for p in PLUGIN.iterdir()}, {"manifest.json", "Main.qml", "DoctorProcess.qml", "DoctorModel.js"})

    def test_bad_manifest_cases(self):
        import tempfile
        with tempfile.TemporaryDirectory() as td:
            root = pathlib.Path(td)
            (root / "Main.qml").write_text("Item {}")
            valid = json.loads((PLUGIN / "manifest.json").read_text())
            for changes in ({"id": "omarchy.bad"}, {"id": "archy"}, {"kinds": ["service"]},
                            {"entryPoints": {"panel": "../Main.qml"}},
                            {"entryPoints": {"panel": "Missing.qml"}}):
                (root / "manifest.json").write_text(json.dumps(valid | changes))
                with self.assertRaises(AssertionError):
                    validate_manifest(root)
            (root / "manifest.json").write_text(json.dumps(valid))
            (root / "Main.qml").unlink()
            (root / "Main.qml").symlink_to(PLUGIN / "Main.qml")
            with self.assertRaises(AssertionError):
                validate_manifest(root)

    def test_fixed_process_and_ui_contract(self):
        process = (PLUGIN / "DoctorProcess.qml").read_text()
        main = (PLUGIN / "Main.qml").read_text()
        self.assertIn('command: ["/usr/bin/archy", "doctor", "--json"]', process)
        for token in ('workingDirectory: "/"', 'clearEnvironment: true', 'splitMarker: ""', 'interval: 20000', 'interval: 250', 'Model.append', 'Model.complete'):
            self.assertIn(token, process)
        self.assertIn('Model.complete(p, exitCode, trustedNormalCompletion, generation)', process)
        self.assertIn('trustedNormalCompletion = exitStatus === 0', process)
        self.assertIn('p.stdoutBytes <= Model.MAX_BYTES && p.stderrBytes === 0', process)
        self.assertIn('if (queued && root.panelOpen) root.refresh(false)', process)
        self.assertNotRegex(process + main, r"StdioCollector|Qt\.openUrlExternally|eval\(|createQmlObject|mise\b")
        for token in ('FloatingWindow', 'shell.hide("archy.status")', 'Text.PlainText', 'PanelSectionHeader', 'PanelSeparator', 'Style.space(', 'DoctorProcess'):
            self.assertIn(token, main)
        for token in ('Project status', 'Run status', 'Open report', 'Open preview', 'Open Blender', 'enabled: false'):
            self.assertIn(token, main)

    def test_tagged_source_contract(self):
        evidence = json.loads(FIXTURE.read_text())
        self.assertEqual(evidence["tag"], "v4.0.6")
        # Offline fixture pins exact source identities; opt-in live drift check uses
        # ARCHY_CHECK_UPSTREAM=1 so ordinary contract runs need no network.
        import os
        if os.environ.get("ARCHY_CHECK_UPSTREAM") != "1":
            return
        for path, digest in (("shell/plugins/dev-gallery/GalleryPanel.qml", "gallery_panel_sha256"),
                             ("shell/services/PluginRegistry.qml", "plugin_registry_sha256")):
            with urlopen(evidence["source"] + path, timeout=10) as response:
                body = response.read()
            self.assertEqual(hashlib.sha256(body).hexdigest(), evidence[digest])
            source = body.decode()
            if digest == "gallery_panel_sha256":
                for api in ("FloatingWindow {", "function open(payloadJson)", "function close()", "shell.hide("):
                    self.assertIn(api, source)
            else:
                for api in ('schemaVersion', 'entryPoints', 'isSafeEntryPoint'):
                    self.assertIn(api, source)


if __name__ == "__main__":
    unittest.main()
