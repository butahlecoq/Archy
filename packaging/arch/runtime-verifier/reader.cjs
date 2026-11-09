const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const web = require('web-ifc');
const { checkStepScope } = require('./step-scope.cjs');

const COORDINATE_TOLERANCE_M = 1e-6;
const MAX_IFC_BYTES = 8 * 1024 * 1024;
const PACKAGE = path.dirname(require.resolve('web-ifc'));
const NODE_JS_SHA256 = '592491e48f22ba5dd2f1d33a6485469bed63ac1b5f48547a80b146536556ff6f';
const NODE_WASM_SHA256 = '36b10f4f9fada12bb89ed664f9e61eae1c82b2ce5f04ee4175205149516498f5';
const GUID_NAMESPACE = Buffer.from('1dc7a2501d205e4bbc8db3d1c1b2f9b5', 'hex');
const IFC_BASE64 = '0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_$';

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

// Metadata slots on every entity in the allowed STEP graph. Other scalar slots
// (coordinates, quantities, IDs and source-bound names) are checked below.
const NULL_METADATA = new Map([
  [web.IFCPROJECT, ['OwnerHistory', 'Description', 'ObjectType', 'LongName', 'Phase']],
  [web.IFCSITE, ['OwnerHistory', 'Description', 'ObjectType', 'Representation', 'LongName', 'CompositionType',
    'RefLatitude', 'RefLongitude', 'RefElevation', 'LandTitleNumber', 'SiteAddress']],
  [web.IFCBUILDING, ['OwnerHistory', 'Description', 'ObjectType', 'Representation', 'LongName', 'CompositionType',
    'ElevationOfRefHeight', 'ElevationOfTerrain', 'BuildingAddress']],
  [web.IFCBUILDINGSTOREY, ['OwnerHistory', 'Description', 'ObjectType', 'Representation', 'LongName', 'CompositionType']],
  [web.IFCBUILDINGELEMENTPROXY, ['OwnerHistory', 'Tag']],
  [web.IFCRELAGGREGATES, ['OwnerHistory', 'Name', 'Description']],
  [web.IFCRELCONTAINEDINSPATIALSTRUCTURE, ['OwnerHistory', 'Name', 'Description']],
  [web.IFCRELDEFINESBYPROPERTIES, ['OwnerHistory', 'Name', 'Description']],
  [web.IFCPRODUCTDEFINITIONSHAPE, ['Name', 'Description']],
  [web.IFCPROPERTYSET, ['OwnerHistory', 'Description']],
  [web.IFCELEMENTQUANTITY, ['OwnerHistory', 'Description']],
  [web.IFCPROPERTYSINGLEVALUE, ['Description', 'Unit']],
  [web.IFCQUANTITYAREA, ['Description']],
  [web.IFCQUANTITYVOLUME, ['Description']],
  [web.IFCSIUNIT, ['Prefix']],
  [web.IFCGEOMETRICREPRESENTATIONCONTEXT, ['ContextIdentifier', 'TrueNorth']],
  [web.IFCGEOMETRICREPRESENTATIONSUBCONTEXT, ['Precision', 'TrueNorth', 'TargetScale', 'UserDefinedTargetView']],
  [web.IFCINDEXEDPOLYCURVE, ['Segments', 'SelfIntersect']],
]);

function checkNullMetadata(api, model) {
  for (const [type, fields] of NULL_METADATA) {
    for (const id of ids(api, model, type)) {
      const entity = api.GetLine(model, id);
      for (const field of fields) {
        requireCondition(entity[field] === null, `unsupported ${field} on IFC entity ${id}`);
      }
    }
  }
}

function expectedRootGuid(api, model, root, expected) {
  const typeNames = new Map([
    [web.IFCPROJECT, 'IfcProject'], [web.IFCSITE, 'IfcSite'], [web.IFCBUILDING, 'IfcBuilding'],
    [web.IFCBUILDINGSTOREY, 'IfcBuildingStorey'], [web.IFCBUILDINGELEMENTPROXY, 'IfcBuildingElementProxy'],
    [web.IFCPROPERTYSET, 'IfcPropertySet'], [web.IFCELEMENTQUANTITY, 'IfcElementQuantity'],
    [web.IFCRELAGGREGATES, 'IfcRelAggregates'],
    [web.IFCRELCONTAINEDINSPATIALSTRUCTURE, 'IfcRelContainedInSpatialStructure'],
    [web.IFCRELDEFINESBYPROPERTIES, 'IfcRelDefinesByProperties'],
  ]);
  const endpoint = id => {
    const entity = api.GetLine(model, id);
    const name = scalar(entity.Name);
    requireCondition(typeNames.has(entity.type) && typeof name === 'string' && name.length > 0,
      'unnamed IFC relationship endpoint');
    return `${typeNames.get(entity.type)}:${name}`;
  };
  const endpoints = refs => refs.map(item => endpoint(reference(item))).sort().join(',');
  const className = typeNames.get(root.type);
  requireCondition(className, 'unsupported IFC root role');
  let scope = expected.project_id;
  let role;
  if (root.type === web.IFCRELAGGREGATES) {
    const parent = reference(root.RelatingObject);
    role = `${endpoint(parent)}>${endpoints(root.RelatedObjects)}`;
    if (api.GetLine(model, parent).type !== web.IFCPROJECT) scope += `|${expected.variant_id}`;
  } else if (root.type === web.IFCRELCONTAINEDINSPATIALSTRUCTURE) {
    scope += `|${expected.variant_id}`;
    role = `${endpoint(reference(root.RelatingStructure))}>${endpoints(root.RelatedElements)}`;
  } else if (root.type === web.IFCRELDEFINESBYPROPERTIES) {
    scope += `|${expected.variant_id}`;
    role = `${endpoint(reference(root.RelatingPropertyDefinition))}>${endpoints(root.RelatedObjects)}`;
  } else {
    if (root.type !== web.IFCPROJECT && root.type !== web.IFCSITE) scope += `|${expected.variant_id}`;
    role = endpoint(root.expressID);
  }
  const hash = crypto.createHash('sha1').update(GUID_NAMESPACE).update(`${scope}|${className}|${role}`, 'utf8').digest();
  const uuid = Buffer.from(hash.subarray(0, 16));
  uuid[6] = (uuid[6] & 0x0f) | 0x50;
  uuid[8] = (uuid[8] & 0x3f) | 0x80;
  const standard = Buffer.concat([Buffer.alloc(2), uuid]).toString('base64').slice(2, 24);
  const alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
  return [...standard].map(char => IFC_BASE64[alphabet.indexOf(char)]).join('');
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
  requireCondition(Buffer.isBuffer(bytes) && bytes.length > 0 && bytes.length <= 1024 * 1024, 'invalid or oversized source bytes');
  const projectSource = expected.source_mode === 'accepted_project_json_v1';
  const normalizedSource = ['accepted_project_normalized_v1', 'accepted_project_normalized_v2'].includes(expected.source_mode);
  requireCondition(projectSource ? expected.source_file === 'input.json' :
    normalizedSource ? ['input.json', 'input.yaml'].includes(expected.source_file) :
    /^source\/[a-z_]+\.json$/.test(expected.source_file), 'invalid source file identity');
  let source;
  if (!normalizedSource) {
    try {
      source = JSON.parse(bytes.toString('utf8'));
    } catch {
      throw new Error('invalid source JSON');
    }
  }
  if (projectSource) {
    requireCondition(source && source.schema_version === '0.1.0' && source.project_id === expected.project_id &&
      source.brief?.storeys === expected.storeys && source.brief?.floor_to_floor_m === expected.floor_to_floor_m &&
      source.site?.north_deg === expected.north_angle_deg, 'project source/expected mismatch');
  } else if (!normalizedSource) {
    requireCondition(source && source.schema_version === 'ifc-reader-source-v1' &&
      Buffer.from(JSON.stringify(source) + '\n').equals(bytes), 'noncanonical source bytes');
    for (const field of ['project_id', 'variant_id', 'shape', 'origin_xy_m', 'width_m', 'depth_m',
      'notch_width_m', 'notch_depth_m', 'footprint_xy_m', 'footprint_area_m2', 'approximate_gfa_m2',
      'mass_volume_m3', 'storeys', 'floor_to_floor_m', 'height_m', 'north_angle_deg']) {
      requireCondition(JSON.stringify(source[field]) === JSON.stringify(expected[field]), `source/expected mismatch: ${field}`);
    }
  }
  const digest = crypto.createHash('sha256').update(bytes).digest('hex');
  requireCondition(expected.source_digest === digest, 'source digest does not match actual bytes');
  return digest;
}

function projectSourceReferences(sourceArtifacts, expected) {
  if (!Array.isArray(sourceArtifacts)) return false;
  if (['accepted_project_json_v1', 'accepted_project_normalized_v1',
    'accepted_project_normalized_v2'].includes(expected.source_mode)) return sourceArtifacts.length === 0;
  return sourceArtifacts.length === 1 && sourceArtifacts[0].path === expected.source_file &&
    sourceArtifacts[0].sha256 === expected.source_digest;
}

function inspectGraph(api, model, expected, budget) {
  requireCondition(api.GetModelSchema(model) === 'IFC4', 'wrong IFC schema');
  checkNullMetadata(api, model);
  const projectIds = ids(api, model, web.IFCPROJECT), siteIds = ids(api, model, web.IFCSITE);
  const buildingIds = ids(api, model, web.IFCBUILDING), storeyIds = ids(api, model, web.IFCBUILDINGSTOREY);
  const proxyIds = ids(api, model, web.IFCBUILDINGELEMENTPROXY);
  for (const [name, kind] of [['wall', web.IFCWALL], ['wall standard case', web.IFCWALLSTANDARDCASE],
    ['slab', web.IFCSLAB], ['space', web.IFCSPACE]]) exactCount(ids(api, model, kind), 0, name);
  for (const [name, values, count] of [['project', projectIds, 1], ['site', siteIds, 1], ['building', buildingIds, 1],
    ['storey', storeyIds, expected.storeys], ['proxy', proxyIds, expected.storeys]]) exactCount(values, count, name);
  const project = api.GetLine(model, projectIds[0]);
  const site = api.GetLine(model, siteIds[0]);
  const building = api.GetLine(model, buildingIds[0]);
  requireCondition(scalar(project.Name) === `${expected.project_id}:project` &&
    scalar(site.Name) === `${expected.project_id}:site` &&
    scalar(building.Name) === `${expected.project_id}:${expected.variant_id}:building`,
  'wrong project/site/building identity');
  requireCondition(['Description', 'ObjectType', 'LongName', 'Phase'].every(key => project[key] === null) &&
    ['Description', 'ObjectType', 'Representation', 'RefLatitude', 'RefLongitude', 'RefElevation', 'LandTitleNumber', 'SiteAddress', 'LongName', 'CompositionType'].every(key => site[key] === null) &&
    ['Description', 'ObjectType', 'Representation', 'ElevationOfRefHeight', 'ElevationOfTerrain', 'BuildingAddress', 'LongName', 'CompositionType'].every(key => building[key] === null),
  'unsupported project/site/building semantic field');
  const contextIds = ids(api, model, web.IFCGEOMETRICREPRESENTATIONCONTEXT);
  const subcontextIds = ids(api, model, web.IFCGEOMETRICREPRESENTATIONSUBCONTEXT);
  exactCount(contextIds, 1, 'model representation context');
  exactCount(subcontextIds, 1, 'body representation subcontext');
  sameSet(project.RepresentationContexts.map(reference), contextIds, 'project representation contexts mismatch');
  const context = api.GetLine(model, contextIds[0]);
  const subcontext = api.GetLine(model, subcontextIds[0]);
  requireCondition(scalar(context.ContextType) === 'Model' && scalar(context.CoordinateSpaceDimension) === 3 &&
    scalar(subcontext.ContextIdentifier) === 'Body' && scalar(subcontext.ContextType) === 'Model' &&
    scalar(subcontext.TargetView) === 'MODEL_VIEW' && reference(subcontext.ParentContext) === contextIds[0],
  'wrong active representation context');
  requireCondition(context.ContextIdentifier === null && context.TrueNorth === null &&
    scalar(context.Precision) === 1e-5 && subcontext.TargetScale === null &&
    subcontext.UserDefinedTargetView === null && subcontext.Precision === null &&
    subcontext.TrueNorth === null, 'unsupported model/body context option');
  const world = api.GetLine(model, reference(context.WorldCoordinateSystem));
  requireCondition(world.type === web.IFCAXIS2PLACEMENT3D, 'wrong world coordinate system');
  for (const [id, type, key, want] of [
    [reference(world.Location), web.IFCCARTESIANPOINT, 'Coordinates', [0, 0, 0]],
    [reference(world.Axis), web.IFCDIRECTION, 'DirectionRatios', [0, 0, 1]],
    [reference(world.RefDirection), web.IFCDIRECTION, 'DirectionRatios', [1, 0, 0]],
  ]) {
    const item = api.GetLine(model, id);
    const values = item[key]?.map(scalar);
    requireCondition(item.type === type && values?.length === 3, 'wrong world coordinate orientation');
    values.forEach((value, axis) => close(value, want[axis], budget.coordinate_m, 'wrong world coordinate orientation'));
  }
  const shapeIds = ids(api, model, web.IFCPRODUCTDEFINITIONSHAPE);
  const representationIds = ids(api, model, web.IFCSHAPEREPRESENTATION);
  const solidIds = ids(api, model, web.IFCEXTRUDEDAREASOLID);
  const profileIds = ids(api, model, web.IFCARBITRARYCLOSEDPROFILEDEF);
  exactCount(shapeIds, expected.storeys, 'product shape');
  exactCount(representationIds, expected.storeys, 'body representation');
  exactCount(solidIds, expected.storeys, 'extruded solid');
  exactCount(profileIds, 1, 'canonical profile');
  exactCount(ids(api, model, web.IFCINDEXEDPOLYCURVE), 1, 'profile curve');
  exactCount(ids(api, model, web.IFCCARTESIANPOINTLIST2D), 1, 'profile point list');
  const profile = api.GetLine(model, profileIds[0]);
  const curve = api.GetLine(model, reference(profile.OuterCurve));
  requireCondition(scalar(profile.ProfileType) === 'AREA' &&
    scalar(profile.ProfileName) === 'ArchyCanonicalFootprint' &&
    curve.type === web.IFCINDEXEDPOLYCURVE && curve.Segments === null && curve.SelfIntersect === null,
  'wrong schematic footprint profile');
  sameSet(proxyIds.map(id => reference(api.GetLine(model, id).Representation)), shapeIds, 'proxy shape membership mismatch');
  sameSet(shapeIds.flatMap(id => api.GetLine(model, id).Representations.map(reference)),
    representationIds, 'body representation membership mismatch');
  sameSet(representationIds.flatMap(id => {
    const representation = api.GetLine(model, id);
    requireCondition(reference(representation.ContextOfItems) === subcontextIds[0] &&
      scalar(representation.RepresentationIdentifier) === 'Body' &&
      scalar(representation.RepresentationType) === 'SweptSolid', 'wrong body representation context/type');
    return representation.Items.map(reference);
  }), solidIds, 'body solid membership mismatch');
  requireCondition(solidIds.every(id => reference(api.GetLine(model, id).SweptArea) === profileIds[0]),
    'wrong canonical solid profile');
  exactCount(ids(api, model, web.IFCUNITASSIGNMENT), 1, 'unit assignment');
  requireCondition(reference(project.UnitsInContext) === ids(api, model, web.IFCUNITASSIGNMENT)[0], 'project has no active unit assignment');
  const assignment = api.GetLine(model, reference(project.UnitsInContext));
  const assigned = assignment.Units.map(reference);
  exactCount(assigned, 3, 'active unit');
  sameSet(assigned, ids(api, model, web.IFCSIUNIT), 'SI unit membership mismatch');
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
  requireCondition(roots.every(root => scalar(root.GlobalId) === expectedRootGuid(api, model, root, expected)),
    'wrong source-bound root GUID');
  const propertySets = ids(api, model, web.IFCPROPERTYSET), quantities = ids(api, model, web.IFCELEMENTQUANTITY);
  exactCount(propertySets, 1, 'provenance property set');
  exactCount(quantities, 1, 'quantity set');
  const definitions = ids(api, model, web.IFCRELDEFINESBYPROPERTIES).map(id => api.GetLine(model, id));
  exactCount(definitions, 2, 'property relation');
  sameSet(definitions.map(item => reference(item.RelatingPropertyDefinition)), [propertySets[0], quantities[0]], 'property definitions mismatch');
  requireCondition(definitions.every(item => item.RelatedObjects.length === 1 && reference(item.RelatedObjects[0]) === buildingIds[0]), 'property set not attached to building');
  const pset = api.GetLine(model, propertySets[0]);
  requireCondition(scalar(pset.Name) === 'Archy_SchematicMassing' && pset.Description === null,
    'missing or conflicting schematic provenance');
  const propertyIds = pset.HasProperties.map(reference);
  sameSet(propertyIds, ids(api, model, web.IFCPROPERTYSINGLEVALUE), 'provenance property membership mismatch');
  const propertyNames = ['ProjectId', 'VariantId', 'RunId', 'CanonicalInputSha256', 'ExportedAtUTC',
    'SourceArtifacts', 'WorkerVersion', 'LibraryVersion', 'VariantSchemaVersion', 'Coordinates',
    'MethodVersion', 'ToleranceVersion', 'Schematic', 'ApprovalState', 'NorthAngleDegrees'];
  const properties = new Map(propertyIds.map(id => {
    const item = api.GetLine(model, id);
    requireCondition(item.Description === null && item.Unit === null, 'unsupported provenance property option');
    return [scalar(item.Name), scalar(item.NominalValue)];
  }));
  sameSet([...properties.keys()], propertyNames, 'provenance property names mismatch');
  exactCount(propertyIds, propertyNames.length, 'provenance property');
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
  requireCondition(projectSourceReferences(sourceArtifacts, expected), 'wrong IFC source artifact reference');
  requireCondition(properties.get('MethodVersion') === 'task02-analytic-massing-v1' &&
    properties.get('ToleranceVersion') === 'polygon-coordinate-parity-v1', 'wrong method/tolerance provenance');
  requireCondition(properties.get('WorkerVersion') === 'ifc-worker-v1' &&
    properties.get('LibraryVersion') === '0.9.0.post1' &&
    properties.get('VariantSchemaVersion') === '0.1.0' &&
    properties.get('Coordinates') === 'metres; right-handed XY, +Z; north is metadata',
  'wrong worker/coordinate provenance');
  requireCondition(typeof properties.get('ExportedAtUTC') === 'string' &&
    /^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d+)?(?:\+00:00|Z)$/.test(properties.get('ExportedAtUTC')) &&
    Number.isFinite(Date.parse(properties.get('ExportedAtUTC'))), 'invalid export timestamp');
  const qset = api.GetLine(model, quantities[0]);
  requireCondition(scalar(qset.Name) === 'Archy_SchematicQuantities' &&
    scalar(qset.MethodOfMeasurement) === 'task02-analytic-massing-v1; approximate, not net or certified' &&
    qset.Description === null,
  'wrong quantity set or measurement method');
  const quantityIds = qset.Quantities.map(reference);
  sameSet(quantityIds, [...ids(api, model, web.IFCQUANTITYAREA), ...ids(api, model, web.IFCQUANTITYVOLUME)], 'quantity membership mismatch');
  const quantityMap = new Map(quantityIds.map(id => {
    const item = api.GetLine(model, id);
    return [scalar(item.Name), item];
  }));
  sameSet([...quantityMap.keys()], ['FootprintArea', 'ApproximateGFA', 'MassVolume'], 'quantity names mismatch');
  exactCount(quantityIds, 3, 'quantity');
  exactCount(ids(api, model, web.IFCQUANTITYAREA), 2, 'area quantity');
  exactCount(ids(api, model, web.IFCQUANTITYVOLUME), 1, 'volume quantity');
  for (const [name, kind, want, tolerance] of [
    ['FootprintArea', 'AREAUNIT', expected.footprint_area_m2, budget.footprint_area_m2],
    ['ApproximateGFA', 'AREAUNIT', expected.approximate_gfa_m2, budget.footprint_area_m2 * expected.storeys],
    ['MassVolume', 'VOLUMEUNIT', expected.mass_volume_m3, budget.footprint_area_m2 * expected.height_m],
  ]) {
    const item = quantityMap.get(name);
    requireCondition(item && item.type === (kind === 'AREAUNIT' ? web.IFCQUANTITYAREA : web.IFCQUANTITYVOLUME) &&
      reference(item.Unit) === units.get(kind).id && item.Description === null, `wrong quantity type/unit ${name}`);
    close(scalar(item.AreaValue || item.VolumeValue), want, tolerance, `wrong quantity ${name}`);
  }
  for (const [name, formula] of [['FootprintArea', 'canonical footprint shoelace; m2'],
    ['ApproximateGFA', 'footprint area * storeys; m2'], ['MassVolume', 'footprint area * total height; m3']]) {
    requireCondition(scalar(quantityMap.get(name).Formula) === formula, `wrong quantity formula ${name}`);
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
    requireCondition(['Description', 'ObjectType', 'Representation', 'LongName', 'CompositionType'].every(key => storey[key] === null),
      'unsupported storey semantic field');
    sameSet(proxiesByStorey.get(storeyIds[index]) || [], [proxyIds[index]], 'proxy/storey containment mismatch');
    requireCondition(scalar(proxy.PredefinedType) === 'USERDEFINED' && scalar(proxy.ObjectType) === 'ArchySchematicStoreyMass', 'wrong schematic proxy type');
    requireCondition(scalar(proxy.Description) === 'Schematic extruded storey mass; DRAFT; no detailed construction semantics' &&
      proxy.Tag === null, 'wrong schematic proxy description');
    close(scalar(storey.Elevation), index * expected.floor_to_floor_m, budget.coordinate_m, 'wrong storey elevation');
    placement(storey, [origin[0], origin[1], index * expected.floor_to_floor_m], 'storey');
    placement(proxy, [origin[0], origin[1], index * expected.floor_to_floor_m], 'proxy');
    ordered.push(proxyIds[index]);
  }
  return { proxyIds: ordered, rootGuids: rootGuids.sort(), exportedAtUTC: properties.get('ExportedAtUTC'),
    activeUnits: Object.fromEntries([...units].map(([kind, item]) => [kind, item.name])) };
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
  const { headerTimestamp } = checkStepScope(bytes);
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
    requireCondition(headerTimestamp === graph.exportedAtUTC, 'STEP HEADER timestamp differs from export provenance');
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
