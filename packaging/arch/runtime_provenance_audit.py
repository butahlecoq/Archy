#!/usr/bin/env python3
"""Compare pinned source archives and a binary wheel without extracting them."""
import argparse
import email
import hashlib
import json
from pathlib import Path
import tarfile
import zipfile


def sha_file(path):
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()


def sha_bytes(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    for name in ('ifc-wheel', 'ifc-upstream-zip', 'ifc-source', 'ifc-mvd-source', 'ifc-simple-spf-source',
                 'blender-source', 'blender-license', 'output'):
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists() or args.output.is_symlink():
        parser.error('output exists')
    source_python = {}
    source_init = None
    with tarfile.open(args.ifc_source, 'r:gz') as archive:
        for member in archive:
            marker = '/src/ifcopenshell-python/'
            if member.isfile() and marker in member.name and member.name.endswith('.py'):
                name = member.name.split(marker, 1)[1]
                data = archive.extractfile(member).read()
                source_python[name] = sha_bytes(data)
                if name == 'ifcopenshell/__init__.py':
                    source_init = data

    with zipfile.ZipFile(args.ifc_wheel) as wheel:
        names = wheel.namelist()
        metadata = [name for name in names if name.endswith('.dist-info/METADATA')]
        if len(metadata) != 1:
            raise ValueError('wheel must have exactly one metadata file')
        fields = email.message_from_bytes(wheel.read(metadata[0]))
        python_files = {name: sha_bytes(wheel.read(name)) for name in names
                        if name.startswith('ifcopenshell/') and name.endswith('.py')}
        wheel_init = wheel.read('ifcopenshell/__init__.py')
        wrapper = wheel.read('ifcopenshell/ifcopenshell_wrapper.py')
        submodules = {}
        for prefix, source_archive in (('mvd', args.ifc_mvd_source),
                                       ('simple_spf', args.ifc_simple_spf_source)):
            source_files = {}
            source_license = None
            with tarfile.open(source_archive, 'r:gz') as archive:
                for member in archive:
                    if not member.isfile() or '/' not in member.name:
                        continue
                    rel = member.name.split('/', 1)[1]
                    if rel.endswith('.py'):
                        source_files[rel] = sha_bytes(archive.extractfile(member).read())
                    elif rel == 'LICENSE':
                        source_license = sha_bytes(archive.extractfile(member).read())
            base = 'ifcopenshell/' + prefix + '/'
            wheel_files = {name[len(base):]: digest for name, digest in python_files.items()
                           if name.startswith(base)}
            common_files = source_files.keys() & wheel_files.keys()
            submodules[prefix] = {
                'source_sha256': sha_file(source_archive), 'license_sha256': source_license,
                'wheel_python_files': len(wheel_files),
                'matching': sum(source_files[name] == wheel_files[name] for name in common_files),
                'mismatched': sorted(name for name in common_files if source_files[name] != wheel_files[name]),
                'absent': sorted(wheel_files.keys() - source_files.keys()),
            }
        native = []
        for info in wheel.infolist():
            if info.filename.startswith('ifcopenshell/') and '.so' in Path(info.filename).name:
                data = wheel.read(info)
                native.append({'path': info.filename, 'sha256': sha_bytes(data),
                               'elf': data.startswith(b'\x7fELF'),
                               'zip_mode': (info.external_attr >> 16) & 0o177777})
        notices = [name for name in names if not name.endswith('/') and
                   (name.endswith(('/LICENSE', '/LICENSE.txt', '/LICENSE.md', '/COPYING', '/NOTICE'))
                    or '.dist-info/licenses/' in name)]

    with zipfile.ZipFile(args.ifc_upstream_zip) as upstream:
        upstream_native = {info.filename: sha_bytes(upstream.read(info)) for info in upstream.infolist()
                           if info.filename.startswith('ifcopenshell/') and '.so' in Path(info.filename).name}
    wheel_native = {item['path']: item['sha256'] for item in native}
    common_native = wheel_native.keys() & upstream_native.keys()

    common = python_files.keys() & source_python.keys()
    mismatched = sorted(name for name in common if python_files[name] != source_python[name])
    absent = sorted(python_files.keys() - source_python.keys())
    blender_source_license = None
    with tarfile.open(args.blender_source, 'r:xz') as archive:
        for member in archive:
            if member.isfile() and member.name.endswith('/release/license/license.md'):
                if blender_source_license is not None:
                    raise ValueError('duplicate Blender source license')
                blender_source_license = sha_bytes(archive.extractfile(member).read())
    result = {
        'ifc_source_sha256': sha_file(args.ifc_source),
        'ifc_wheel_sha256': sha_file(args.ifc_wheel),
        'ifc_wheel_name': fields.get('Name'), 'ifc_wheel_version': fields.get('Version'),
        'ifc_license_expression': fields.get('License-Expression'),
        'ifc_declared_license_files': fields.get_all('License-File', []),
        'ifc_wheel_notice_paths': sorted(notices),
        'ifc_python_files': len(python_files), 'ifc_python_common': len(common),
        'ifc_python_mismatched': mismatched, 'ifc_python_absent_from_tag': absent,
        'ifc_init_diff_is_version_only': bool(source_init is not None and
            source_init.count(b'__version__ = version = "0.0.0"') == 1 and
            source_init.replace(b'__version__ = version = "0.0.0"',
                                f'__version__ = version = "{fields.get("Version")}"'.encode()) == wheel_init),
        'ifc_wrapper_has_swig_generated_marker': b'automatically generated by SWIG' in wrapper[:256],
        'ifc_submodules': submodules,
        'ifc_native_files': sorted(native, key=lambda item: item['path']),
        'ifc_upstream_zip_sha256': sha_file(args.ifc_upstream_zip),
        'ifc_upstream_native_matching': sum(wheel_native[name] == upstream_native[name] for name in common_native),
        'ifc_upstream_native_mismatched': sorted(name for name in common_native if wheel_native[name] != upstream_native[name]),
        'ifc_upstream_native_missing': sorted(wheel_native.keys() - upstream_native.keys()),
        'ifc_upstream_native_extra': sorted(upstream_native.keys() - wheel_native.keys()),
        'blender_source_sha256': sha_file(args.blender_source),
        'blender_source_license_sha256': blender_source_license,
        'blender_binary_license_sha256': sha_file(args.blender_license),
        'limitations': 'Filename and pure-Python comparisons do not prove native binary/source correspondence or notice closure.',
    }
    with args.output.open('x') as output:
        json.dump(result, output, indent=2, sort_keys=True)
        output.write('\n')
    print('audit_sha256=' + sha_file(args.output))


if __name__ == '__main__':
    main()
