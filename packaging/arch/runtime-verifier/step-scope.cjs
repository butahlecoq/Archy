'use strict';

// Complete, bounded ISO 10303-21 clear-text parse for this schematic IFC4 profile.
// web-ifc performs the independent semantic and mesh checks after this inventory check.
const entityArity = new Map(Object.entries({
  IFCARBITRARYCLOSEDPROFILEDEF: 3, IFCAXIS2PLACEMENT3D: 3, IFCBUILDING: 12,
  IFCBUILDINGELEMENTPROXY: 9, IFCBUILDINGSTOREY: 10, IFCCARTESIANPOINT: 1,
  IFCCARTESIANPOINTLIST2D: 1, IFCDIRECTION: 1, IFCELEMENTQUANTITY: 6,
  IFCEXTRUDEDAREASOLID: 4, IFCGEOMETRICREPRESENTATIONCONTEXT: 6,
  IFCGEOMETRICREPRESENTATIONSUBCONTEXT: 10, IFCINDEXEDPOLYCURVE: 3,
  IFCLOCALPLACEMENT: 2, IFCPRODUCTDEFINITIONSHAPE: 3, IFCPROJECT: 9,
  IFCPROPERTYSET: 5, IFCPROPERTYSINGLEVALUE: 4, IFCQUANTITYAREA: 5,
  IFCQUANTITYVOLUME: 5, IFCRELAGGREGATES: 6, IFCRELCONTAINEDINSPATIALSTRUCTURE: 6,
  IFCRELDEFINESBYPROPERTIES: 6, IFCSHAPEREPRESENTATION: 4, IFCSITE: 14,
  IFCSIUNIT: 4, IFCUNITASSIGNMENT: 1,
}));
const typedValues = new Set(['IFCBOOLEAN', 'IFCLABEL', 'IFCREAL']);
const allowed = new Set([...entityArity.keys(), ...typedValues]);
const derivedSlots = new Map([
  ['IFCSIUNIT', new Set([0])],
  ['IFCGEOMETRICREPRESENTATIONSUBCONTEXT', new Set([2, 3, 4, 5])],
]);
// These are the only graph entry points. The semantic reader checks their exact counts
// and relationships; every other DATA entity must be used by one of these graphs.
const rootTypes = new Set(['IFCPROJECT', 'IFCRELAGGREGATES',
  'IFCRELCONTAINEDINSPATIALSTRUCTURE', 'IFCRELDEFINESBYPROPERTIES']);
const MAX_TOKENS = 250000;
const MAX_ENTITIES = 20000;
const MAX_DEPTH = 64;
const STEP_TOKEN = /(?:#[1-9][0-9]*|\.[A-Za-z0-9_]+\.|[+-]?(?:(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:[Ee][+-]?[0-9]+)?)|[A-Za-z][A-Za-z0-9_-]*|[()=;,*$])/y;

function checkStepScope(bytes) {
  if (!Buffer.isBuffer(bytes) || !bytes.length || bytes.length > 8 * 1024 * 1024) throw new Error('invalid or oversized STEP bytes');
  const input = bytes.toString('latin1');
  const tokens = [];
  let position = 0;
  while (position < input.length) {
    const code = input.charCodeAt(position);
    if (code === 9 || code === 10 || code === 13 || code === 32) { position++; continue; }
    if (input.startsWith('/*', position)) throw new Error('unsupported STEP comment');
    if (input[position] === "'") {
      const start = ++position;
      let closed = false;
      while (position < input.length) {
        if (input[position] === "'") {
          if (input[position + 1] === "'") position += 2;
          else { position++; closed = true; break; }
        } else position++;
      }
      if (!closed) throw new Error('unterminated STEP string');
      tokens.push({ kind: 'string', value: input.slice(start, position - 1).replaceAll("''", "'") });
      if (tokens.length > MAX_TOKENS) throw new Error('STEP token bound exceeded');
      continue;
    }
    STEP_TOKEN.lastIndex = position;
    const match = STEP_TOKEN.exec(input);
    if (!match) throw new Error('unsupported STEP syntax');
    const value = match[0];
    position += value.length;
    tokens.push({ kind: value[0] === '#' ? 'reference' : 'token', value: value.toUpperCase(), end: position });
    if (tokens.length > MAX_TOKENS) throw new Error('STEP token bound exceeded');
  }
  let index = 0;
  const peek = () => tokens[index];
  const take = () => tokens[index++];
  const expect = value => { if (take()?.value !== value) throw new Error(`invalid STEP structure: expected ${value}`); };
  const entities = new Set();
  const refs = new Set();
  const edges = new Map();
  const roots = new Set();
  let currentEntity = null;
  function expression(depth) {
    if (depth > MAX_DEPTH) throw new Error('STEP nesting bound exceeded');
    const token = take();
    if (!token) throw new Error('incomplete STEP expression');
    if (token.value === '(') {
      if (peek()?.value !== ')') {
        expression(depth + 1);
        while (peek()?.value === ',') { take(); expression(depth + 1); }
      }
      expect(')');
      return;
    }
    if (token.kind === 'reference') {
      refs.add(token.value);
      if (currentEntity !== null) edges.get(currentEntity).add(token.value);
      return;
    }
    if (token.kind === 'string' || token.value === '$' || token.value === '*' ||
        /^\.[A-Z0-9_]+\.$/.test(token.value) ||
        /^[+-]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:E[+-]?[0-9]+)?$/.test(token.value)) return;
    if (allowed.has(token.value)) { expect('('); if (peek()?.value !== ')') { expression(depth + 1); while (peek()?.value === ',') { take(); expression(depth + 1); } } expect(')'); return; }
    throw new Error('unsupported STEP expression or entity');
  }
  const expectString = value => {
    const token = take();
    if (token?.kind !== 'string' || token.value !== value) throw new Error('unsupported STEP HEADER value');
  };
  const expectSingleton = value => { expect('('); expectString(value); expect(')'); };
  expect('ISO-10303-21'); expect(';'); expect('HEADER'); expect(';');
  expect('FILE_DESCRIPTION'); expect('(');
  expectSingleton('ViewDefinition[DesignTransferView]'); expect(','); expectString('2;1');
  expect(')'); expect(';');
  expect('FILE_NAME'); expect('(');
  expectString('model.ifc'); expect(',');
  const timestamp = take();
  if (timestamp?.kind !== 'string' ||
      !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,6})?(?:\+00:00|Z)$/.test(timestamp.value) ||
      !Number.isFinite(Date.parse(timestamp.value)) ||
      new Date(timestamp.value).toISOString().slice(0, 19) !== timestamp.value.slice(0, 19)) {
    throw new Error('invalid STEP HEADER timestamp');
  }
  expect(','); expectSingleton(''); expect(','); expectSingleton(''); expect(',');
  expectString('IfcOpenShell 0.9.0.post1'); expect(',');
  expectString('IfcOpenShell 0.9.0.post1'); expect(','); expectString('Nobody');
  expect(')'); expect(';');
  expect('FILE_SCHEMA'); expect('('); expectSingleton('IFC4'); expect(')'); expect(';');
  expect('ENDSEC'); expect(';'); expect('DATA'); expect(';');
  while (peek()?.kind === 'reference') {
    const id = take().value;
    if (entities.has(id) || entities.size >= MAX_ENTITIES) throw new Error('duplicate or excessive STEP entity');
    entities.add(id);
    edges.set(id, new Set());
    expect('=');
    const type = take()?.value;
    if (!allowed.has(type) || typedValues.has(type)) throw new Error('unsupported STEP entity');
    if (rootTypes.has(type)) roots.add(id);
    currentEntity = id;
    expect('(');
    let fields = 0;
    while (peek()?.value !== ')') {
      if (fields) expect(',');
      if (derivedSlots.get(type)?.has(fields) && peek()?.value !== '*') {
        throw new Error('unsupported STEP derived field');
      }
      expression(0);
      fields++;
      if (fields > entityArity.get(type)) throw new Error('unsupported STEP entity arity');
    }
    if (fields !== entityArity.get(type)) throw new Error('unsupported STEP entity arity');
    expect(')'); expect(';');
    currentEntity = null;
  }
  expect('ENDSEC'); expect(';'); expect('END-ISO-10303-21'); expect(';');
  if (index !== tokens.length) throw new Error('trailing STEP content');
  if (!/^[\t\n\r ]*$/.test(input.slice(tokens[index - 1].end))) throw new Error('trailing STEP content');
  for (const id of refs) if (!entities.has(id)) throw new Error('dangling STEP reference');
  if (!entities.size) throw new Error('empty STEP data');
  const reached = new Set(roots);
  const pending = [...roots];
  while (pending.length) {
    for (const id of edges.get(pending.pop())) {
      if (!reached.has(id)) { reached.add(id); pending.push(id); }
    }
  }
  if (reached.size !== entities.size) throw new Error('unreachable STEP entity');
  return { headerTimestamp: timestamp.value };
}

module.exports = { checkStepScope };
