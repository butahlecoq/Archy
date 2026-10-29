const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const web = require('web-ifc');

const COORDINATE_TOLERANCE_M = 1e-6;
const MAX_IFC_BYTES = 8 * 1024 * 1024;
const PACKAGE = path.join(__dirname, 'node_modules', 'web-ifc');
const NODE_JS_SHA256 = '592491e48f22ba5dd2f1d33a6485469bed63ac1b5f48547a80b146536556ff6f';
const NODE_WASM_SHA256 = '36b10f4f9fada12bb89ed664f9e61eae1c82b2ce5f04ee4175205149516498f5';

function requireCondition(value, message) {
  if (!value) throw new Error(message);
}

function scalar(value) {
  if (value == null) return null;
  return value.value ?? value._representationValue ?? Number(value._internalValue);
}

function reference(value) {
  requireCondition(value && value.type === web.REF && Number.isInteger(value.value), 'invalid IFC reference');
  return value.value;
}

function ids(api, model, type, inherited = false) {
  const list = api.GetLineIDsWithType(model, type, inherited);
  return Array.from({ length: list.size() }, (_, index) => list.get(index));
}

function exactCount(values, count, name) {
  requireCondition(values.length === count, `wrong ${name} count: ${values.length}, expected ${count}`);
}

function close(actual, expected, budget, message) {
  requireCondition(Number.isFinite(actual) && Math.abs(actual - expected) <= budget,
    `${message}: ${actual}, expected ${expected}, budget ${budget}`);
}

function sameSet(actual, expected, message) {
  requireCondition(actual.length === expected.length && new Set(actual).size === actual.length &&
    actual.every(value => expected.includes(value)), message);
}

function transformVertex(vertex, matrix) {
  requireCondition(vertex.length === 3 && matrix.length === 16 && [...vertex, ...matrix].every(Number.isFinite), 'invalid mesh transform');
  const [x, y, z] = vertex;
  const wx = matrix[0] * x + matrix[4] * y + matrix[8] * z + matrix[12];
  const wy = matrix[1] * x + matrix[5] * y + matrix[9] * z + matrix[13];
  const wz = matrix[2] * x + matrix[6] * y + matrix[10] * z + matrix[14];
  return [wx, -wz, wy];
}

function measureSolid(vertices, indices) {
  requireCondition(vertices.length <= 200000 && indices.length <= 1200000, 'mesh exceeds verifier capacity');
  requireCondition(vertices.every(vertex => vertex.length === 3 && vertex.every(Number.isFinite)), 'nonfinite mesh vertex');
  requireCondition(indices.every(index => Number.isInteger(index) && index >= 0 && index < vertices.length), 'invalid mesh index');
  requireCondition(vertices.length >= 4 && indices.length >= 12 && indices.length % 3 === 0, 'mesh is not a closed triangulated solid');
  const bounds = [Infinity, Infinity, Infinity, -Infinity, -Infinity, -Infinity];
  for (const vertex of vertices) {
    for (let axis = 0; axis < 3; axis++) {
      bounds[axis] = Math.min(bounds[axis], vertex[axis]);
      bounds[axis + 3] = Math.max(bounds[axis + 3], vertex[axis]);
    }
  }
  const vertexKey = index => vertices[index].map(value => Math.round(value / COORDINATE_TOLERANCE_M)).join(',');
  const edges = new Map();
  const capEdges = new Map();
  const capPoints = new Map();
  let signedVolume = 0;
  let topArea = 0;
  let bottomArea = 0;
  for (let index = 0; index < indices.length; index += 3) {
    const triangle = indices.slice(index, index + 3);
    const [a, b, c] = triangle.map(i => vertices[i]);
    for (let edge = 0; edge < 3; edge++) {
      const first = vertexKey(triangle[edge]);
      const second = vertexKey(triangle[(edge + 1) % 3]);
      requireCondition(first !== second, 'degenerate mesh edge');
      const key = [first, second].sort().join('|');
      const item = edges.get(key) || { count: 0, winding: 0 };
      item.count++;
      item.winding += first < second ? 1 : -1;
      edges.set(key, item);
    }
    signedVolume += (a[0] * (b[1] * c[2] - b[2] * c[1]) +
      a[1] * (b[2] * c[0] - b[0] * c[2]) + a[2] * (b[0] * c[1] - b[1] * c[0])) / 6;
    if (Math.max(a[2], b[2], c[2]) - Math.min(a[2], b[2], c[2]) <= COORDINATE_TOLERANCE_M) {
      const projected = ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])) / 2;
      if (Math.abs(a[2] - bounds[5]) <= COORDINATE_TOLERANCE_M) {
        topArea += projected;
        for (let edge = 0; edge < 3; edge++) {
          const first = [a, b, c][edge], second = [a, b, c][(edge + 1) % 3];
          const firstKey = first.slice(0, 2).map(value => Math.round(value / COORDINATE_TOLERANCE_M)).join(',');
          const secondKey = second.slice(0, 2).map(value => Math.round(value / COORDINATE_TOLERANCE_M)).join(',');
          requireCondition(firstKey !== secondKey, 'degenerate cap edge');
          const key = [firstKey, secondKey].sort().join('|');
          const item = capEdges.get(key) || { count: 0, first: firstKey, second: secondKey };
          item.count++;
          capEdges.set(key, item);
          capPoints.set(firstKey, first.slice(0, 2));
          capPoints.set(secondKey, second.slice(0, 2));
        }
      }
      else if (Math.abs(a[2] - bounds[2]) <= COORDINATE_TOLERANCE_M) bottomArea += projected;
    }
  }
  requireCondition([...edges.values()].every(edge => edge.count === 2 && edge.winding === 0), 'mesh is not closed with consistent winding');
  requireCondition(signedVolume > 0 && topArea > 0 && bottomArea < 0, 'mesh has invalid signed volume or caps');
  requireCondition([...capEdges.values()].every(edge => edge.count === 1 || edge.count === 2), 'invalid cap triangulation');
  const boundary = [...capEdges.values()].filter(edge => edge.count === 1);
  const successors = new Map(boundary.map(edge => [edge.first, edge.second]));
  requireCondition(boundary.length >= 3 && successors.size === boundary.length, 'invalid cap boundary');
  const outline = [];
  const start = boundary[0].first;
  let current = start;
  for (let i = 0; i < boundary.length; i++) {
    requireCondition(current && !outline.some(item => item.key === current), 'cap boundary is not one loop');
    outline.push({ key: current, xy: capPoints.get(current) });
    current = successors.get(current);
  }
  requireCondition(current === start, 'cap boundary is open');
  return { bounds_m: bounds, signed_volume_m3: signedVolume, cap_area_m2: topArea,
    bottom_cap_area_m2: -bottomArea, cap_outline_xy_m: outline.map(item => item.xy), triangles: indices.length / 3 };
}

function sameOutline(observed, closedExpected, tolerance) {
  const expected = closedExpected.slice(0, -1);
  if (observed.length !== expected.length) return false;
  for (const direction of [1, -1]) {
    for (let offset = 0; offset < expected.length; offset++) {
      if (observed.every((point, index) => point.every((value, axis) =>
        Math.abs(value - expected[(offset + direction * index + expected.length) % expected.length][axis]) <= tolerance))) {
        return true;
      }
    }
  }
  return false;
}

function checkPackage() {
  requireCondition(process.versions.node === '22.22.2', `untested Node version ${process.versions.node}`);
  const pkg = JSON.parse(fs.readFileSync(path.join(PACKAGE, 'package.json')));
  requireCondition(pkg.version === '0.0.78', 'wrong web-ifc version');
  for (const [file, expected] of [['web-ifc-api-node.js', NODE_JS_SHA256], ['web-ifc-node.wasm', NODE_WASM_SHA256]]) {
    const observed = crypto.createHash('sha256').update(fs.readFileSync(path.join(PACKAGE, file))).digest('hex');
    requireCondition(observed === expected, `wrong web-ifc ${file} SHA-256`);
  }
}

function expectedBudget(expected) {
  const points = expected.footprint_xy_m;
  requireCondition(Array.isArray(points) && points.length >= 5 && points.length <= 7 &&
    points.every(point => Array.isArray(point) && point.length === 2 && point.every(Number.isFinite)), 'invalid expected footprint');
  let perimeter = 0;
  for (let i = 0; i < points.length - 1; i++) {
    perimeter += Math.abs(points[i + 1][0] - points[i][0]) + Math.abs(points[i + 1][1] - points[i][1]);
  }
  return { coordinate_m: COORDINATE_TOLERANCE_M,
    footprint_area_m2: perimeter * COORDINATE_TOLERANCE_M + (points.length - 1) * COORDINATE_TOLERANCE_M ** 2 };
}

function verifySource(bytes, expected) {
  requireCondition(Buffer.isBuffer(bytes) && bytes.length > 0 && bytes.length <= 64 * 1024, 'invalid or oversized source bytes');
  requireCondition(/^source\/[a-z_]+\.json$/.test(expected.source_file), 'invalid source file identity');
  let source;
  try {
    source = JSON.parse(bytes.toString('utf8'));
  } catch {
    throw new Error('invalid source JSON');
  }
  requireCondition(source && source.schema_version === 'ifc-reader-source-v1' &&
    Buffer.from(JSON.stringify(source) + '\n').equals(bytes), 'noncanonical source bytes');
  for (const field of ['project_id', 'variant_id', 'shape', 'origin_xy_m', 'width_m', 'depth_m',
    'notch_width_m', 'notch_depth_m', 'footprint_xy_m', 'footprint_area_m2', 'approximate_gfa_m2',
    'mass_volume_m3', 'storeys', 'floor_to_floor_m', 'height_m', 'north_angle_deg']) {
    requireCondition(JSON.stringify(source[field]) === JSON.stringify(expected[field]), `source/expected mismatch: ${field}`);
  }
  const digest = crypto.createHash('sha256').update(bytes).digest('hex');
  requireCondition(expected.source_digest === digest, 'source digest does not match actual bytes');
  return digest;
}

function inspectGraph(api, model, expected, budget) {
  requireCondition(api.GetModelSchema(model) === 'IFC4', 'wrong IFC schema');
  const projectIds = ids(api, model, web.IFCPROJECT), siteIds = ids(api, model, web.IFCSITE);
  const buildingIds = ids(api, model, web.IFCBUILDING), storeyIds = ids(api, model, web.IFCBUILDINGSTOREY);
  const proxyIds = ids(api, model, web.IFCBUILDINGELEMENTPROXY);
  for (const [name, values, count] of [['project', projectIds, 1], ['site', siteIds, 1], ['building', buildingIds, 1],
    ['storey', storeyIds, expected.storeys], ['proxy', proxyIds, expected.storeys]]) exactCount(values, count, name);
  const project = api.GetLine(model, projectIds[0]);
  requireCondition(reference(project.UnitsInContext) === ids(api, model, web.IFCUNITASSIGNMENT)[0], 'project has no active unit assignment');
  const assignment = api.GetLine(model, reference(project.UnitsInContext));
  const assigned = assignment.Units.map(reference);
  exactCount(assigned, 3, 'active unit');
  const units = new Map();
  for (const id of assigned) {
    const unit = api.GetLine(model, id);
    requireCondition(unit.type === web.IFCSIUNIT && unit.Prefix === null, 'unsupported active unit');
    const kind = scalar(unit.UnitType), name = scalar(unit.Name);
    requireCondition(!units.has(kind), 'duplicate active unit');
    units.set(kind, { id, name });
  }
  for (const [kind, name] of [['LENGTHUNIT', 'METRE'], ['AREAUNIT', 'SQUARE_METRE'], ['VOLUMEUNIT', 'CUBIC_METRE']]) {
    requireCondition(units.get(kind)?.name === name, `wrong active ${kind}`);
  }
  const aggs = ids(api, model, web.IFCRELAGGREGATES).map(id => api.GetLine(model, id));
  exactCount(aggs, 3, 'aggregate relation');
  const children = parent => {
    const rel = aggs.filter(item => reference(item.RelatingObject) === parent);
    exactCount(rel, 1, 'aggregate parent');
    return rel[0].RelatedObjects.map(reference);
  };
  sameSet(children(projectIds[0]), siteIds, 'project/site hierarchy mismatch');
  sameSet(children(siteIds[0]), buildingIds, 'site/building hierarchy mismatch');
  sameSet(children(buildingIds[0]), storeyIds, 'building/storey hierarchy mismatch');
  const contained = ids(api, model, web.IFCRELCONTAINEDINSPATIALSTRUCTURE).map(id => api.GetLine(model, id));
  exactCount(contained, expected.storeys, 'containment relation');
  const proxiesByStorey = new Map();
  for (const rel of contained) {
    const storey = reference(rel.RelatingStructure);
    requireCondition(storeyIds.includes(storey) && !proxiesByStorey.has(storey), 'wrong storey containment');
    proxiesByStorey.set(storey, rel.RelatedElements.map(reference));
  }
  const roots = ids(api, model, web.IFCROOT, true).map(id => api.GetLine(model, id));
  const rootGuids = roots.map(root => scalar(root.GlobalId));
  requireCondition(rootGuids.every(guid => typeof guid === 'string' && /^[0-9A-Za-z_$]{22}$/.test(guid)) &&
    new Set(rootGuids).size === rootGuids.length, 'invalid or duplicate root GUID');
  const propertySets = ids(api, model, web.IFCPROPERTYSET), quantities = ids(api, model, web.IFCELEMENTQUANTITY);
  exactCount(propertySets, 1, 'provenance property set');
  exactCount(quantities, 1, 'quantity set');
  const definitions = ids(api, model, web.IFCRELDEFINESBYPROPERTIES).map(id => api.GetLine(model, id));
  exactCount(definitions, 2, 'property relation');
  sameSet(definitions.map(item => reference(item.RelatingPropertyDefinition)), [propertySets[0], quantities[0]], 'property definitions mismatch');
  requireCondition(definitions.every(item => item.RelatedObjects.length === 1 && reference(item.RelatedObjects[0]) === buildingIds[0]), 'property set not attached to building');
  const pset = api.GetLine(model, propertySets[0]);
  requireCondition(scalar(pset.Name) === 'Archy_SchematicMassing', 'missing schematic provenance');
  const properties = new Map(pset.HasProperties.map(ref => {
    const item = api.GetLine(model, reference(ref));
    return [scalar(item.Name), scalar(item.NominalValue)];
  }));
  for (const [name, value] of [['ProjectId', expected.project_id], ['VariantId', expected.variant_id],
    ['RunId', expected.run_id], ['CanonicalInputSha256', expected.source_digest], ['ApprovalState', 'DRAFT'],
    ['Schematic', true], ['NorthAngleDegrees', expected.north_angle_deg]]) {
    requireCondition(properties.get(name) === value, `wrong DRAFT/source property ${name}`);
  }
  let sourceArtifacts;
  try {
    sourceArtifacts = JSON.parse(properties.get('SourceArtifacts'));
  } catch {
    throw new Error('invalid IFC source artifact references');
  }
  requireCondition(Array.isArray(sourceArtifacts) && sourceArtifacts.length === 1 &&
    sourceArtifacts[0].path === expected.source_file &&
    sourceArtifacts[0].sha256 === expected.source_digest, 'wrong IFC source artifact reference');
  requireCondition(properties.get('MethodVersion') === 'task02-analytic-massing-v1' &&
    properties.get('ToleranceVersion') === 'polygon-coordinate-parity-v1', 'wrong method/tolerance provenance');
  const qset = api.GetLine(model, quantities[0]);
  requireCondition(scalar(qset.Name) === 'Archy_SchematicQuantities', 'wrong quantity set');
  const quantityMap = new Map(qset.Quantities.map(ref => {
    const item = api.GetLine(model, reference(ref));
    return [scalar(item.Name), item];
  }));
  exactCount([...quantityMap], 3, 'quantity');
  for (const [name, kind, want, tolerance] of [
    ['FootprintArea', 'AREAUNIT', expected.footprint_area_m2, budget.footprint_area_m2],
    ['ApproximateGFA', 'AREAUNIT', expected.approximate_gfa_m2, budget.footprint_area_m2 * expected.storeys],
    ['MassVolume', 'VOLUMEUNIT', expected.mass_volume_m3, budget.footprint_area_m2 * expected.height_m],
  ]) {
    const item = quantityMap.get(name);
    requireCondition(item && reference(item.Unit) === units.get(kind).id, `wrong quantity unit ${name}`);
    close(scalar(item.AreaValue || item.VolumeValue), want, tolerance, `wrong quantity ${name}`);
  }
  const origin = expected.origin_xy_m;
  const placement = (entity, want, label) => {
    const matrix = api.GetWorldTransformMatrix(model, reference(entity.ObjectPlacement));
    requireCondition(matrix.length === 16 && matrix.every(Number.isFinite), `invalid ${label} placement`);
    for (const [index, target] of [[12, want[0]], [13, want[1]], [14, want[2]]]) close(matrix[index], target, budget.coordinate_m, `wrong ${label} placement`);
    for (const index of [0, 5, 10, 15]) close(matrix[index], 1, budget.coordinate_m, `rotated ${label} placement`);
    for (const index of [1, 2, 4, 6, 8, 9]) close(matrix[index], 0, budget.coordinate_m, `rotated ${label} placement`);
  };
  placement(api.GetLine(model, siteIds[0]), [0, 0, 0], 'site');
  placement(api.GetLine(model, buildingIds[0]), [origin[0], origin[1], 0], 'building');
  const ordered = [];
  for (let index = 0; index < expected.storeys; index++) {
    const storey = api.GetLine(model, storeyIds[index]);
    const proxy = api.GetLine(model, proxyIds[index]);
    requireCondition(scalar(storey.Name) === `${expected.project_id}:${expected.variant_id}:storey:${String(index).padStart(3, '0')}`, 'wrong storey identity');
    requireCondition(scalar(proxy.Name) === `${expected.project_id}:${expected.variant_id}:mass:${String(index).padStart(3, '0')}`, 'wrong proxy identity');
    sameSet(proxiesByStorey.get(storeyIds[index]) || [], [proxyIds[index]], 'proxy/storey containment mismatch');
    requireCondition(scalar(proxy.PredefinedType) === 'USERDEFINED' && scalar(proxy.ObjectType) === 'ArchySchematicStoreyMass', 'wrong schematic proxy type');
    close(scalar(storey.Elevation), index * expected.floor_to_floor_m, budget.coordinate_m, 'wrong storey elevation');
    placement(storey, [origin[0], origin[1], index * expected.floor_to_floor_m], 'storey');
    placement(proxy, [origin[0], origin[1], index * expected.floor_to_floor_m], 'proxy');
    ordered.push(proxyIds[index]);
  }
  return { proxyIds: ordered, rootGuids: rootGuids.sort(), activeUnits: Object.fromEntries([...units].map(([kind, item]) => [kind, item.name])) };
}

function inspectMeshes(api, model, proxyIds, expected, budget) {
  const observed = new Map();
  api.StreamAllMeshesWithTypes(model, [web.IFCBUILDINGELEMENTPROXY], mesh => {
    requireCondition(proxyIds.includes(mesh.expressID) && !observed.has(mesh.expressID), 'unexpected or duplicate proxy mesh');
    requireCondition(mesh.geometries.size() === 1, 'unsupported proxy geometry count');
    const placed = mesh.geometries.get(0);
    const geometry = api.GetGeometry(model, placed.geometryExpressID);
    try {
      const raw = Array.from(api.GetVertexArray(geometry.GetVertexData(), geometry.GetVertexDataSize()));
      const indices = Array.from(api.GetIndexArray(geometry.GetIndexData(), geometry.GetIndexDataSize()));
      requireCondition(raw.length % 6 === 0 && raw.length >= 24 && raw.length <= 1200000, 'unsupported vertex layout');
      const vertices = [];
      for (let i = 0; i < raw.length; i += 6) vertices.push(transformVertex(raw.slice(i, i + 3), placed.flatTransformation));
      observed.set(mesh.expressID, { ...measureSolid(vertices, indices), vertices, indices });
    } finally {
      geometry.delete();
    }
  }, true);
  exactCount([...observed], expected.storeys, 'proxy mesh');
  const xValues = expected.footprint_xy_m.map(point => point[0]);
  const yValues = expected.footprint_xy_m.map(point => point[1]);
  const proxies = [];
  for (let index = 0; index < proxyIds.length; index++) {
    const result = observed.get(proxyIds[index]);
    const z = index * expected.floor_to_floor_m;
    const bounds = [Math.min(...xValues), Math.min(...yValues), z, Math.max(...xValues), Math.max(...yValues), z + expected.floor_to_floor_m];
    result.bounds_m.forEach((value, axis) => close(value, bounds[axis], budget.coordinate_m, 'transformed mesh bounds'));
    requireCondition(sameOutline(result.cap_outline_xy_m, expected.footprint_xy_m, budget.coordinate_m), 'transformed footprint outline mismatch');
    close(result.cap_area_m2, expected.footprint_area_m2, budget.footprint_area_m2, 'cap footprint area/notch');
    close(result.bottom_cap_area_m2, expected.footprint_area_m2, budget.footprint_area_m2, 'bottom footprint area');
    const volumeBudget = budget.footprint_area_m2 * expected.floor_to_floor_m + expected.footprint_area_m2 * 2 * budget.coordinate_m;
    close(result.signed_volume_m3, expected.footprint_area_m2 * expected.floor_to_floor_m, volumeBudget, 'signed mass volume');
    if (expected.shape === 'l_shape') {
      const notchX = expected.origin_xy_m[0] + expected.width_m - expected.notch_width_m;
      const notchY = expected.origin_xy_m[1] + expected.depth_m - expected.notch_depth_m;
      for (let i = 0; i < result.indices.length; i += 3) {
        const triangle = result.indices.slice(i, i + 3).map(vertex => result.vertices[vertex]);
        if (triangle.every(vertex => Math.abs(vertex[2] - result.bounds_m[5]) <= budget.coordinate_m)) {
          const centroid = [0, 1].map(axis => triangle.reduce((sum, vertex) => sum + vertex[axis], 0) / 3);
          requireCondition(!(centroid[0] > notchX + budget.coordinate_m && centroid[1] > notchY + budget.coordinate_m), 'L notch filled');
        }
      }
    }
    proxies.push({ id: proxyIds[index], bounds_m: result.bounds_m, signed_volume_m3: result.signed_volume_m3,
      cap_area_m2: result.cap_area_m2, cap_outline_xy_m: result.cap_outline_xy_m, triangles: result.triangles });
  }
  return proxies;
}

async function inspectBuffer(bytes, expected, sourceBytes) {
  requireCondition(Buffer.isBuffer(bytes) && bytes.length > 0 && bytes.length <= MAX_IFC_BYTES, 'invalid or oversized IFC bytes');
  requireCondition(expected && expected.schema_version === undefined && Number.isInteger(expected.storeys) && expected.storeys >= 1 && expected.storeys <= 100, 'invalid expected fixture');
  const sourceDigest = verifySource(sourceBytes, expected);
  const budget = expectedBudget(expected);
  checkPackage();
  const api = new web.IfcAPI();
  api.SetWasmPath(PACKAGE + path.sep, true);
  await api.Init();
  let model;
  try {
    model = api.OpenModel(new Uint8Array(bytes), { ALLOW_INCOMPATIBLE_SCHEMA_ALIASES: false,
      COORDINATE_TO_ORIGIN: false, MEMORY_LIMIT: 128 * 1024 * 1024 });
    const graph = inspectGraph(api, model, expected, budget);
    const proxies = inspectMeshes(api, model, graph.proxyIds, expected, budget);
    return { status: 'ok', engine: 'web-ifc', engine_version: '0.0.78', node_version: process.versions.node,
      schema: 'IFC4', transform_policy: 'web-ifc placed column-major Y-up to IFC XYZ; once',
      linear_scaling: true, source_digest: sourceDigest, budget, active_units: graph.activeUnits,
      root_guids: graph.rootGuids, proxies };
  } finally {
    if (model !== undefined) api.CloseModel(model);
    api.Dispose();
  }
}

module.exports = { inspectBuffer, transformVertex, measureSolid, checkPackage };
