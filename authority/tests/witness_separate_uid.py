"""Disposable container integration probe; run with the witness binary and fixture mounted read-only."""
import hashlib
import json
import os
import pathlib
import signal
import socket
import subprocess
import sys
import time

BASE = pathlib.Path(os.environ.get('ARCHY_WITNESS_LAB', '/lab'))
STATE = BASE / 'state'
JOURNAL = BASE / 'journal'
PROJECT = BASE / 'broker/project/project.json'
SOCKET = BASE / 'witness.sock'
BROKER = 2001
AGENT = 2002
BINARY = pathlib.Path('/input/archy-journal-witness')
FIXTURE = pathlib.Path('/input/courtyard.json')


def owned_dir(path, uid, mode):
    path.mkdir()
    os.chown(path, uid, uid)
    path.chmod(mode)


def synced_record(sequence, previous, key, payload):
    record = {'sequence': sequence, 'previous_sha256': previous,
              'idempotency_key': key, 'payload': payload}
    data = json.dumps(record, separators=(',', ':')).encode()
    digest = hashlib.sha256(data).hexdigest()
    path = JOURNAL / f'{sequence:020}-{digest}.event'
    with path.open('xb') as out:
        out.write(data)
        out.flush()
        os.fsync(out.fileno())
    os.chown(path, BROKER, BROKER)
    path.chmod(0o600)
    fd = os.open(JOURNAL, os.O_RDONLY | os.O_DIRECTORY)
    os.fsync(fd)
    os.close(fd)
    return path, digest


def request(uid, operation):
    result = subprocess.run(['setpriv', '--reuid', str(uid), '--regid', str(uid), '--clear-groups',
                             'python3', __file__, 'client', operation],
                            capture_output=True, text=True, timeout=5)
    assert result.returncode == 0, result.stderr
    return result.stdout.strip()


def start():
    server = subprocess.Popen([str(BINARY), 'serve', str(STATE), str(SOCKET)],
                              stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    for _ in range(100):
        if SOCKET.exists():
            return server
        if server.poll() is not None:
            raise AssertionError(server.stderr.read().decode())
        time.sleep(.02)
    raise AssertionError('socket did not appear')


def stop(server):
    server.send_signal(signal.SIGKILL)
    server.wait(timeout=5)


def main():
    BASE.mkdir(mode=0o755)
    owned_dir(STATE, 0, 0o700)
    owned_dir(JOURNAL, BROKER, 0o700)
    owned_dir(BASE / 'broker', BROKER, 0o700)
    owned_dir(BASE / 'broker/project', BROKER, 0o700)
    PROJECT.write_bytes(FIXTURE.read_bytes())
    os.chown(PROJECT, BROKER, BROKER)
    PROJECT.chmod(0o600)
    archy = BASE / 'broker/project/.archy'
    owned_dir(archy, BROKER, 0o700)
    owned_dir(archy / 'staging', BROKER, 0o700)
    owned_dir(archy / 'revisions', BROKER, 0o700)
    lock = archy / 'lock'
    lock.write_bytes(b'')
    os.chown(lock, BROKER, BROKER)
    lock.chmod(0o600)
    broker_parent = BASE / 'broker/journal'
    owned_dir(broker_parent, BROKER, 0o700)
    bad_init = subprocess.run([str(BINARY), 'init', str(STATE), str(broker_parent),
                               str(PROJECT), str(BROKER), str(BROKER)], capture_output=True)
    assert bad_init.returncode != 0 and b'witness path ancestor' in bad_init.stderr
    subprocess.run([str(BINARY), 'init', str(STATE), str(JOURNAL),
                    str(PROJECT), str(BROKER), str(BROKER)], check=True)
    server = start()
    assert request(BROKER, 'S') == 'EXACT 0'
    denied_peer = subprocess.run(['setpriv', '--reuid', str(AGENT), '--regid', str(AGENT), '--clear-groups',
                                  'python3', __file__, 'client', 'S'], capture_output=True)
    assert denied_peer.returncode != 0
    wrong_uid = subprocess.run(['setpriv', '--reuid', str(AGENT), '--regid', str(BROKER), '--clear-groups',
                                'python3', __file__, 'client', 'S'], capture_output=True, text=True)
    assert wrong_uid.returncode == 0 and wrong_uid.stdout.startswith('DENIED unauthorized peer')
    first, digest = synced_record(1, '0' * 64, 'first', 'candidate-one')
    assert request(BROKER, 'S') == 'STALE 0 1'
    assert request(BROKER, 'E') == 'EXTENDED 1'
    assert request(BROKER, 'E') == 'EXACT 1'
    first_bytes = first.read_bytes()
    first.unlink()
    removed_status = request(BROKER, 'S')
    assert removed_status.startswith('DENIED'), removed_status
    replacement, _ = synced_record(1, '0' * 64, 'other', 'candidate-two')
    replacement_status = request(BROKER, 'S')
    assert replacement_status.startswith('DENIED'), replacement_status
    replacement.unlink()
    first.write_bytes(first_bytes)
    os.chown(first, BROKER, BROKER)
    first.chmod(0o600)
    assert request(BROKER, 'S') == 'EXACT 1'
    second, _ = synced_record(2, digest, 'second', 'candidate-three')
    assert request(BROKER, 'S') == 'STALE 1 2'
    second.unlink()
    assert request(BROKER, 'S') == 'EXACT 1'  # An unobserved tail cannot be protected.
    second, _ = synced_record(2, digest, 'second', 'candidate-three')
    assert request(BROKER, 'E') == 'EXTENDED 2'
    stop(server)
    server = start()
    assert request(BROKER, 'S') == 'EXACT 2'
    last_state = STATE / 'state-00000000000000000002'
    last_bytes = last_state.read_bytes()
    last_state.unlink()
    assert request(BROKER, 'S').startswith('DENIED')
    last_state.write_bytes(last_bytes)
    last_state.chmod(0o600)
    assert request(BROKER, 'S') == 'EXACT 2'
    denied_write = subprocess.run(['setpriv', '--reuid', str(BROKER), '--regid', str(BROKER), '--clear-groups',
                                   'touch', str(STATE / 'attack')], capture_output=True)
    assert denied_write.returncode != 0
    original = PROJECT.read_bytes()
    PROJECT.write_bytes(original + b' ')
    assert request(BROKER, 'S').startswith('DENIED')
    PROJECT.write_bytes(original)
    assert request(BROKER, 'S') == 'EXACT 2'
    replaced = PROJECT.with_name('old-project.json')
    PROJECT.rename(replaced)
    PROJECT.write_bytes(original)
    os.chown(PROJECT, BROKER, BROKER)
    PROJECT.chmod(0o600)
    assert request(BROKER, 'S').startswith('DENIED')
    PROJECT.unlink()
    replaced.rename(PROJECT)
    assert request(BROKER, 'S') == 'EXACT 2'
    state_one = STATE / 'state-00000000000000000001'
    original_state_one = state_one.read_bytes()
    state_one.write_bytes(b'0' * 64 + b'\n')
    assert request(BROKER, 'S').startswith('DENIED')
    state_one.write_bytes(original_state_one)
    assert request(BROKER, 'S') == 'EXACT 2'
    (STATE / 'state-00000000000000000000').unlink()
    assert request(BROKER, 'S').startswith('DENIED')
    stop(server)
    print('PASS: separate UIDs, root parent, unextended tail, rollback/replacement, restart, project inode/source, corrupt/missing state, unauthorized peer/write')


if __name__ == '__main__':
    if len(sys.argv) > 1 and sys.argv[1] == 'client':
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as conn:
            conn.connect(str(SOCKET))
            conn.sendall(sys.argv[2].encode())
            print(conn.recv(256).decode().strip())
    else:
        main()
