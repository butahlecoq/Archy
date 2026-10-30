// Read-only desktop v1 contracts shared by QML and Node. No command data is executable.
var STATUS_MAX_BYTES = 33554432;
var MAX_VARIANTS = 1000;
var MAX_ARTIFACTS = 4096;
var CAPABILITIES_MAX_BYTES = 65536;
var PREVIEW_SYMBOL_NOTICE = 'Screen-width massing edges are nonphysical preview symbols; GLB dimensions remain exact.';
function obj(v, keys) { return v !== null && typeof v === 'object' && !Array.isArray(v) && Object.keys(v).length === keys.length && keys.every(function(k) { return Object.prototype.hasOwnProperty.call(v,k); }); }
function str(v) { return typeof v === 'string' && v.length <= 4096; }
function small(v) { return str(v) && v.length > 0; }
function uint(v) { return typeof v === 'number' && isFinite(v) && Math.floor(v) === v && v >= 0 && v <= 9007199254740991; }
function hash(v) { return typeof v === 'string' && /^[0-9a-f]{64}$/.test(v); }
function validManifest(v) {
  return typeof v === 'string' && v.length > 1 && v.length <= 4096 && v[0] === '/'
    && v[1] !== '/' && !/[\u0000-\u001f\u007f]/.test(v)
    && !/(^|\/)\.\.?($|\/)/.test(v);
}
function validRunId(v) { return typeof v === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(v); }
function validCursor(v) { return typeof v === 'string' && v.length <= 200 && /^v1:[0-9a-f]{64}:[0-9]+:(genesis|[0-9a-f]{64}):[1-9][0-9]*$/.test(v); }
function capabilitiesArgv() { return ['/usr/bin/archy','desktop','capabilities','--json']; }
function statusArgv(manifest, run, cursor) {
  if (!validManifest(manifest) || (run !== null && !validRunId(run)) || (cursor !== null && !validCursor(cursor))) return null;
  var a = ['/usr/bin/archy','desktop','status','--project',manifest,'--limit','20'];
  if (run !== null) a.push('--run',run);
  if (cursor !== null) a.push('--cursor',cursor);
  a.push('--json'); return a;
}
function validCapabilities(c) {
  if (!obj(c,['schema_version','operations']) || c.schema_version !== 'archy-desktop-capabilities-v1' || !Array.isArray(c.operations) || c.operations.length !== 5) return false;
  var ids = ['project_status','run_status','open_report','open_preview','open_blender'];
  return c.operations.every(function(o) { return obj(o,['id','available','reason']) && ids.indexOf(o.id) >= 0 && typeof o.available === 'boolean' && (o.reason === null || str(o.reason)); })
    && ids.every(function(id) { return c.operations.filter(function(o) { return o.id === id }).length === 1 });
}
function validSummary(r) {
  var render = r && r.operation === 'render';
  var keys = ['run_id','revision','operation','variant_count','all_required_checks_passed','artifact_count'];
  if (render) keys.push('render_outcome');
  if (render && r.preview_symbols !== undefined) keys.push('preview_symbols');
  return obj(r,keys)
    && validRunId(r.run_id) && uint(r.revision) && ['generate','edit','evaluate','render','propose','export-ifc'].indexOf(r.operation) >= 0
    && (!render || ['verified','partial','failed'].indexOf(r.render_outcome) >= 0)
    && uint(r.variant_count) && r.variant_count <= MAX_VARIANTS
    && uint(r.artifact_count) && r.artifact_count <= MAX_ARTIFACTS
    && (!render || r.preview_symbols === undefined || validPreviewSymbols(r.preview_symbols, r.variant_count))
    && typeof r.all_required_checks_passed === 'boolean'
    && (r.operation !== 'propose' || (r.variant_count === 1 && !r.all_required_checks_passed));
}
function checkSummary(r) {
  if (r.operation === 'propose') return 'review proposal';
  var checks = 'required checks ' + (r.all_required_checks_passed ? 'passed' : 'unmet');
  return r.operation === 'export-ifc' ? 'draft schematic IFC · ' + checks : checks;
}
function validPreviewSymbols(symbols, variantCount) {
  if (!Array.isArray(symbols) || symbols.length > variantCount) return false;
  var seen = {};
  return symbols.every(function(symbol) {
    if (!obj(symbol,['variant_id','views','notice'])
        || !/^variant-[0-9a-f]{64}$/.test(symbol.variant_id)
        || Object.prototype.hasOwnProperty.call(seen,symbol.variant_id)
        || symbol.notice !== PREVIEW_SYMBOL_NOTICE
        || !Array.isArray(symbol.views) || symbol.views.length < 1 || symbol.views.length > 3
        || !symbol.views.every(function(view) { return ['top','front','iso'].indexOf(view) >= 0; })
        || symbol.views.some(function(view, i) { return symbol.views.indexOf(view) !== i; })) return false;
    seen[symbol.variant_id] = true;
    return true;
  });
}
function validArtifact(a) {
  return obj(a,['id','kind','byte_len','sha256','integrity']) && small(a.id)
    && ['report','preview','blender','variant','evaluation','source','run_data'].indexOf(a.kind) >= 0
    && (a.kind !== 'report' || a.id === 'report.md' || a.id === 'report.json')
    && (a.kind !== 'preview' || (/^previews\/[A-Za-z0-9_/-]+\.png$/.test(a.id) && a.id.indexOf('..') < 0)
      || /^renders\/variant-[0-9a-f]{64}\/view-(top|front|iso)\.png$/.test(a.id))
    && (a.kind !== 'blender' || (/^scenes\/[A-Za-z0-9_/-]+\.blend$/.test(a.id) && a.id.indexOf('..') < 0)
      || /^renders\/variant-[0-9a-f]{64}\/scene\.blend$/.test(a.id))
    && uint(a.byte_len) && hash(a.sha256) && a.integrity === 'verified';
}
function sameSummary(a, b) {
  return ['run_id','revision','operation','variant_count','all_required_checks_passed','artifact_count','render_outcome']
    .every(function(k) { return a[k] === b[k]; })
    && JSON.stringify(a.preview_symbols || []) === JSON.stringify(b.preview_symbols || []);
}
function validStatus(s, requestedRunId) {
  if (!obj(s,['schema_version','project','runs','selected_run','next_cursor','backend_jobs']) || s.schema_version !== 'archy-desktop-status-v1') return false;
  if (!obj(s.project,['id','revision','source_sha256']) || !small(s.project.id) || !uint(s.project.revision) || !hash(s.project.source_sha256)) return false;
  if (!Array.isArray(s.runs) || s.runs.length > 100 || !s.runs.every(validSummary)) return false;
  for (var i=0;i<s.runs.length;i++) {
    if (s.runs[i].revision > s.project.revision || (i>0 && s.runs[i-1].revision <= s.runs[i].revision)) return false;
  }
  if (s.selected_run !== null && (!obj(s.selected_run,['summary','artifacts']) || !validSummary(s.selected_run.summary)
      || s.selected_run.summary.revision > s.project.revision
      || !Array.isArray(s.selected_run.artifacts) || s.selected_run.artifacts.length > MAX_ARTIFACTS
      || s.selected_run.artifacts.length !== s.selected_run.summary.artifact_count
      || !s.selected_run.artifacts.every(validArtifact))) return false;
  var selected = s.selected_run === null ? null : s.selected_run.summary;
  if (requestedRunId === null || requestedRunId === undefined) {
    if (s.runs.length === 0 ? selected !== null : selected === null || !sameSummary(selected, s.runs[0])) return false;
  } else {
    if (!validRunId(requestedRunId) || selected === null || selected.run_id !== requestedRunId) return false;
    for (var j=0;j<s.runs.length;j++) {
      if (s.runs[j].run_id === requestedRunId && !sameSummary(selected, s.runs[j])) return false;
    }
  }
  return (s.next_cursor === null || validCursor(s.next_cursor))
    && obj(s.backend_jobs,['status','reason']) && s.backend_jobs.status === 'unknown' && str(s.backend_jobs.reason);
}
function newProbe(generation, kind, requestedRunId) { return { generation:generation, kind:kind, requestedRunId:requestedRunId || null, stdout:'', stdoutBytes:0, stderrBytes:0, failed:false, cancelled:false }; }
function append(p, stream, chunk) {
  if (p.cancelled) return true;
  if (p.failed || typeof chunk !== 'string') { p.failed=true; return false; }
  var limit = p.kind === 'capabilities' ? CAPABILITIES_MAX_BYTES : STATUS_MAX_BYTES;
  if (stream === 'stdout') {
    if (chunk.length > limit - p.stdoutBytes) { p.failed=true; return false; }
    for (var i=0;i<chunk.length;i++) if (chunk.charCodeAt(i)>127) { p.failed=true; return false; }
    p.stdoutBytes += chunk.length; p.stdout += chunk;
  } else if (stream === 'stderr') {
    // The trusted desktop producer has empty stderr on success. Reject the
    // first nonempty read, avoiding any decoded-character/UTF-8 byte mismatch.
    if (chunk.length > 0) { p.failed=true; return false; }
  } else { p.failed=true; return false; }
  return true;
}
function cancel(p) { p.cancelled=true; p.stdout=''; }
function complete(p, code, exitStatus, generation) {
  var result = { capabilities:null, status:null, reason:'desktop status unavailable' };
  if (!p || p.failed || p.cancelled || p.generation !== generation || code !== 0 || exitStatus !== 0 || p.stderrBytes !== 0 || p.stdoutBytes === 0) return result;
  try {
    var value=JSON.parse(p.stdout);
    if (p.kind === 'capabilities' && validCapabilities(value)) { result.capabilities=value; result.reason=''; }
    if (p.kind === 'status' && validStatus(value, p.requestedRunId)) { result.status=value; result.reason=''; }
  } catch (e) { result.reason='malformed desktop JSON'; }
  return result;
}
