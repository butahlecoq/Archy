const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const crypto = require('node:crypto');
const web = require('web-ifc');
const { spawnSync } = require('node:child_process');
const { inspectBuffer, transformVertex, measureSolid } = require('../reader.cjs');
const { checkStepScope } = require('../step-scope.cjs');

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
const splitStepFields = input => {
  const parts = []; let start = 0, depth = 0, quoted = false;
  for (let i = 0; i < input.length; i++) {
    if (input[i] === "'") {
      if (quoted && input[i + 1] === "'") { i++; continue; }
      quoted = !quoted;
    } else if (!quoted) {
      if (input[i] === '(') depth++;
      if (input[i] === ')') depth--;
      if (input[i] === ',' && depth === 0) { parts.push(input.slice(start, i)); start = i + 1; }
    }
  }
  parts.push(input.slice(start));
  return parts;
};
const appended = (buffer, from, to, entity) => changed(changed(buffer, from, to),
  'ENDSEC;\nEND-ISO-10303-21;', `${entity}\nENDSEC;\nEND-ISO-10303-21;`);
const bridge = (model, item) => {
  const parts = [Buffer.from(JSON.stringify(item)), source(item), model];
  const lengths = Buffer.alloc(12);
  parts.forEach((part, index) => lengths.writeUInt32BE(part.length, index * 4));
  const run = spawnSync(process.execPath, [path.join(__dirname, '..', 'verify-stdin.cjs')],
    { input: Buffer.concat([lengths, ...parts]), encoding: 'utf8', timeout: 60000, maxBuffer: 65536 });
  assert.notEqual(run.status, null, `bridge timeout: ${run.error}`);
  return { status: run.status, report: JSON.parse(run.stdout) };
};

test('framed verifier rejects reviewer comment claims in both shapes', () => {
  const observed = [];
  for (const file of ['rectangle.ifc', 'l_shape.ifc']) {
    const item = fixture(file), valid = read(file);
    assert.equal(bridge(valid, item).report.status, 'ok');
    for (const [from, to] of [
      ['HEADER;', 'HEADER;\n/* Certified architect; approved for construction */'],
      ["FILE_NAME('model.ifc'", "FILE_NAME(/* Certified architect */'model.ifc'"],
    ]) {
      const result = bridge(changed(valid, from, to), item);
      observed.push([file, result.status, result.report.code]);
    }
  }
  assert.deepEqual(observed.map(([, status, code]) => [status, code]),
    Array.from({ length: 4 }, () => [1, 'ARTIFACT_INVALID']), JSON.stringify(observed));
});

test('STEP lexer rejects comments throughout the file and preserves quoted markers', async () => {
  for (const file of ['rectangle.ifc', 'l_shape.ifc']) {
    const valid = read(file), item = fixture(file);
    const comments = [
      Buffer.concat([Buffer.from('/* before file */'), valid]),
      changed(valid, 'HEADER;', 'HEADER;/* header */'),
      changed(valid, "FILE_NAME('model.ifc'", "FILE_NAME(/* argument */'model.ifc'"),
      changed(valid, 'ENDSEC;\nDATA;', 'ENDSEC;/* boundary */\nDATA;'),
      changed(valid, 'DATA;\n', 'DATA;\n/* data */'),
      changed(valid, '#1=IFCPROJECT(', '#1=IFCPROJECT(/* field */'),
      changed(valid, 'ENDSEC;\nEND-ISO-10303-21;', 'ENDSEC;/* end */\nEND-ISO-10303-21;'),
      Buffer.concat([valid, Buffer.from('/* trailing */')]),
      changed(valid, 'DATA;\n', 'DATA;\n/**/'),
      changed(valid, 'DATA;\n', 'DATA;\n/* never closed'),
    ];
    for (const model of comments) assert.throws(() => checkStepScope(model), /STEP comment/, file);
    for (const model of [comments[4], comments[7]]) {
      const result = bridge(model, item);
      assert.equal(result.status, 1, `${file}: ${JSON.stringify(result.report)}`);
      assert.equal(result.report.code, 'ARTIFACT_INVALID');
    }
    for (const literal of ["'/*'", "'*/'", "'/* not a comment */'", "'can''t /* quote */'"]) {
      const model = changed(valid, "IFCLABEL('DRAFT')", `IFCLABEL(${literal})`);
      assert.doesNotThrow(() => checkStepScope(model), `${file}: ${literal}`);
      await assert.rejects(inspectBuffer(model, item, source(item)), /DRAFT|source property/);
    }
    const afterLiteral = changed(valid, "IFCLABEL('DRAFT')", "IFCLABEL('/*')/* actual comment */");
    assert.throws(() => checkStepScope(afterLiteral), /STEP comment/);
  }
});

test('framed verifier rejects the six reviewer HEADER authority claims', () => {
  const mutations = [
    ["'ViewDefinition[DesignTransferView]'", "'Approved for construction'"],
    ["FILE_NAME('model.ifc'", "FILE_NAME('Certified and approved'"],
    ["(''),(''),'IfcOpenShell", "('Certified architect'),(''),'IfcOpenShell"],
  ];
  const observed = [];
  for (const file of ['rectangle.ifc', 'l_shape.ifc']) {
    const item = fixture(file), valid = read(file);
    assert.equal(bridge(valid, item).report.status, 'ok');
    for (const [from, to] of mutations) {
      const result = bridge(changed(valid, from, to), item);
      observed.push([file, to, result.status, result.report.code]);
    }
  }
  assert.deepEqual(observed.map(([, , status, code]) => [status, code]),
    Array.from({ length: 6 }, () => [1, 'ARTIFACT_INVALID']), JSON.stringify(observed));
});

test('STEP HEADER profile fixes every field, its order, arity, and syntax', () => {
  const mutations = [
    ["'ViewDefinition[DesignTransferView]'", "'Certified'"],
    ["'2;1'", "'Approved'"],
    ["FILE_NAME('model.ifc'", "FILE_NAME('approved.ifc'"],
    ["(''),(''),'IfcOpenShell", "('Architect'),(''),'IfcOpenShell"],
    ["(''),'IfcOpenShell", "('Certified organization'),'IfcOpenShell"],
    ["'IfcOpenShell 0.9.0.post1'", "'Certified exporter'"],
    ["'IfcOpenShell 0.9.0.post1','Nobody'", "'IfcOpenShell 0.9.0.post1','Approved system'"],
    ["'Nobody'", "'Certified authority'"],
    ["'IFC4'", "'IFC2X3'"],
    ["FILE_DESCRIPTION(('ViewDefinition[DesignTransferView]'),'2;1')",
      "FILE_DESCRIPTION(('ViewDefinition[DesignTransferView]'),'2;1','Approved')"],
    ["(''),(''),'IfcOpenShell", "(''),(''),(''),'IfcOpenShell"],
    ["'IfcOpenShell 0.9.0.post1','Nobody'", "'Nobody','IfcOpenShell 0.9.0.post1'"],
    ["FILE_SCHEMA(('IFC4'))", "FILE_SCHEMA('IFC4')"],
    ["FILE_NAME('model.ifc'", "FILE_NAME(IFCLABEL('model.ifc')"],
    ["FILE_NAME('model.ifc'", "FILE_NAME('model.ifc','extra'"],
    ["'2026-10-29T14:43:23.203944+00:00'", "'2026-02-30T14:43:23.203944+00:00'"],
    ["'2026-10-29T14:43:23.203944+00:00'", "'2026-10-29T14:43:23Z;Approved'"],
  ];
  for (const file of ['rectangle.ifc', 'l_shape.ifc']) {
    const valid = read(file);
    const stamp = checkStepScope(valid).headerTimestamp;
    assert.match(stamp, /^2026-10-29T14:43:23\.\d{6}\+00:00$/);
    for (const [from, to] of mutations) {
      assert.throws(() => checkStepScope(changed(valid, from.replaceAll('2026-10-29T14:43:23.203944+00:00', stamp), to)), /HEADER|STEP|timestamp/,
        `${file}: ${from} -> ${to}`);
    }
    const formatted = changed(valid, 'HEADER;\nFILE_DESCRIPTION', 'HEADER;\n \tfile_description')
      .toString('utf8').replace("FILE_SCHEMA(('IFC4'));", "File_Schema ( ( 'IFC4' ) ) ;");
    assert.equal(checkStepScope(Buffer.from(formatted)).headerTimestamp, stamp);
  }
});

test('framed verifier binds a legitimate dynamic HEADER timestamp to export provenance', () => {
  const newStamp = '2026-10-30T02:03:04.123456+00:00';
  for (const file of ['rectangle.ifc', 'l_shape.ifc']) {
    const item = fixture(file), valid = read(file);
    const oldStamp = checkStepScope(valid).headerTimestamp;
    const changedHeader = changed(valid, `FILE_NAME('model.ifc','${oldStamp}'`,
      `FILE_NAME('model.ifc','${newStamp}'`);
    const mismatch = bridge(changedHeader, item);
    assert.equal(mismatch.status, 1, `${file}: ${JSON.stringify(mismatch.report)}`);
    assert.match(mismatch.report.message, /HEADER timestamp/);
    const matching = changed(changedHeader, `IFCLABEL('${oldStamp}')`, `IFCLABEL('${newStamp}')`);
    assert.equal(bridge(matching, item).report.status, 'ok', file);
  }
});

test('normalized accepted source binds original JSON and YAML bytes without text interpretation', async () => {
  for (const format of ['json', 'yaml']) {
    const original = fixture('rectangle.ifc');
    const bytes = fs.readFileSync(path.join(__dirname, '../../../fixtures/briefs', `courtyard-v2.${format}`));
    const digest = crypto.createHash('sha256').update(bytes).digest('hex');
    const expected = { ...original, source_mode: 'accepted_project_normalized_v2',
      source_file: `input.${format}`, source_digest: digest };
    const old = read(original.file).toString('utf8');
    const cleared = old.replace(/('SourceArtifacts',\$,IFCLABEL\(')\[[^\n]*?\]('\))/, '$1[]$2');
    assert.notEqual(cleared, old);
    const model = Buffer.from(cleared.replaceAll(original.source_digest, digest));
    assert.equal((await inspectBuffer(model, expected, bytes)).status, 'ok');
    assert.equal(bridge(model, { ...expected, source_file: original.source_file }).report.code, 'ARTIFACT_INVALID');
    await assert.rejects(inspectBuffer(model, expected, Buffer.concat([bytes, Buffer.from('\n')])), /digest/);
  }
});

test('bridge rejects ambiguous reachable provenance, quantities, and contexts', () => {
  const item = fixture('rectangle.ifc');
  const valid = read(item.file);
  assert.equal(bridge(valid, item).report.status, 'ok');
  const cases = [
    ['approved first', '#95,#96,#97', '#999,#95,#96,#97', "#999=IFCPROPERTYSINGLEVALUE('ApprovalState',$,IFCLABEL('APPROVED'),$);"],
    ['approved last', '#107,#108,#109', '#107,#108,#109,#999', "#999=IFCPROPERTYSINGLEVALUE('ApprovalState',$,IFCLABEL('APPROVED'),$);"],
    ['same approval', '#95,#96,#97', '#999,#95,#96,#97', "#999=IFCPROPERTYSINGLEVALUE('ApprovalState',$,IFCLABEL('DRAFT'),$);"],
    ['extra property', '#95,#96,#97', '#999,#95,#96,#97', "#999=IFCPROPERTYSINGLEVALUE('OtherClaim',$,IFCLABEL('x'),$);"],
    ['wrong area first', '(#110,#111,#112)', '(#999,#110,#111,#112)', "#999=IFCQUANTITYAREA('FootprintArea',$,#7,999.,'wrong');"],
    ['wrong area last', '(#110,#111,#112)', '(#110,#111,#112,#999)', "#999=IFCQUANTITYAREA('FootprintArea',$,#7,999.,'wrong');"],
    ['same area', '(#110,#111,#112)', '(#999,#110,#111,#112)', "#999=IFCQUANTITYAREA('FootprintArea',$,#7,200.,'same');"],
    ['extra quantity', '(#110,#111,#112)', '(#110,#111,#112,#999)', "#999=IFCQUANTITYAREA('OtherArea',$,#7,1.,'extra');"],
    ['extra context', '(#14),#9', '(#14,#999),#9', "#999=IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-05,#13,$);"],
    ['dangling context', '(#14),#9', '(#14,#999),#9', null],
  ];
  for (const [name, from, to, entity] of cases) {
    const model = entity ? appended(valid, from, to, entity) : changed(valid, from, to);
    const result = bridge(model, item);
    assert.equal(result.status, 1, `${name}: ${JSON.stringify(result.report)}`);
    assert.equal(result.report.code, 'ARTIFACT_INVALID', name);
  }
});

test('framed reader binds valid-looking root GUIDs to exporter roles', () => {
  for (const file of ['rectangle.ifc', 'l_shape.ifc']) {
    const item = fixture(file);
    const original = read(file);
    const line = original.toString('utf8').split('\n').find(value => /^#4=IFCRELAGGREGATES\(/.test(value));
    assert.ok(line);
    const forged = changed(original, line, line.replace(/'[^']+'/, "'2Certified000000000000'"));
    const result = bridge(forged, item);
    assert.equal(result.status, 1, `${file}: ${JSON.stringify(result.report)}`);
    assert.equal(result.report.code, 'ARTIFACT_INVALID');
    assert.match(result.report.message, /root GUID/);
  }
});

test('STEP scope enforces every allowed entity arity and exporter derived slot', () => {
  for (const file of ['rectangle.ifc', 'l_shape.ifc']) {
    const original = read(file);
    const lines = original.toString('utf8').split('\n').filter(line => /^#\d+=IFC[A-Z0-9]+\(/.test(line));
    const firstByType = new Map(lines.map(line => [line.match(/^#\d+=(IFC[A-Z0-9]+)\(/)[1], line]));
    assert.equal(firstByType.size, 27, `${file}: allowed entity inventory`);
    for (const [type, line] of firstByType) {
      const head = line.slice(0, line.indexOf('(') + 1);
      const args = splitStepFields(line.slice(head.length, -2));
      const extra = changed(original, line, `${head}${[...args, "'Certified'"].join(',')});`);
      const missing = changed(original, line, `${head}${args.slice(0, -1).join(',')});`);
      assert.throws(() => checkStepScope(extra), /arity/, `${file} ${type} extra field`);
      assert.throws(() => checkStepScope(missing), /arity/, `${file} ${type} missing field`);
    }
    const derived = changed(original,
      "IFCGEOMETRICREPRESENTATIONSUBCONTEXT('Body','Model',*,*,*,*",
      "IFCGEOMETRICREPRESENTATIONSUBCONTEXT('Body','Model','Certified',*,*,*");
    assert.throws(() => checkStepScope(derived), /derived field/);
    const result = bridge(derived, fixture(file));
    assert.equal(result.status, 1, `${file}: ${JSON.stringify(result.report)}`);
    assert.equal(result.report.code, 'ARTIFACT_INVALID');
  }
});

test('framed reader enforces the exporter model and body context profile', () => {
  for (const file of ['rectangle.ifc', 'l_shape.ifc']) {
    const item = fixture(file);
    const valid = read(file);
    assert.equal(bridge(valid, item).report.status, 'ok');
    const mutations = [
      ['true north', buffer => appended(buffer, "1.E-05,#13,$);", "1.E-05,#13,#999);", '#999=IFCDIRECTION((1.,0.));')],
      ['precision', buffer => changed(buffer, "1.E-05,#13,$);", "1.,#13,$);")],
      ['context identifier', buffer => changed(buffer, "#14=IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model'", "#14=IFCGEOMETRICREPRESENTATIONCONTEXT('Other','Model'")],
      ['world origin', buffer => changed(buffer, '#10=IFCCARTESIANPOINT((0.,0.,0.));', '#10=IFCCARTESIANPOINT((1.,0.,0.));')],
      ['world orientation', buffer => changed(buffer, '#12=IFCDIRECTION((1.,0.,0.));', '#12=IFCDIRECTION((0.,1.,0.));')],
      ['body target scale', buffer => changed(buffer, '#14,$,.MODEL_VIEW.,$);', '#14,2.,.MODEL_VIEW.,$);')],
      ['body target label', buffer => changed(buffer, '#14,$,.MODEL_VIEW.,$);', "#14,$,.MODEL_VIEW.,'Other');")],
      ['profile purpose', buffer => changed(buffer, "#18=IFCARBITRARYCLOSEDPROFILEDEF(.AREA.,'ArchyCanonicalFootprint'", "#18=IFCARBITRARYCLOSEDPROFILEDEF(.AREA.,'OtherFootprint'")],
      ['curve self intersection', buffer => changed(buffer, '#17=IFCINDEXEDPOLYCURVE(#16,$,$);', '#17=IFCINDEXEDPOLYCURVE(#16,$,.T.);')],
      ['project name', buffer => changed(buffer, `${item.project_id}:project`, `${item.project_id}:other`)],
      ['project description', buffer => changed(buffer, `${item.project_id}:project',$,$`, `${item.project_id}:project','APPROVED',$`)],
      ['site geolocation', buffer => changed(buffer, '#23,$,$,$,$,$,$,$,$);', '#23,$,$,$,(42,0,0),$,$,$,$);')],
      ['proxy description', buffer => changed(buffer, 'Schematic extruded storey mass; DRAFT; no detailed construction semantics', 'Certified detailed construction')],
      ['quantity method', buffer => changed(buffer, 'task02-analytic-massing-v1; approximate, not net or certified', 'certified net measurement')],
      ['quantity formula', buffer => changed(buffer, 'canonical footprint shoelace; m2', 'certified survey; m2')],
      ['quantity description', buffer => changed(buffer, "IFCQUANTITYAREA('FootprintArea',$", "IFCQUANTITYAREA('FootprintArea','certified'")],
    ];
    for (const [name, mutate] of mutations) {
      const result = bridge(mutate(valid), item);
      assert.equal(result.status, 1, `${file} ${name}: ${JSON.stringify(result.report)}`);
      assert.equal(result.report.code, 'ARTIFACT_INVALID', `${file} ${name}`);
    }
    const equivalent = changed(valid, '#12=IFCDIRECTION((1.,0.,0.));', '#12=IFCDIRECTION((1.0000001,0.,0.));');
    assert.equal(bridge(equivalent, item).report.status, 'ok', file);
  }
});

// Field names come from the IFC4 entity schemas exposed by web-ifc. Mutations
// use the first active instance of each type and leave source bytes untouched.
test('framed reader rejects claims in every reachable semantic metadata slot', async () => {
  const fields = {
    IFCAXIS2PLACEMENT3D: [],
    IFCCARTESIANPOINT: [],
    IFCCARTESIANPOINTLIST2D: [],
    IFCDIRECTION: [],
    IFCEXTRUDEDAREASOLID: [],
    IFCLOCALPLACEMENT: [],
    IFCUNITASSIGNMENT: [],
    IFCSIUNIT: ['Prefix'],
    IFCINDEXEDPOLYCURVE: ['Segments', 'SelfIntersect'],
    IFCPROJECT: ['Name', 'Description', 'ObjectType', 'LongName', 'Phase'],
    IFCSITE: ['Name', 'Description', 'ObjectType', 'LongName', 'CompositionType', 'RefLatitude',
      'RefLongitude', 'RefElevation', 'LandTitleNumber', 'SiteAddress'],
    IFCBUILDING: ['Name', 'Description', 'ObjectType', 'LongName', 'CompositionType',
      'ElevationOfRefHeight', 'ElevationOfTerrain', 'BuildingAddress'],
    IFCBUILDINGSTOREY: ['Name', 'Description', 'ObjectType', 'LongName', 'CompositionType'],
    IFCBUILDINGELEMENTPROXY: ['Name', 'Description', 'ObjectType', 'Tag'],
    IFCRELAGGREGATES: ['Name', 'Description'],
    IFCRELCONTAINEDINSPATIALSTRUCTURE: ['Name', 'Description'],
    IFCRELDEFINESBYPROPERTIES: ['Name', 'Description'],
    IFCPRODUCTDEFINITIONSHAPE: ['Name', 'Description'],
    IFCPROPERTYSET: ['Name', 'Description'],
    IFCELEMENTQUANTITY: ['Name', 'Description', 'MethodOfMeasurement'],
    IFCPROPERTYSINGLEVALUE: ['Name', 'Description'],
    IFCQUANTITYAREA: ['Name', 'Description', 'Formula'],
    IFCQUANTITYVOLUME: ['Name', 'Description', 'Formula'],
    IFCARBITRARYCLOSEDPROFILEDEF: ['ProfileName'],
    IFCGEOMETRICREPRESENTATIONCONTEXT: ['ContextIdentifier', 'ContextType'],
    IFCGEOMETRICREPRESENTATIONSUBCONTEXT: ['ContextIdentifier', 'ContextType', 'UserDefinedTargetView'],
    IFCSHAPEREPRESENTATION: ['RepresentationIdentifier', 'RepresentationType'],
  };
  for (const file of ['rectangle.ifc', 'l_shape.ifc']) {
    const item = fixture(file), original = read(file);
    const fixtureTypes = new Set([...original.toString('utf8').matchAll(/^#\d+=(IFC[A-Z0-9]+)\(/gm)]
      .map(match => match[1]));
    assert.deepEqual([...fixtureTypes].sort(), Object.keys(fields).sort(), `${file}: entity schema inventory`);
    const api = new web.IfcAPI(); await api.Init();
    const model = api.OpenModel(new Uint8Array(original));
    try {
      for (const [type, names] of Object.entries(fields)) {
        const ids = api.GetLineIDsWithType(model, web[type]);
        assert.ok(ids.size() > 0, `${file}: missing ${type}`);
        const id = ids.get(0), entity = api.GetLine(model, id);
        const keys = Object.keys(entity).filter(key => key !== 'type' && key !== 'expressID');
        const line = original.toString('utf8').split('\n').find(value => value.startsWith(`#${id}=${type}(`));
        assert.ok(line, `${file}: missing STEP line ${type}`);
        const args = splitStepFields(line.slice(line.indexOf('(') + 1, -2));
        assert.equal(args.length, keys.length, `${file} ${type}: schema fields`);
        for (const name of names) {
          const index = keys.indexOf(name);
          assert.ok(index >= 0, `${type}.${name} absent from schema`);
          const mutated = [...args];
          mutated[index] = "'Certified and Approved for construction'";
          const altered = Buffer.from(original.toString('utf8').replace(line,
            `${line.slice(0, line.indexOf('(') + 1)}${mutated.join(',')});`));
          const result = bridge(altered, item);
          assert.equal(result.status, 1, `${file} ${type}.${name}: ${JSON.stringify(result.report)}`);
          assert.equal(result.report.code, 'ARTIFACT_INVALID', `${file} ${type}.${name}`);
        }
      }
    } finally { api.CloseModel(model); api.Dispose(); }
  }
});

test('STEP scope consumes the whole file and rejects unsupported entities', () => {
  const valid = read('rectangle.ifc');
  const text = valid.toString('utf8');
  for (const body of [
    Buffer.concat([valid, Buffer.from('\n<script>alert(1)</script>\n')]),
    changed(valid, 'ENDSEC;\nEND-ISO-10303-21;', "#999=IFCEXTERNALREFERENCE('https://attacker.example/ifc',$,$);\nENDSEC;\nEND-ISO-10303-21;"),
    changed(valid, "FILE_SCHEMA(('IFC4'))", "FILE_SCHEMA(('IFC2X3'))"),
    changed(valid, 'END-ISO-10303-21;', 'END-ISO-10303-21;\nDATA;'),
    changed(valid, '#1=IFCPROJECT(', '#1=IFCEXTERNALREFERENCE('),
  ]) assert.throws(() => checkStepScope(body), /STEP|unsupported/i);
  checkStepScope(valid);
  checkStepScope(Buffer.from(text.replace('DATA;\n', " \nDaTa;\n")
    .replace('#1=IFCPROJECT(', '#1 =\nifcproject (')));
});

test('rejects comments after the final STEP terminator in the actual model bytes', async () => {
  const item = fixture('rectangle.ifc');
  const model = Buffer.concat([read(item.file), Buffer.from("/* <script>alert(1)</script> */")]);
  await assert.rejects(inspectBuffer(model, item, source(item)), /STEP comment/);
  checkStepScope(Buffer.concat([read(item.file), Buffer.from(' \r\n\t')]));
});

test('rejects an unreferenced allowed DATA entity in the actual model bytes', async () => {
  const item = fixture('l_shape.ifc');
  const model = changed(read(item.file), 'ENDSEC;\nEND-ISO-10303-21;',
    '#999=IFCCARTESIANPOINT((1.,2.,3.));\nENDSEC;\nEND-ISO-10303-21;');
  await assert.rejects(inspectBuffer(model, item, source(item)), /unreachable STEP entity/);
});

test('STEP scope bounds bytes, tokens, nesting, and unterminated comments', () => {
  const valid = read('rectangle.ifc');
  checkStepScope(Buffer.concat([valid, Buffer.alloc(8 * 1024 * 1024 - valid.length, 32)]));
  assert.throws(() => checkStepScope(Buffer.alloc(8 * 1024 * 1024 + 1, 32)), /oversized/);
  assert.throws(() => checkStepScope(Buffer.concat([valid, Buffer.from('/* never closed')])), /comment/);
  assert.throws(() => checkStepScope(Buffer.from(valid.toString().replace('DATA;\n', `DATA;\n#999=IFCSITE(${'('.repeat(65)}1${')'.repeat(65)});\n`))), /nesting/);
  const entities = Array.from({ length: 20001 }, (_, index) => `#${index + 1000}=IFCCARTESIANPOINT($);\n`).join('');
  assert.throws(() => checkStepScope(Buffer.from(valid.toString().replace('DATA;\n', `DATA;\n${entities}`))), /excessive/);
  assert.throws(() => checkStepScope(Buffer.from("'x' ".repeat(250001))), /token bound/);
});

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
  ['missing active units', 'l_shape.ifc', "#9=IFCUNITASSIGNMENT((#7,#8,#6));", "#9=IFCUNITASSIGNMENT((#7,#8));", /unit|unreachable/i],
  ['wrong active unit prefix', 'l_shape.ifc', '#6=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);', '#6=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);', /unit|Prefix/i],
  ['wrong building origin', 'l_shape.ifc', "#24=IFCCARTESIANPOINT((13.,-2.,0.));", "#24=IFCCARTESIANPOINT((14.,-2.,0.));", /placement|bound/i],
  ['wrong storey height', 'l_shape.ifc', "#62=IFCCARTESIANPOINT((0.,0.,3.2));", "#62=IFCCARTESIANPOINT((0.,0.,4.));", /placement|bound/i],
  ['modified DRAFT', 'l_shape.ifc', "IFCLABEL('DRAFT')", "IFCLABEL('APPROVED')", /DRAFT/i],
  ['wrong footprint quantity', 'l_shape.ifc', "IFCQUANTITYAREA('FootprintArea',$,#7,75.", "IFCQUANTITYAREA('FootprintArea',$,#7,76.", /quantity/i],
  ['wrong footprint quantity unit', 'l_shape.ifc', "IFCQUANTITYAREA('FootprintArea',$,#7,75.", "IFCQUANTITYAREA('FootprintArea',$,#6,75.", /unit/i],
  ['missing proxy', 'l_shape.ifc', '#31=IFCBUILDINGELEMENTPROXY(', '#31=IFCWALL(', /STEP|wall|proxy|contain|model/i],
  ['extra proxy', 'l_shape.ifc', '#31=IFCBUILDINGELEMENTPROXY(', "#120=IFCBUILDINGELEMENTPROXY('1pjZ7N8p5EDQjTmNq0gxxR',$,'extra',$,$,$,$,$,.USERDEFINED.);\n#31=IFCBUILDINGELEMENTPROXY(", /proxy|unreachable/i],
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

test('rejects offsetting site and building placements with unchanged mass geometry', async () => {
  const item = fixture('l_shape.ifc');
  const model = changed(changed(read(item.file),
    '#19=IFCCARTESIANPOINT((0.,0.,0.));', '#19=IFCCARTESIANPOINT((1.,0.,0.));'),
    '#24=IFCCARTESIANPOINT((13.,-2.,0.));', '#24=IFCCARTESIANPOINT((12.,-2.,0.));');
  await assert.rejects(inspectBuffer(model, item, source(item)), /site placement/);
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
    { encoding: 'utf8', timeout: 60000, maxBuffer: 65536 });
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
        { encoding: 'utf8', timeout: 60000, maxBuffer: 65536 });
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
        { encoding: 'utf8', timeout: 60000, maxBuffer: 65536 });
      assert.equal(run.status, 1);
      assert.equal(JSON.parse(run.stdout).status, 'error');
      assert.ok(run.stdout.length < 2048);
    }
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
