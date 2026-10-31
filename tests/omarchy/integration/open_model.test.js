const {test} = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const path = require('node:path');
const root = path.resolve(__dirname, '../../..');
const desktop = vm.createContext({});
vm.runInContext(fs.readFileSync(path.join(root,'integrations/omarchy/plugin/DesktopModel.js'),'utf8'), desktop);
const open = vm.createContext({DesktopModel:desktop});
const source = fs.readFileSync(path.join(root,'integrations/omarchy/plugin/OpenModel.js'),'utf8').replace(/^\.import[^\n]*\n/,'');
vm.runInContext(source, open);
const status = JSON.parse(fs.readFileSync(path.join(root,'tests/fixtures/desktop/accepted_status.json')));
const renderStatus = JSON.parse(fs.readFileSync(path.join(root,'tests/fixtures/desktop/render_status.json')));

test('disclosed render summary keeps fixed selected-run opener binding', () => {
  const disclosed = structuredClone(renderStatus);
  const preview = disclosed.selected_run.artifacts.find(a => a.kind === 'preview');
  const symbol = {
    variant_id: preview.id.split('/')[1], views: ['top'],
    notice: 'Screen-width massing edges are nonphysical preview symbols; GLB dimensions remain exact.'
  };
  disclosed.runs[0].preview_symbols = [symbol];
  disclosed.selected_run.summary.preview_symbols = [symbol];
  assert.deepEqual(Array.from(open.argv('/tmp/project.yaml', disclosed, 'preview', preview.id)),
    ['/usr/bin/archy','desktop','open','--project','/tmp/project.yaml','--run',disclosed.selected_run.summary.run_id,'--artifact',preview.id,'--kind','preview','--json']);
  disclosed.selected_run.summary.preview_symbols = [];
  assert.equal(open.argv('/tmp/project.yaml', disclosed, 'preview', preview.id), null);
});

test('captured render views and scene bind to selected verified run with fixed argv', () => {
  assert.equal(desktop.validStatus(renderStatus), true);
  for (const artifact of renderStatus.selected_run.artifacts.filter(a => a.kind === 'preview' || a.kind === 'blender')) {
    const args = Array.from(open.argv('/tmp/project.yaml', renderStatus, artifact.kind, artifact.id));
    assert.deepEqual(args, ['/usr/bin/archy','desktop','open','--project','/tmp/project.yaml','--run',renderStatus.selected_run.summary.run_id,'--artifact',artifact.id,'--kind',artifact.kind,'--json']);
  }
  for (const bad of ['renders/../view-top.png','renders/variant-bad/view-top.png','renders/variant-'+'a'.repeat(64)+'/evil.png','renders/variant-'+'a'.repeat(64)+'/../scene.blend'])
    assert.equal(open.argv('/tmp/project.yaml',renderStatus,'preview',bad),null);
  const scene=renderStatus.selected_run.artifacts.find(a=>a.kind==='blender').id;
  assert.equal(open.argv('/tmp/project.yaml',renderStatus,'preview',scene),null);
  const forged=structuredClone(renderStatus);
  forged.selected_run.artifacts.find(a=>a.id===scene).kind='run_data';
  assert.equal(open.argv('/tmp/project.yaml',forged,'blender',scene),null);
  const missing=structuredClone(renderStatus);
  missing.selected_run.artifacts=missing.selected_run.artifacts.filter(a=>a.id!==scene);
  missing.selected_run.summary.artifact_count--;
  missing.runs[0].artifact_count--;
  assert.equal(open.argv('/tmp/project.yaml',missing,'blender',scene),null);
});

test('open argv binds exact selected run and verified report', () => {
  const artifact = status.selected_run.artifacts.find(a=>a.id==='report.md').id;
  assert.deepEqual(Array.from(open.argv('/tmp/My Project/project.yaml',status,'report',artifact)),
    ['/usr/bin/archy','desktop','open','--project','/tmp/My Project/project.yaml','--run',status.selected_run.summary.run_id,'--artifact','report.md','--kind','report','--json']);
});
test('injected paths, mismatched selection, missing previews and forged artifacts refuse', () => {
  for (const artifact of ['--help','../report.md','report.md;id','file:///tmp/x','previews/../../x.png'])
    assert.equal(open.argv('/tmp/project.yaml',status,'report',artifact),null);
  assert.equal(open.argv('/tmp/project.yaml',status,'preview','previews/fake.png'),null);
  const forged=structuredClone(status); forged.selected_run.artifacts.push({id:'report.md;id',kind:'report',integrity:'verified',byte_len:1,sha256:'a'.repeat(64)});
  assert.equal(open.argv('/tmp/project.yaml',forged,'report','report.md;id'),null);
  const wrong=structuredClone(status); wrong.selected_run.summary.run_id=wrong.runs[1].run_id;
  assert.equal(open.argv('/tmp/project.yaml',wrong,'report','report.md'),null);
});
test('malformed, mismatched and failed receipts are unavailable', () => {
  const run=status.selected_run.summary.run_id;
  const sha=status.selected_run.artifacts.find(a=>a.id==='report.md').sha256;
  const good={schema_version:'archy-desktop-open-v1',status:'handed_to_viewer',run_id:run,artifact_id:'report.md',kind:'report',sha256:sha};
  assert.equal(open.complete(JSON.stringify(good),0,0,run,'report.md','report',sha),true);
  for (const text of ['{',JSON.stringify({...good,run_id:'other'}),JSON.stringify({...good,status:'rendered'}),JSON.stringify({...good,command:'sh'})])
    assert.equal(open.complete(text,0,0,run,'report.md','report',sha),false);
  assert.equal(open.complete(JSON.stringify({...good,sha256:'b'.repeat(64)}),0,0,run,'report.md','report',sha),false);
  assert.equal(open.complete(JSON.stringify(good),1,0,run,'report.md','report',sha),false);
  assert.equal(open.complete('x'.repeat(8193),0,0,run,'report.md','report',sha),false);
});
