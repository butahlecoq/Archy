const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const path = require('node:path');
const root = path.resolve(__dirname, '../../..');
const model = vm.createContext({});
vm.runInContext(fs.readFileSync(path.join(root, 'integrations/omarchy/plugin/DesktopModel.js'), 'utf8'), model);
const caps = JSON.parse(fs.readFileSync(path.join(root, 'tests/fixtures/desktop/capabilities.json')));
const status = JSON.parse(fs.readFileSync(path.join(root, 'tests/fixtures/desktop/accepted_status.json')));

test('fixed capabilities and bounded verified status accept published fixtures', () => {
  assert.ok(model.validCapabilities(caps));
  assert.ok(model.validStatus(status));
  assert.equal(status.backend_jobs.status, 'unknown');
  assert.equal(status.selected_run.artifacts[0].integrity, 'verified');
});
test('project path is explicit absolute local input and argv preserves spaces', () => {
  assert.equal(model.validManifest('/tmp/My Project/project.yaml'), true);
  for (const bad of ['project.yaml', 'file:///tmp/project.yaml', '/tmp/a\nproject.yaml', '/tmp/a\0b', '/tmp/a'.repeat(1400)]) assert.equal(model.validManifest(bad), false);
  assert.deepEqual(Array.from(model.capabilitiesArgv()), ['/usr/bin/archy','desktop','capabilities','--json']);
  assert.deepEqual(Array.from(model.statusArgv('/tmp/My Project/project.yaml', null, null)), ['/usr/bin/archy','desktop','status','--project','/tmp/My Project/project.yaml','--limit','20','--json']);
});
test('selected run and cursor are exact validated typed arguments', () => {
  const id = status.runs[0].run_id;
  const cursor = 'v1:' + 'a'.repeat(64) + ':5:' + 'b'.repeat(64) + ':20';
  assert.deepEqual(Array.from(model.statusArgv('/tmp/project.yaml', id, cursor)).slice(-5), ['--run', id, '--cursor', cursor, '--json']);
  for (const bad of ['../x', '--help', '', 'x'.repeat(129)]) assert.equal(model.validRunId(bad), false);
  assert.equal(model.validCursor(cursor), true);
  assert.equal(model.validCursor('v1:wrong'), false);
});
test('unknown versions, malformed shapes, unverified artifacts and invented jobs fail', () => {
  const mutate = f => { const c = structuredClone(status); f(c); return c; };
  for (const item of [
    mutate(x => x.schema_version='2'), mutate(x => x.runs[0].operation='open'),
    mutate(x => x.selected_run.artifacts[0].integrity='unchecked'),
    mutate(x => x.backend_jobs.status='running'), mutate(x => x.runs=Array(21).fill(x.runs[0])),
    mutate(x => x.next_cursor='../../escape'), mutate(x => x.project.id='x'.repeat(4097)),
    mutate(x => x.runs[1].revision=x.runs[0].revision),
    mutate(x => x.selected_run.summary.artifact_count=999)
  ]) assert.equal(model.validStatus(item), false);
  const badCaps = structuredClone(caps); badCaps.operations.push({id:'open_report', available:true, reason:null});
  assert.equal(model.validCapabilities(badCaps), false);
});
test('transport rejects non-ASCII, stderr, oversized and stale completions', () => {
  const p = model.newProbe(7, 'status');
  assert.equal(model.append(p, 'stdout', JSON.stringify(status)), true);
  assert.ok(model.complete(p, 0, 0, 7).status);
  assert.equal(model.complete(p, 0, 0, 8).status, null);
  const q = model.newProbe(1, 'status'); model.append(q, 'stdout', JSON.stringify(status)); assert.equal(model.append(q, 'stderr', 'warning'), false);
  assert.equal(model.complete(q, 0, 0, 1).status, null);
  const u = model.newProbe(1, 'status'); assert.equal(model.append(u, 'stdout', 'é'), false);
  const big = model.newProbe(1, 'capabilities'); assert.equal(model.append(big, 'stdout', 'a'.repeat(65537)), false);
});
test('selected run matches the requested page and explicit selector', () => {
  const probe = (document, requestedRunId) => {
    const p = model.newProbe(4, 'status', requestedRunId);
    model.append(p, 'stdout', JSON.stringify(document));
    return model.complete(p, 0, 0, 4).status;
  };
  assert.ok(probe(status, null));
  const wrongDefault = structuredClone(status);
  wrongDefault.selected_run.summary = wrongDefault.runs[1];
  assert.equal(probe(wrongDefault, null), null);
  const wrongSummary = structuredClone(status);
  wrongSummary.selected_run.summary.operation = 'generate';
  assert.equal(probe(wrongSummary, null), null);
  const wrongExplicit = structuredClone(status);
  assert.equal(probe(wrongExplicit, status.runs[1].run_id), null);
  const rightExplicit = structuredClone(status);
  rightExplicit.selected_run.summary = rightExplicit.runs[1];
  assert.ok(probe(rightExplicit, status.runs[1].run_id));
  const offPage = structuredClone(status);
  offPage.runs = [offPage.runs[0]];
  offPage.selected_run.summary = status.runs[1];
  assert.ok(probe(offPage, status.runs[1].run_id));
  const empty = structuredClone(status);
  empty.runs = []; empty.selected_run = null; empty.next_cursor = null;
  assert.ok(probe(empty, null));
  empty.selected_run = status.selected_run;
  assert.equal(probe(empty, null), null);
});
