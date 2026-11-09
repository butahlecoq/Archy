#!/usr/bin/env python3
"""Run A12 reader cases from captured runtime bytes and locked archives, in scratch."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'packaging/arch'))
import runtime
from runtime_manifest import AnchoredRoot, capture_tree


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--node-archive', type=Path, required=True)
    parser.add_argument('--web-ifc-archive', type=Path, required=True)
    parser.add_argument('--scratch', type=Path, required=True)
    args = parser.parse_args()
    node = runtime.pinned_bytes(args.node_archive, runtime.UPSTREAM_PINS['node'])
    web = runtime.pinned_bytes(args.web_ifc_archive, runtime.UPSTREAM_PINS['web_ifc'])
    entries = capture_tree(ROOT, allow_pending=True)
    with tempfile.TemporaryDirectory(prefix='a12-reader-probe-', dir=args.scratch) as temp:
        base = Path(temp).absolute()
        stage_path = base / 'stage'
        stage_path.mkdir()
        with AnchoredRoot(stage_path) as stage:
            runtime.extract_regular_archive(node, 'node-v22.22.2-linux-x64', stage,
                runtime.PREFIX / 'node', {'bin/node', 'LICENSE'},
                ('LICENSE', Path('usr/share/licenses/archy-runtime/node-LICENSE')))
            runtime.extract_regular_archive(web, 'package', stage,
                runtime.PREFIX / 'ifc-verifier/node_modules/web-ifc',
                notice=('LICENSE.md', Path('usr/share/licenses/archy-runtime/web-ifc-LICENSE.md')))
            runtime.stage_captured_inputs(entries, stage)
            runtime.verify_captured_inputs(entries, stage)
            stage.verify_outputs()
        package = stage_path / 'usr/lib/archy/runtime'
        reader = base / 'tests/backend/ifc-reader'
        reader.mkdir(parents=True)
        for name in ('reader.cjs', 'step-scope.cjs', 'verify-stdin.cjs'):
            shutil.copyfile(package / 'ifc-verifier' / name, reader / name)
        shutil.copytree(ROOT / 'tests/backend/ifc-reader/test', reader / 'test')
        shutil.copytree(ROOT / 'tests/backend/ifc-reader/fixtures', reader / 'fixtures')
        shutil.copytree(ROOT / 'tests/fixtures/briefs', base / 'tests/fixtures/briefs')
        env = {'PATH': '/usr/bin:/bin', 'LC_ALL': 'C', 'TZ': 'UTC', 'TMPDIR': str(base),
               'NODE_PATH': str(package / 'ifc-verifier/node_modules')}
        version = subprocess.run([str(package / 'node/bin/node'), '--version'], env=env,
                                 text=True, capture_output=True, timeout=30, check=True).stdout.strip()
        if version != 'v22.22.2':
            raise ValueError('pinned Node version mismatch: ' + version)
        print('pinned node:', version)
        command = [str(package / 'node/bin/node'), '--test',
                   '--test-name-pattern=framed verifier rejects reviewer comment claims|normalized accepted source binds original JSON and YAML bytes',
                   str(reader / 'test/reader.test.cjs')]
        result = subprocess.run(command, env=env, text=True, stdout=subprocess.PIPE,
                                stderr=subprocess.STDOUT, timeout=360)
        print(result.stdout, end='')
        if result.returncode:
            raise SystemExit(result.returncode)


if __name__ == '__main__':
    main()
