"""Exercise bounded SDK stdio framing and JSON-RPC parse errors on real pipes."""
import json
import array
import fcntl
import os
import pathlib
import select
import shutil
import subprocess
import tempfile
import termios
import threading
import time

ROOT = pathlib.Path(__file__).resolve().parents[3]
TARGET = pathlib.Path(os.environ.get('CARGO_TARGET_DIR', ROOT / 'target'))
if not TARGET.is_absolute():
    TARGET = ROOT / TARGET
BIN = TARGET / 'debug/archy-mcp'
META = {'io.modelcontextprotocol/protocolVersion': '2026-07-28',
        'io.modelcontextprotocol/clientInfo': {'name': 'parse-test', 'version': '0'},
        'io.modelcontextprotocol/clientCapabilities': {}}
MAX_FRAME = 1_048_576


def encode(identifier, method='tools/call', modern=True):
    if method == 'tools/list':
        params = {}
    else:
        params = {'name': 'get_project', 'arguments': {'project_id': 'courtyard-demo'}}
    if modern:
        params['_meta'] = META
    return json.dumps({'jsonrpc': '2.0', 'id': identifier, 'method': method,
                       'params': params}, ensure_ascii=False, separators=(',', ':')).encode() + b'\n'


class Reader:
    def __init__(self, stream):
        self.stream = stream
        self.pending = bytearray()

    def frame(self, timeout=10):
        deadline = time.monotonic() + timeout
        while b'\n' not in self.pending:
            left = deadline - time.monotonic()
            assert left > 0 and select.select([self.stream], [], [], left)[0], 'response timeout'
            chunk = os.read(self.stream.fileno(), 65536)
            assert chunk, 'unexpected stdout EOF'
            self.pending.extend(chunk)
            assert len(self.pending) <= MAX_FRAME * 2, 'unexpected buffered output'
        raw, _, rest = self.pending.partition(b'\n')
        self.pending = bytearray(rest)
        assert len(raw) + 1 <= MAX_FRAME
        return json.loads(raw)


def start(project):
    return subprocess.Popen([str(BIN), '--project', str(project)], stdin=subprocess.PIPE,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, bufsize=0)


def finish(process):
    process.stdin.close()
    process.wait(timeout=10)
    assert process.returncode == 0, (process.returncode, process.stderr.read())


def parse_error(reply):
    assert reply == {'jsonrpc': '2.0', 'id': None,
                     'error': {'code': -32700, 'message': 'Parse error'}}, reply


def fragments(stream, payload):
    for cut in (1, 2, 3, 5, 8):
        if not payload:
            break
        stream.write(payload[:cut])
        stream.flush()
        payload = payload[cut:]
    if payload:
        stream.write(payload)
        stream.flush()


with tempfile.TemporaryDirectory(prefix='archy-mcp-parse-') as td:
    project = pathlib.Path(td) / 'project.yaml'
    shutil.copyfile(ROOT / 'examples/courtyard-housing/project.yaml', project)
    original = project.read_bytes()
    process = start(project)
    reader = Reader(process.stdout)
    fragments(process.stdin, encode('initial-😀'))
    assert reader.frame()['id'] == 'initial-😀'
    bad = [b'{"jsonrpc":"2.0","id":41,\n',
           b'{"jsonrpc":"2.0","id":41,"method":"\\ud800"}\n',
           b'{"jsonrpc":"2.0","id":41,"method":"\\udfff"}\n',
           b'{"jsonrpc":"2.0","id":41,"method":"\xff"}\n',
           b'\n']
    for index, frame in enumerate(bad):
        escape = frame.find(b'\\ud8')
        if escape < 0:
            escape = frame.find(b'\\udf')
        if escape >= 0:
            fragments(process.stdin, frame[:escape + 3])
            assert not select.select([process.stdout], [], [], 0.05)[0]
            fragments(process.stdin, frame[escape + 3:])
        else:
            fragments(process.stdin, frame)
        parse_error(reader.frame())
        process.stdin.write(encode(100 + index))
        process.stdin.flush()
        assert reader.frame()['id'] == 100 + index
    emoji = encode('emoji-😀')
    split = emoji.index('😀'.encode()) + 1
    fragments(process.stdin, emoji[:split])
    assert not select.select([process.stdout], [], [], 0.05)[0]
    fragments(process.stdin, emoji[split:])
    assert reader.frame()['id'] == 'emoji-😀'
    process.stdin.write(b'\xef\xbb\xbf' + encode('bom'))
    process.stdin.write(encode('crlf').rstrip(b'\n') + b'\r\n')
    process.stdin.flush()
    ids = {reader.frame()['id'], reader.frame()['id']}
    assert ids == {'bom', 'crlf'}
    process.stdin.write(b'{"bad":\n{"bad":\n' + encode('after-batch'))
    process.stdin.flush()
    replies = [reader.frame() for _ in range(3)]
    for reply in (r for r in replies if r.get('id') is None):
        parse_error(reply)
    assert sum(r.get('id') is None for r in replies) == 2
    assert sum(r.get('id') == 'after-batch' for r in replies) == 1
    process.stdin.write(b' ' * (MAX_FRAME + 4096))
    process.stdin.flush()
    assert not select.select([process.stdout], [], [], 0.05)[0]
    process.stdin.write(b'\n')
    process.stdin.write(encode('after-oversize'))
    process.stdin.flush()
    assert reader.frame()['id'] == 'after-oversize'
    finish(process)
    assert project.read_bytes() == original
    assert sorted(p.name for p in pathlib.Path(td).iterdir()) == ['project.yaml']

    process = start(project)
    reader = Reader(process.stdout)
    init = {'jsonrpc': '2.0', 'id': 1, 'method': 'initialize',
            'params': {'protocolVersion': '2025-11-25', 'capabilities': {},
                       'clientInfo': {'name': 'test', 'version': '0'}}}
    process.stdin.write(json.dumps(init).encode() + b'\n')
    process.stdin.flush()
    assert reader.frame()['result']['protocolVersion'] == '2025-11-25'
    process.stdin.write(b'{"jsonrpc":"2.0","method":"notifications/initialized"}\n')
    process.stdin.write(b'{"id":99,\n')
    process.stdin.flush()
    parse_error(reader.frame())
    process.stdin.write(encode(2, modern=False))
    process.stdin.flush()
    assert reader.frame()['id'] == 2
    finish(process)

    process = start(project)
    reader = Reader(process.stdout)
    process.stdin.write(encode(1))
    process.stdin.flush()
    assert reader.frame()['id'] == 1
    marker = b'SECRET_UNTRUSTED_MARKER'
    large = b'{"id":"' + marker + b'","data":"' + b'x' * (MAX_FRAME - 256) + b'",]\n'
    assert len(large) < MAX_FRAME
    process.stdin.write(large)
    process.stdin.flush()
    parse_error(reader.frame())
    process.stdin.write(encode(2))
    process.stdin.flush()
    assert reader.frame()['id'] == 2
    finish(process)

    process = start(project)
    reader = Reader(process.stdout)
    process.stdin.write(encode(1))
    process.stdin.flush()
    assert reader.frame()['id'] == 1
    frames = []
    expected_ids = set()
    for i in range(64):
        identifier = 1000 + i
        expected_ids.add(identifier)
        frames.append(encode(identifier, 'tools/list' if i % 4 == 0 else 'tools/call'))
        if i % 4 == 0:
            frames.append(b'{"id":"SECRET_UNTRUSTED_MARKER",\n')
    payload = b''.join(frames)
    errors = []
    def send_all():
        try:
            process.stdin.write(payload)
            process.stdin.flush()
        except OSError as exc:
            errors.append(exc)
    thread = threading.Thread(target=send_all, daemon=True)
    thread.start()
    capacity = fcntl.fcntl(process.stdout.fileno(), fcntl.F_GETPIPE_SZ)
    threshold = max(1, capacity - os.fpathconf(process.stdout.fileno(), 'PC_PIPE_BUF'))
    deadline = time.monotonic() + 10
    available = 0
    while time.monotonic() < deadline:
        count = array.array('i', [0])
        fcntl.ioctl(process.stdout.fileno(), termios.FIONREAD, count, True)
        available = count[0]
        if available >= threshold:
            break
        time.sleep(0.01)
    assert available >= threshold, (available, capacity, 'stdout did not reach backpressure')
    found_ids = set()
    parse_count = 0
    for _ in range(len(frames)):
        reply = reader.frame(timeout=20)
        if reply.get('id') is None:
            parse_error(reply)
            parse_count += 1
        else:
            assert reply['id'] in expected_ids and reply['id'] not in found_ids, reply
            found_ids.add(reply['id'])
        assert 'SECRET_UNTRUSTED_MARKER' not in json.dumps(reply)
    thread.join(timeout=10)
    assert not thread.is_alive() and not errors, errors
    assert found_ids == expected_ids and parse_count == 16
    finish(process)

    process = start(project)
    reader = Reader(process.stdout)
    process.stdin.write(encode(1))
    process.stdin.flush()
    assert reader.frame()['id'] == 1
    process.stdin.write(b'[' * 64 + b'0' + b']' * 64 + b'\n')
    process.stdin.flush()
    assert reader.frame()['error']['code'] == -32600
    process.stdin.write(b'[' * 65 + b'0' + b']' * 65 + b'\n')
    process.stdin.close()
    process.wait(timeout=10)
    assert process.returncode in (0, 1) and not process.stdout.read()

    deep = b'[' * 65 + b'0' + b']' * 65 + b'\n'
    for fragmented in (False, True):
        process = start(project)
        expected = {700, 701, 702}
        payload = b''.join(encode(identifier, method='tools/list')
                           for identifier in sorted(expected)) + deep
        if fragmented:
            cut = payload.index(deep) + 64
            process.stdin.write(payload[:cut])
            process.stdin.flush()
            process.stdin.write(payload[cut:])
        else:
            process.stdin.write(payload)
        process.stdin.close()
        process.stdin = None
        output, stderr = process.communicate(timeout=10)
        replies = [json.loads(line) for line in output.splitlines()]
        assert {reply.get('id') for reply in replies} == expected and all(
            'result' in reply for reply in replies), (
            fragmented, process.returncode, replies, stderr)
        assert process.returncode in (0, 1)

    process = start(project)
    reader = Reader(process.stdout)
    process.stdin.write(encode(1))
    process.stdin.flush()
    assert reader.frame()['id'] == 1
    process.stdin.write(b'{"incomplete":')
    process.stdin.flush()
    finish(process)
    assert not reader.pending and not process.stdout.read(), 'incomplete EOF produced output'
    assert project.read_bytes() == original
    assert sorted(p.name for p in pathlib.Path(td).iterdir()) == ['project.yaml']
print('MCP stdio parse errors, fragmentation, bounds, concurrency, legacy, EOF and nonmutation: PASS')
