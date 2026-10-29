const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const crypto = require('node:crypto');
const { spawnSync } = require('node:child_process');
const { inspectBuffer, transformVertex, measureSolid } = require('../reader.cjs');

const fixtureRoot = path.join(__dirname, '..', 'fixtures');
const cases = JSON.parse(fs.readFileSync(path.join(fixtureRoot, 'expected.json'))).cases;
const read = name => fs.readFileSync(path.join(fixtureRoot, name));
const fixture = name => cases.find(item => item.file === name);
const source = item => fs.readFileSync(path.join(fixtureRoot, item.source_file));
const changed = (buffer, from, to) => {
  const text = buffer.toString('utf8');
  assert.ok(text.includes(from), `mutation source missing: ${from}`);
  return Buffer.from(text.replace(from, to));
};

test('column-major web-ifc placement converts Y-up to IFC XYZ once', () => {
  const matrix = [1, 0, 0, 0, 0, 0, -1, 0, 0, 1, 0, 0, 13, 1.6, 2, 1];
  assert.deepEqual(transformVertex([-5, -10, 1.6], matrix), [8, -12, 3.2]);
});

test('mesh measurement rejects invalid indices and an open solid', () => {
  assert.throws(() => measureSolid([[0,0,0], [1,0,0], [0,1,0]], [0,1,3]), /index/);
  assert.throws(() => measureSolid([[0,0,0], [1,0,0], [0,1,0]], [0,1,2]), /closed/);
});

for (const item of cases) {
  test(`independent graph and transformed mesh: ${item.file}`, async () => {
    const report = await inspectBuffer(read(item.file), item, source(item));
    assert.equal(report.status, 'ok');
    assert.equal(report.schema, 'IFC4');
    assert.equal(report.proxies.length, 3);
    assert.equal(report.proxies[0].cap_area_m2, item.footprint_area_m2);
    assert.ok(report.proxies.every(proxy => Math.abs(proxy.signed_volume_m3 - item.footprint_area_m2 * 3.2) < 0.001));
  });
}

test('repeated L exports preserve root GUID inventory', async () => {
  const first = await inspectBuffer(read('l_shape.ifc'), fixture('l_shape.ifc'), source(fixture('l_shape.ifc')));
  const second = await inspectBuffer(read('l_shape_repeat.ifc'), fixture('l_shape_repeat.ifc'), source(fixture('l_shape_repeat.ifc')));
  assert.deepEqual(first.root_guids, second.root_guids);
});

const negative = [
  ['missing active units', 'l_shape.ifc', "#9=IFCUNITASSIGNMENT((#7,#8,#6));", "#9=IFCUNITASSIGNMENT((#7,#8));", /unit/i],
  ['wrong active unit prefix', 'l_shape.ifc', '#6=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);', '#6=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);', /unit/i],
  ['wrong building origin', 'l_shape.ifc', "#24=IFCCARTESIANPOINT((13.,-2.,0.));", "#24=IFCCARTESIANPOINT((14.,-2.,0.));", /placement|bound/i],
  ['wrong storey height', 'l_shape.ifc', "#62=IFCCARTESIANPOINT((0.,0.,3.2));", "#62=IFCCARTESIANPOINT((0.,0.,4.));", /placement|bound/i],
  ['modified DRAFT', 'l_shape.ifc', "IFCLABEL('DRAFT')", "IFCLABEL('APPROVED')", /DRAFT/i],
  ['wrong footprint quantity', 'l_shape.ifc', "IFCQUANTITYAREA('FootprintArea',$,#7,75.", "IFCQUANTITYAREA('FootprintArea',$,#7,76.", /quantity/i],
  ['wrong footprint quantity unit', 'l_shape.ifc', "IFCQUANTITYAREA('FootprintArea',$,#7,75.", "IFCQUANTITYAREA('FootprintArea',$,#6,75.", /unit/i],
  ['missing proxy', 'l_shape.ifc', '#31=IFCBUILDINGELEMENTPROXY(', '#31=IFCWALL(', /proxy|contain|model/i],
  ['extra proxy', 'l_shape.ifc', '#31=IFCBUILDINGELEMENTPROXY(', "#120=IFCBUILDINGELEMENTPROXY('1pjZ7N8p5EDQjTmNq0gxxR',$,'extra',$,$,$,$,$,.USERDEFINED.);\n#31=IFCBUILDINGELEMENTPROXY(", /proxy/i],
  ['duplicate root GUID', 'l_shape.ifc', "#52=IFCBUILDINGELEMENTPROXY('1ze5Qv7LfVZBtp0cKil9B5'", "#52=IFCBUILDINGELEMENTPROXY('0O5DAy1yvL_8qu5jUNXP4C'", /GUID/i],
  ['filled L notch', 'l_shape.ifc', "(5.,5.),(5.,10.)", "(10.,10.),(5.,10.)", /area|notch|mesh|outline/i],
];
for (const [name, file, from, to, message] of negative) {
  test(`rejects ${name}`, async () => {
    await assert.rejects(inspectBuffer(changed(read(file), from, to), fixture(file), source(fixture(file))), message);
  });
}

test('rejects corrupt and truncated IFC', async () => {
  await assert.rejects(inspectBuffer(Buffer.from('broken'), fixture('rectangle.ifc'), source(fixture('rectangle.ifc'))));
  await assert.rejects(inspectBuffer(read('rectangle.ifc').subarray(0, 100), fixture('rectangle.ifc'), source(fixture('rectangle.ifc'))));
});

test('rejects a changed canonical L inner boundary with unchanged area and bounds', async () => {
  const item = structuredClone(fixture('l_shape.ifc'));
  item.footprint_xy_m[3][0] = 19;
  item.footprint_xy_m[4][0] = 19;
  const sourceItem = JSON.parse(source(item));
  sourceItem.footprint_xy_m[3][0] = 19;
  sourceItem.footprint_xy_m[4][0] = 19;
  const sourceBytes = Buffer.from(JSON.stringify(sourceItem) + '\n');
  const oldDigest = item.source_digest;
  item.source_digest = crypto.createHash('sha256').update(sourceBytes).digest('hex');
  const model = Buffer.from(read(item.file).toString('utf8').replaceAll(oldDigest, item.source_digest));
  await assert.rejects(inspectBuffer(model, item, sourceBytes), /footprint outline/i);
});

test('rejects source bytes changed while IFC and expected digest claims agree', async () => {
  const item = fixture('l_shape.ifc');
  const modified = Buffer.from(source(item).toString('utf8').replace('"north_angle_deg":17', '"north_angle_deg":18'));
  assert.notDeepEqual(modified, source(item));
  await assert.rejects(inspectBuffer(read(item.file), item, modified), /source|digest/i);
});

test('rejects matching false IFC and expected digest when source bytes disagree', async () => {
  const item = structuredClone(fixture('l_shape.ifc'));
  const oldDigest = item.source_digest;
  item.source_digest = 'f'.repeat(64);
  const model = Buffer.from(read(item.file).toString('utf8').replaceAll(oldDigest, item.source_digest));
  await assert.rejects(inspectBuffer(model, item, source(item)), /source|digest/i);
});

test('command emits one JSON report and leaves inputs unchanged', () => {
  const model = path.join(fixtureRoot, 'l_shape.ifc');
  const expected = path.join(fixtureRoot, 'expected.json');
  const sourcePath = path.join(fixtureRoot, 'source', 'l_shape.json');
  const before = [fs.readFileSync(model), fs.readFileSync(expected), fs.readFileSync(sourcePath)];
  const run = spawnSync(process.execPath, [path.join(__dirname, '..', 'verify.cjs'), model, expected, 'l_shape.ifc', sourcePath],
    { encoding: 'utf8', timeout: 10000, maxBuffer: 65536 });
  assert.equal(run.status, 0, run.stderr);
  assert.equal(run.stderr, '');
  assert.equal(JSON.parse(run.stdout).status, 'ok');
  assert.deepEqual([fs.readFileSync(model), fs.readFileSync(expected), fs.readFileSync(sourcePath)], before);
});

test('command rejects nonregular and invalid inputs with bounded JSON error', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'archy-ifc-reader-'));
  try {
    const link = path.join(root, 'link.ifc');
    fs.symlinkSync(path.join(fixtureRoot, 'l_shape.ifc'), link);
    for (const file of [link, root, path.join(root, 'missing')]) {
      const run = spawnSync(process.execPath,
        [path.join(__dirname, '..', 'verify.cjs'), file, path.join(fixtureRoot, 'expected.json'), 'l_shape.ifc',
          path.join(fixtureRoot, 'source', 'l_shape.json')],
        { encoding: 'utf8', timeout: 10000, maxBuffer: 65536 });
      assert.equal(run.status, 1);
      assert.equal(JSON.parse(run.stdout).status, 'error');
      assert.ok(run.stdout.length < 2048);
    }
    const sourceLink = path.join(root, 'source.json');
    fs.symlinkSync(path.join(fixtureRoot, 'source', 'l_shape.json'), sourceLink);
    const oversized = path.join(root, 'oversized.json');
    fs.writeFileSync(oversized, Buffer.alloc(64 * 1024 + 1, 32));
    for (const file of [sourceLink, oversized]) {
      const run = spawnSync(process.execPath,
        [path.join(__dirname, '..', 'verify.cjs'), path.join(fixtureRoot, 'l_shape.ifc'),
          path.join(fixtureRoot, 'expected.json'), 'l_shape.ifc', file],
        { encoding: 'utf8', timeout: 10000, maxBuffer: 65536 });
      assert.equal(run.status, 1);
      assert.equal(JSON.parse(run.stdout).status, 'error');
      assert.ok(run.stdout.length < 2048);
    }
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
