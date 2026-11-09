"""Exercise bounded SDK stdio framing and JSON-RPC parse errors on real pipes."""
import json
import atexit
import fcntl
import os
import pathlib
import select
import shutil
import subprocess
import tempfile
import threading
import time

ROOT = pathlib.Path(__file__).resolve().parents[3]
TARGET = pathlib.Path(os.environ.get('CARGO_TARGET_DIR', ROOT / 'target'))
if not TARGET.is_absolute():
    TARGET = pathlib.Path.cwd() / TARGET
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


PROCESSES = []


@atexit.register
def cleanup_processes():
    for process in PROCESSES:
        if process.poll() is None:
            process.kill()
    for process in PROCESSES:
        process.wait(timeout=10)
        for stream in (process.stdin, process.stdout, process.stderr):
            if stream is not None and not stream.closed:
                stream.close()


def start(project, stdout=subprocess.PIPE, env=None, pass_fds=(), binary=BIN):
    process = subprocess.Popen([str(binary), '--project', str(project)], stdin=subprocess.PIPE,
                               stdout=stdout, stderr=subprocess.PIPE, bufsize=0,
                               env=env, pass_fds=pass_fds)
    PROCESSES.append(process)
    return process


def finish(process):
    process.stdin.close()
    process.wait(timeout=10)
    assert process.returncode == 0, (process.returncode, process.stderr.read())


def parse_error(reply):
    assert reply == {'jsonrpc': '2.0', 'id': None,
                     'error': {'code': -32700, 'message': 'Parse error'}}, reply


def successful_batch_reply(reply, method):
    assert reply.get('jsonrpc') == '2.0' and 'error' not in reply, reply
    result = reply.get('result')
    assert isinstance(result, dict), reply
    if method == 'tools/list':
        tools = result.get('tools')
        assert isinstance(tools, list) and any(
            isinstance(tool, dict) and tool.get('name') == 'get_project'
            and isinstance(tool.get('inputSchema'), dict)
            and isinstance(tool.get('outputSchema'), dict) for tool in tools), reply
    else:
        assert method == 'tools/call', method
        assert result.get('isError') is False, reply
        structured = result.get('structuredContent')
        assert isinstance(structured, dict) and structured.get('ok') is True, reply
        data = structured.get('data')
        assert isinstance(data, dict) and data.get('project_id') == 'courtyard-demo', reply
        content = result.get('content')
        assert isinstance(content, list) and content and content[0].get('type') == 'text', reply
        assert json.loads(content[0]['text']) == structured, reply


def saturated_call_reply(reply):
    assert reply.get('jsonrpc') == '2.0' and 'error' not in reply, reply
    result = reply.get('result')
    assert isinstance(result, dict) and result.get('isError') is True, reply
    structured = result.get('structuredContent')
    assert structured == {'ok': False, 'data': None, 'error': {
        'code': 'BACKEND_UNAVAILABLE', 'field': 'tool',
        'message': 'Concurrent tool limit reached', 'retriable': True}}, reply
    content = result.get('content')
    assert isinstance(content, list) and len(content) == 1 and content[0].get('type') == 'text', reply
    assert json.loads(content[0]['text']) == structured, reply


def check_fabricated_error_rejection():
    for method in ('tools/list', 'tools/call'):
        forged = {'jsonrpc': '2.0', 'id': 1000, 'error': {'code': -32000,
                  'message': 'fabricated'}}
        try:
            successful_batch_reply(forged, method)
        except AssertionError:
            pass
        else:
            raise AssertionError(('fabricated error accepted', method))
    forged_tool_error = {'jsonrpc': '2.0', 'id': 1001, 'result': {
        'isError': True, 'structuredContent': {'ok': False, 'data': None}}}
    try:
        successful_batch_reply(forged_tool_error, 'tools/call')
    except AssertionError:
        pass
    else:
        raise AssertionError('fabricated tool error accepted')


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


def probe_event(fd, expected, deadline):
    remaining = deadline - time.monotonic()
    assert remaining > 0 and select.select([fd], [], [], remaining)[0], (
        expected, 'stdout write event timeout')
    actual = os.read(fd, 1)
    assert actual == expected, (expected, actual, 'stdout write event order')


def drain_available(fd, reader, control_fd):
    drained = 0
    while select.select([fd], [], [], 0)[0]:
        if drained == 0:
            assert not select.select([control_fd], [], [], 0)[0], (
                'stdout write resumed before drain read')
        chunk = os.read(fd, 65536)
        if not chunk:
            break
        drained += len(chunk)
        reader.pending.extend(chunk)
        assert len(reader.pending) <= MAX_FRAME * 2, 'unexpected buffered output'
    assert drained > 0, 'Pending without a positive stdout drain'
    return drained


def check_probe_wiring():
    source_dir = pathlib.Path(__file__).resolve().parents[1] / 'src'
    main = ' '.join((source_dir / 'main.rs').read_text().split())
    example = ' '.join((pathlib.Path(__file__).resolve().parents[1] /
                        'examples/stdout_backpressure_probe.rs').read_text().split())
    for expression in ('let (reader, writer) = stdio_pipes();',
                       'let input = JsonLineFilter::new(DepthGuard::new(reader), output.clone());',
                       'AsyncRwTransport::<RoleServer, _, _>::new_server(input, ResponseGuard::new(output))',
                       '.with_max_line_length(1_048_576);',
                       'ArchyServer::new(project).serve(transport).await'):
        assert expression in main and expression in example, ('transport wiring drift', expression)
    assert 'let output = FrameWriter::new(writer);' in main
    assert 'let output = FrameWriter::new(ObservedWriter {' in example
    assert 'inner: writer,' in example


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

    check_fabricated_error_rejection()
    check_probe_wiring()
    subprocess.run(['cargo', 'build', '--locked', '-p', 'archy-mcp',
                    '--example', 'stdout_backpressure_probe'], check=True)
    probe = TARGET / 'debug/examples/stdout_backpressure_probe'
    read_fd, write_fd = os.pipe2(os.O_CLOEXEC | os.O_NONBLOCK)
    control_read, control_write = os.pipe2(os.O_CLOEXEC)
    process = None
    thread = None
    try:
        capacity = fcntl.fcntl(read_fd, fcntl.F_SETPIPE_SZ, 4096)
        env = os.environ.copy()
        env['ARCHY_MCP_WRITE_PROBE_FD'] = str(control_write)
        process = start(project, stdout=write_fd, env=env, pass_fds=(control_write,), binary=probe)
        os.close(write_fd)
        write_fd = None
        os.close(control_write)
        control_write = None
        process.stdout = os.fdopen(read_fd, 'rb', buffering=0)
        reader = Reader(process.stdout)
        process.stdin.write(encode(1))
        process.stdin.flush()
        assert reader.frame()['id'] == 1
        initial_frames = []
        call_ids = []
        expected_ids = set()
        for i in range(64):
            identifier = 1000 + i
            expected_ids.add(identifier)
            if i % 4 == 0:
                initial_frames.append(encode(identifier, 'tools/list'))
                initial_frames.append(b'{"id":"SECRET_UNTRUSTED_MARKER",\n')
            else:
                call_ids.append(identifier)
        payload = b''.join(initial_frames)
        errors = []
        def send_all():
            try:
                process.stdin.write(payload)
                process.stdin.flush()
            except OSError as exc:
                errors.append(exc)
        thread = threading.Thread(target=send_all, daemon=True)
        thread.start()
        deadline = time.monotonic() + 10
        for cycle in range(2):
            probe_event(control_read, b'P', deadline)
            assert not select.select([control_read], [], [], 0)[0], (
                cycle, 'stdout write resumed before drain')
            drain_available(read_fd, reader, control_read)
            probe_event(control_read, b'R', deadline)
        found_ids = set()
        parse_count = 0
        drain_deadline = time.monotonic() + 20
        for _ in range(len(initial_frames)):
            reply = reader.frame(timeout=max(0.01, drain_deadline - time.monotonic()))
            if reply.get('id') is None:
                parse_error(reply)
                parse_count += 1
            else:
                assert reply['id'] in expected_ids and reply['id'] not in found_ids, reply
                successful_batch_reply(reply, 'tools/list' if reply['id'] % 4 == 0 else 'tools/call')
                found_ids.add(reply['id'])
            assert 'SECRET_UNTRUSTED_MARKER' not in json.dumps(reply)
        thread.join(timeout=10)
        assert not thread.is_alive() and not errors, errors
        for identifier in call_ids:
            process.stdin.write(encode(identifier))
            process.stdin.flush()
            reply = reader.frame(timeout=max(0.01, drain_deadline - time.monotonic()))
            assert reply.get('id') == identifier and identifier not in found_ids, reply
            successful_batch_reply(reply, 'tools/call')
            found_ids.add(identifier)
            assert 'SECRET_UNTRUSTED_MARKER' not in json.dumps(reply)
        assert found_ids == expected_ids and parse_count == 16
        finish(process)
        print('MCP stdout transport Pending/resumed writes:', 2, 'pipe bytes:', capacity)
    finally:
        if process is not None and process.poll() is None:
            process.kill()
            process.wait(timeout=10)
        if thread is not None:
            thread.join(timeout=10)
        if process is None or process.stdout is None:
            os.close(read_fd)
        if write_fd is not None:
            os.close(write_fd)
        if control_write is not None:
            os.close(control_write)
        os.close(control_read)

    process = start(project)
    reader = Reader(process.stdout)
    process.stdin.write(encode(1) + payload)
    process.stdin.flush()
    production_replies = [reader.frame() for _ in range(1 + len(initial_frames))]
    for identifier in call_ids:
        process.stdin.write(encode(identifier))
        process.stdin.flush()
        production_replies.append(reader.frame())
    finish(process)
    assert not process.stderr.read(), 'shipping binary wrote to stderr'
    assert len(production_replies) == 81, len(production_replies)
    production_ids = set()
    production_parse_count = 0
    for reply in production_replies:
        if reply.get('id') is None:
            parse_error(reply)
            production_parse_count += 1
        else:
            assert reply['id'] in expected_ids | {1} and reply['id'] not in production_ids, reply
            successful_batch_reply(reply, 'tools/list' if reply['id'] != 1 and reply['id'] % 4 == 0 else 'tools/call')
            production_ids.add(reply['id'])
        assert 'SECRET_UNTRUSTED_MARKER' not in json.dumps(reply)
    assert production_ids == expected_ids | {1} and production_parse_count == 16

    process = start(project)
    reader = Reader(process.stdout)
    process.stdin.write(encode(1))
    process.stdin.flush()
    assert reader.frame()['id'] == 1
    burst = b''.join(encode(1000 + i, 'tools/list' if i % 4 == 0 else 'tools/call') +
                     (b'{"id":"SECRET_UNTRUSTED_MARKER",\n' if i % 4 == 0 else b'')
                     for i in range(64))
    burst_errors = []
    def send_burst():
        try:
            process.stdin.write(burst)
            process.stdin.flush()
        except OSError as exc:
            burst_errors.append(exc)
    burst_thread = threading.Thread(target=send_burst, daemon=True)
    burst_thread.start()
    burst_ids = set()
    burst_parse_count = 0
    saturated = 0
    succeeded = 0
    for _ in range(80):
        reply = reader.frame(timeout=20)
        identifier = reply.get('id')
        if identifier is None:
            parse_error(reply)
            burst_parse_count += 1
        else:
            assert identifier in expected_ids and identifier not in burst_ids, reply
            if identifier % 4 == 0:
                successful_batch_reply(reply, 'tools/list')
            elif isinstance(reply.get('result'), dict) and reply['result'].get('isError') is True:
                saturated_call_reply(reply)
                saturated += 1
            else:
                successful_batch_reply(reply, 'tools/call')
                succeeded += 1
            burst_ids.add(identifier)
        assert 'SECRET_UNTRUSTED_MARKER' not in json.dumps(reply)
    burst_thread.join(timeout=10)
    assert not burst_thread.is_alive() and not burst_errors, burst_errors
    assert burst_ids == expected_ids and burst_parse_count == 16 and succeeded + saturated == 48, (
        len(burst_ids), burst_parse_count, succeeded, saturated)
    finish(process)
    assert not reader.pending and not process.stdout.read(), 'extra burst output'
    assert not process.stderr.read(), 'shipping burst wrote to stderr'
    print('MCP mixed burst calls: success', succeeded, 'bounded saturation', saturated)

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
