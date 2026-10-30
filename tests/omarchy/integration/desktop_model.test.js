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
const renderStatus = JSON.parse(fs.readFileSync(path.join(root, 'tests/fixtures/desktop/render_status.json')));
// Captured from generate --count 90, evaluate --all, render --all, desktop status --run.
const largeRenderStatus = JSON.parse(fs.readFileSync(path.join(root, 'tests/fixtures/desktop/render_status_90_variants.json')));

test('verified render preview disclosure survives the selected-run desktop contract', () => {
  const disclosed = structuredClone(renderStatus);
  const symbol = {
    variant_id: disclosed.selected_run.artifacts.find(a => a.id.startsWith('renders/variant-')).id.split('/')[1],
    views: ['top'],
    notice: 'Screen-width massing edges are nonphysical preview symbols; GLB dimensions remain exact.'
  };
  disclosed.runs[0].preview_symbols = [symbol];
  disclosed.selected_run.summary.preview_symbols = [symbol];
  assert.equal(model.validStatus(disclosed), true);
  disclosed.selected_run.summary.preview_symbols = [];
  assert.equal(model.validStatus(disclosed), false);
  for (const edit of [
    x => { x.runs[0].preview_symbols[0].views = ['top', 'top']; x.selected_run.summary.preview_symbols[0].views = ['top', 'top']; },
    x => { x.runs[0].preview_symbols[0].notice = 'verified'; x.selected_run.summary.preview_symbols[0].notice = 'verified'; },
  ]) {
    const bad = structuredClone(renderStatus);
    bad.runs[0].preview_symbols = [structuredClone(symbol)];
    bad.selected_run.summary.preview_symbols = [structuredClone(symbol)];
    edit(bad);
    assert.equal(model.validStatus(bad), false);
  }
});

test('preview symbols are render-only and bounded by variant count', () => {
  const nonRender = structuredClone(status);
  nonRender.runs[0].preview_symbols = [];
  nonRender.selected_run.summary.preview_symbols = [];
  assert.equal(model.validStatus(nonRender), false);
  const render = structuredClone(renderStatus);
  const symbol = {
    variant_id: 'variant-' + 'a'.repeat(64),
    views: ['top'],
    notice: 'Screen-width massing edges are nonphysical preview symbols; GLB dimensions remain exact.'
  };
  render.runs[0].variant_count = 0;
  render.selected_run.summary.variant_count = 0;
  render.runs[0].preview_symbols = [symbol];
  render.selected_run.summary.preview_symbols = [symbol];
  assert.equal(model.validStatus(render), false);
  render.runs[0].variant_count = 1;
  render.selected_run.summary.variant_count = 1;
  assert.equal(model.validStatus(render), true);
});

test('captured 90-variant Rust status is readable through the bounded desktop probe', () => {
  const selected = largeRenderStatus.selected_run;
  assert.equal(selected.summary.variant_count, 90);
  assert.equal(selected.summary.artifact_count, 274);
  assert.equal(selected.artifacts.length, 274);
  const bytes = fs.readFileSync(path.join(root, 'tests/fixtures/desktop/render_status_90_variants.json'), 'utf8');
  assert.equal(Buffer.byteLength(bytes), 66298);
  const p = model.newProbe(1, 'status', selected.summary.run_id);
  assert.equal(model.append(p, 'stdout', bytes), true);
  assert.equal(model.complete(p, 0, 0, 1).status?.selected_run.summary.artifact_count, 274);
});

test('accepted-run limits allow the last legal artifact and reject overflow', () => {
  const atLimit = structuredClone(largeRenderStatus);
  while (atLimit.selected_run.artifacts.length < 4096) {
    atLimit.selected_run.artifacts.push({
      id: 'extra/' + atLimit.selected_run.artifacts.length,
      kind: 'run_data', byte_len: 0, sha256: 'a'.repeat(64), integrity: 'verified'
    });
  }
  atLimit.runs[0].variant_count = 1000;
  atLimit.selected_run.summary.variant_count = 1000;
  atLimit.runs[0].artifact_count = 4096;
  atLimit.selected_run.summary.artifact_count = 4096;
  assert.equal(model.validStatus(atLimit), true);
  const tooManyVariants = structuredClone(atLimit);
  tooManyVariants.runs[0].variant_count = tooManyVariants.selected_run.summary.variant_count = 1001;
  assert.equal(model.validStatus(tooManyVariants), false);
  atLimit.selected_run.artifacts.push({
    id: 'extra/4096', kind: 'run_data', byte_len: 0, sha256: 'a'.repeat(64), integrity: 'verified'
  });
  atLimit.runs[0].artifact_count = atLimit.selected_run.summary.artifact_count = 4097;
  assert.equal(model.validStatus(atLimit), false);
});

test('captured render fixture and failed status keep exact typed summaries', () => {
  assert.equal(model.validStatus(renderStatus), true);
  const failed = structuredClone(renderStatus);
  failed.runs[0].render_outcome = 'failed';
  failed.selected_run.summary.render_outcome = 'failed';
  failed.selected_run.artifacts = [];
  failed.runs[0].artifact_count = 0;
  failed.selected_run.summary.artifact_count = 0;
  assert.equal(model.validStatus(failed), true);
  for (const mutate of [
    x => { delete x.runs[0].render_outcome; delete x.selected_run.summary.render_outcome; },
    x => { x.runs[0].render_outcome = 'invented'; x.selected_run.summary.render_outcome = 'invented'; },
    x => { x.runs[1].render_outcome = 'failed'; },
    x => { x.selected_run.summary.render_outcome = 'partial'; },
  ]) { const bad = structuredClone(renderStatus); mutate(bad); assert.equal(model.validStatus(bad), false); }
});

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
  const statusLimit = model.newProbe(1, 'status');
  assert.equal(model.append(statusLimit, 'stdout', 'a'.repeat(33554432)), true);
  assert.equal(model.append(statusLimit, 'stdout', 'a'), false);
  assert.equal(model.complete(statusLimit, 0, 0, 1).status, null);
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
