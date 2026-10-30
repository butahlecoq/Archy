import hashlib, os, posixpath, sys, tarfile
from pathlib import Path

archive, staged = map(Path, sys.argv[1:3])
omit = sys.argv[3] if len(sys.argv) > 3 else None
root = 'blender-4.5.15-linux-x64'
files = {}
links = {}
digest = lambda: hashlib.sha256()
def hash_stream(stream):
    h = digest()
    while chunk := stream.read(1024 * 1024): h.update(chunk)
    return h.hexdigest()
with tarfile.open(archive, 'r|xz') as tf:
    for entry in tf:
        name = entry.name.rstrip('/')
        if not name.startswith(root + '/'):
            assert name == root
            continue
        rel = name[len(root)+1:]
        assert rel and not rel.startswith('/') and '..' not in rel.split('/')
        path = staged / rel
        if entry.isfile():
            with tf.extractfile(entry) as source: expected = hash_stream(source)
            if rel == omit:
                assert not path.exists()
            else:
                with path.open('rb') as actual: got = hash_stream(actual)
                if expected != got: raise SystemExit('digest mismatch: ' + rel)
            files[rel] = expected
        elif entry.issym():
            target = posixpath.normpath(posixpath.join(posixpath.dirname(rel), entry.linkname))
            if target.startswith('../') or target.startswith('/'): raise SystemExit('unsafe link: ' + rel)
            links[rel] = target
        elif not entry.isdir():
            raise SystemExit('unexpected member: ' + rel)
for rel,target in links.items():
    seen=set()
    while target in links:
        if target in seen: raise SystemExit('link cycle')
        seen.add(target)
        target=links[target]
    expected=files[target]
    with (staged/rel).open('rb') as actual: got=hash_stream(actual)
    if expected != got: raise SystemExit('link digest mismatch: ' + rel)
actual={str(p.relative_to(staged)) for p in staged.rglob('*') if p.is_file()}
expected_paths = (set(files) | set(links)) - ({omit} if omit else set())
assert omit is None or omit in files
assert actual == expected_paths, (len(actual), len(files), len(links), list(actual - expected_paths)[:5])
print('matched archive regular files:',len(files) - bool(omit),'dereferenced symlinks:',len(links), 'explicit omission:',omit)
