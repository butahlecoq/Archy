// Verify the published Unicode patterns in an ECMAScript UTF-16 regex engine.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '../../..');
const fields = [
  ['compare_variants', 'objective'],
  ['generate_variants', 'destination'],
  ['evaluate_variant', 'destination'],
  ['render_variant', 'preset'],
  ['render_variant', 'destination'],
  ['export_ifc', 'destination'],
  ['propose_approval', 'destination'],
  ['propose_approval', 'reason'],
];
for (const [tool, field] of fields) {
  const file = path.join(root, 'schemas/mcp', `${tool}.schema.json`);
  const branches = JSON.parse(fs.readFileSync(file, 'utf8')).input.properties[field].anyOf;
  const patterns = branches.map(branch => new RegExp(branch.pattern));
  for (const invalid of ['\uD800', '\uDBFF', '\uDC00', '\uDFFF', 'a\uD800b', '\uDFFF\uD800']) {
    assert(patterns.every(pattern => !pattern.test(invalid)), `${tool}.${field}: lone surrogate`);
  }
  for (const valid of ['\uD800\uDC00', '\uDBFF\uDFFF', '😀', 'x\n😀']) {
    assert(patterns[3].test(valid), `${tool}.${field}: supplementary scalar`);
  }
}
console.log(`MCP Unicode patterns pass ECMAScript UTF-16 checks for ${fields.length} fields`);
