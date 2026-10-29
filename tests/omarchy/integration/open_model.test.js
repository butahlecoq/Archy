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
