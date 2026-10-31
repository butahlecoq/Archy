const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const path = require('node:path');
const { spawn } = require('node:child_process');
const { StringDecoder } = require('node:string_decoder');

const source = fs.readFileSync(path.resolve(__dirname, '../../../integrations/omarchy/plugin/DoctorModel.js'), 'utf8');
const model = vm.createContext({});
vm.runInContext(source, model);
const report = () => ({
  schema_version: '1', host: { os_id: null, kernel: null, architecture: 'x86_64', environment: 'wsl', evidence_sources: [] },
  platform: { supported_baseline: 'v4.0.6', compatibility: null, raw_package_identity: null, upstream_version: null, development_override: false },
  checks: [{ id: 'platform.omarchy', required_for: ['desktop'], status: 'deferred', observed: null, expected: null, evidence: [], reason: 'not observed', remedy: 'use Omarchy' }],
  capabilities: Object.fromEntries(['core', 'blender', 'ifc', 'containment', 'desktop'].map(k => [k, { ready: false, required_checks: [] }])),
  ready: false, target_acceptance: 'deferred', target_gates: [{ id: 'OM-A21-01', status: 'deferred', reason: 'not run' }]
});

test('valid exit 0 and 1 reports preserve schema data without inferring acceptance', () => {
  for (const code of [0, 1]) {
    const p = model.newProbe(1); model.append(p, 'stdout', JSON.stringify(report()));
    const result = model.complete(p, code, true, 1);
    assert.equal(result.report.target_acceptance, 'deferred');
    assert.equal(result.report.capabilities.desktop.ready, false);
  }
});
test('exit 2 and invalid report never yield success', () => {
  for (const code of [2, -1]) {
    const p = model.newProbe(1); model.append(p, 'stdout', JSON.stringify(report()));
    assert.equal(model.complete(p, code, true, 1).report, null);
  }
  for (const change of [r => r.schema_version = '2', r => r.checks[0].status = 'ready', r => r.host.os_id = 3, r => r.target_gates[0].status = 'available', r => r.checks = Array(129).fill(r.checks[0]), r => r.platform.supported_baseline = 'x'.repeat(4097)]) {
    const r = report(); change(r);
    const p = model.newProbe(1); model.append(p, 'stdout', JSON.stringify(r));
    assert.equal(model.complete(p, 0, true, 1).report, null);
  }
});
test('fragments, byte bounds, stderr flood, stale completion and teardown', () => {
  const p = model.newProbe(4);
  const json = JSON.stringify(report()).replace('not observed', 'caf\\u00e9 observed');
  for (const c of json) model.append(p, 'stdout', c);
  assert.ok(model.complete(p, 1, true, 4).report);
  assert.equal(model.complete(p, 1, true, 5).report, null);
  const q = model.newProbe(6);
  assert.equal(model.append(q, 'stderr', 'é'.repeat(32769)), false);
  assert.ok(q.stderrBytes <= 65536);
  assert.equal(model.complete(q, 0, true, 6).report, null);
  const o = model.newProbe(8);
  assert.equal(model.append(o, 'stdout', 'x'.repeat(65537)), false);
  assert.equal(o.stdoutBytes, 0);
  assert.equal(o.stdout, '');
  const t = model.newProbe(7); model.append(t, 'stdout', json); model.cancel(t);
  assert.equal(model.complete(t, 0, true, 7).report, null);
});
test('truncation and incomplete stream are unavailable', () => {
  const p = model.newProbe(1); model.append(p, 'stdout', '{');
  assert.equal(model.complete(p, 0, true, 1).report, null);
  const q = model.newProbe(1); model.append(q, 'stdout', JSON.stringify(report()));
  assert.equal(model.complete(q, 0, false, 1).report, null);
  const brokenUtf8 = model.newProbe(1);
  assert.equal(model.append(brokenUtf8, 'stdout', '{"reason":"\ufffd"}'), false);
  assert.equal(model.complete(brokenUtf8, 0, true, 1).report, null);
});
test('trusted doctor completion requires ASCII stdout and empty stderr', () => {
  const unicode = model.newProbe(1);
  assert.equal(model.append(unicode, 'stdout', JSON.stringify(report())), true);
  assert.equal(model.append(unicode, 'stdout', 'é'), false);
  assert.equal(model.complete(unicode, 0, true, 1).report, null);
  const warning = model.newProbe(2);
  model.append(warning, 'stdout', JSON.stringify(report()));
  model.append(warning, 'stderr', 'warning');
  assert.equal(model.complete(warning, 0, true, 2).report, null);
  const extra = model.newProbe(3);
  model.append(extra, 'stdout', JSON.stringify(report()) + '\n{}');
  assert.equal(model.complete(extra, 0, true, 3).report, null);
});
test('refresh controller permits one child and discards cancelled generations', () => {
  const s = model.newSession();
  const first = model.begin(s);
  assert.equal(model.begin(s), null);
  model.teardown(s);
  assert.equal(model.complete(first, 0, true, s.generation).report, null);
  assert.equal(model.begin(s, true), null);
  assert.equal(model.append(first, 'stdout', 'late output'), true);
  assert.equal(first.stdout, '');
  assert.equal(model.reaped(s), true);
  assert.ok(model.begin(s));
  model.teardown(s);
  assert.equal(model.reaped(s), false);
});
test('real child close/reopen before exit queues exactly one replacement child', async () => {
  const s = model.newSession();
  const first = model.begin(s);
  const old = spawn(process.execPath, ['-e', 'process.on("SIGTERM", () => { process.stderr.write("late"); setTimeout(() => process.exit(0), 25) }); process.stdout.write("ready"); setInterval(() => {}, 1000)'], { cwd: '/', env: {} });
  const lateReads = [];
  old.stderr.on('data', data => lateReads.push(model.append(first, 'stderr', data.toString('utf8'))));
  await new Promise(resolve => old.stdout.once('data', resolve));
  model.teardown(s);
  old.kill('SIGTERM');
  assert.equal(model.begin(s, true), null);
  assert.equal(model.begin(s, true), null);
  await new Promise(resolve => old.once('close', resolve));
  assert.deepEqual(lateReads, [true]);
  assert.equal(first.stderrBytes, 0);
  assert.equal(model.complete(first, 0, true, s.generation).report, null);
  assert.equal(model.reaped(s), true);
  const second = model.begin(s);
  assert.ok(second);
  assert.equal(model.begin(s), null);
  const newer = spawn(process.execPath, ['-e', `process.stdout.write(${JSON.stringify(JSON.stringify(report()))})`], { cwd: '/', env: {} });
  newer.stdout.on('data', data => model.append(second, 'stdout', data.toString('utf8')));
  const code = await new Promise(resolve => newer.once('close', resolve));
  assert.ok(model.complete(second, code, true, s.generation).report);
  assert.equal(model.reaped(s), false);
});
async function probeChild(script, timeoutMs = 500, startupDelayMs = 0) {
  const s = model.newSession(); const p = model.begin(s);
  const child = spawn(process.execPath, ['-e',
    `Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ${startupDelayMs}); require('node:fs').writeSync(3, 'ready'); ${script}`],
    { cwd: '/', env: { LANG: 'C.UTF-8' }, stdio: ['ignore', 'pipe', 'pipe', 'pipe'] });
  let timedOut = false;
  const expire = () => {
    timedOut = true; model.teardown(s); child.kill('SIGTERM');
    setTimeout(() => child.kill('SIGKILL'), 20).unref();
  };
  // Match the production 20 s deadline for startup; measure the probe's
  // shorter work deadline only after Node has actually begun executing.
  let timer = setTimeout(expire, 20000);
  child.stdio[3].once('data', () => {
    clearTimeout(timer);
    timer = setTimeout(expire, timeoutMs);
  });
  const outDecoder = new StringDecoder('utf8'); const errDecoder = new StringDecoder('utf8');
  child.stdout.on('data', data => { if (!model.append(p, 'stdout', outDecoder.write(data))) { model.teardown(s); child.kill('SIGTERM'); } });
  child.stderr.on('data', data => { if (!model.append(p, 'stderr', errDecoder.write(data))) { model.teardown(s); child.kill('SIGTERM'); } });
  const result = await new Promise(resolve => child.on('close', code => resolve(model.complete(p, code, true, s.generation))));
  clearTimeout(timer); model.reaped(s);
  return { result, timedOut, p };
}
test('real subprocess probe handles fragmented output, stderr flood and timeout', async () => {
  const r = report(); r.checks[0].reason = 'café'; const json = JSON.stringify(r).replace('é', '\\u00e9');
  // A 650 ms pre-ready delay reproduces the old false timeout at 500 ms.
  const fragment = await probeChild(`const b=Buffer.from(${JSON.stringify(json)}); const i=37; process.stdout.write(b.subarray(0,i+1)); setTimeout(() => { process.stdout.write(b.subarray(i+1)); process.exitCode = 1 }, 10)`, 500, 650);
  assert.equal(fragment.timedOut, false);
  assert.ok(fragment.result.report);
  assert.equal(fragment.result.report.checks[0].reason, 'café');
  const flood = await probeChild(`process.stderr.write('x'.repeat(70000)); process.stdout.write(${JSON.stringify(json)})`);
  assert.equal(flood.result.report, null);
  assert.ok(flood.p.stderrBytes <= 65536);
  const hung = await probeChild('setInterval(() => {}, 1000)', 50);
  assert.equal(hung.timedOut, true);
  assert.equal(hung.result.report, null);
});
