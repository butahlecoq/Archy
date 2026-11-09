// Shared by QML and the Node contract harness. Data is never interpreted as QML/commands.
var MAX_BYTES = 65536;
function newSession() { return { generation: 0, busy: false, pending: false, probe: null }; }
function begin(s, queueIfBusy) {
  if (s.busy) { if (queueIfBusy) s.pending = true; return null; }
  s.busy = true; s.generation++;
  s.probe = newProbe(s.generation);
  return s.probe;
}
function teardown(s) { s.generation++; s.pending = false; if (s.probe) cancel(s.probe); }
function reaped(s) {
  var pending = s.pending;
  s.busy = false; s.pending = false; s.probe = null;
  return pending;
}
function bytes(s) {
  var n = 0;
  for (var i = 0; i < s.length; i++) {
    var c = s.charCodeAt(i);
    if (c < 128) n++;
    else if (c < 2048) n += 2;
    else if (c >= 0xd800 && c <= 0xdbff && i + 1 < s.length && s.charCodeAt(i + 1) >= 0xdc00 && s.charCodeAt(i + 1) <= 0xdfff) { n += 4; i++; }
    else n += 3;
  }
  return n;
}
function newProbe(generation) { return { generation: generation, stdout: '', stderrBytes: 0, stdoutBytes: 0, failed: false, cancelled: false }; }
function append(p, stream, chunk) {
  if (p.cancelled) return true;
  if (p.failed || typeof chunk !== 'string' || chunk.indexOf('\ufffd') !== -1) { p.failed = true; return false; }
  if (stream === 'stdout') {
    // The fixed doctor producer emits ASCII-escaped JSON. Reject split UTF-8
    // replacement and every other non-ASCII code unit before accumulation.
    if (chunk.length > MAX_BYTES - p.stdoutBytes) { p.failed = true; return false; }
    for (var i = 0; i < chunk.length; i++) {
      if (chunk.charCodeAt(i) > 127) { p.failed = true; return false; }
    }
    p.stdoutBytes += chunk.length; p.stdout += chunk;
  } else if (stream === 'stderr') {
    var count = bytes(chunk);
    if (count > MAX_BYTES - p.stderrBytes) { p.failed = true; return false; }
    p.stderrBytes += count;
  } else { p.failed = true; return false; }
  return true;
}
function cancel(p) { p.cancelled = true; p.stdout = ''; }
function object(v, keys) {
  return v !== null && typeof v === 'object' && !Array.isArray(v) && Object.keys(v).length === keys.length && keys.every(function(k) { return Object.prototype.hasOwnProperty.call(v, k); });
}
function str(v) { return typeof v === 'string' && v.length <= 4096; }
function nullable(v) { return v === null || str(v); }
function strings(v, max) { return Array.isArray(v) && v.length <= max && v.every(str); }
function status(v) { return ['available', 'missing', 'unsupported', 'unavailable', 'deferred'].indexOf(v) !== -1; }
function validReport(r) {
  if (!object(r, ['schema_version', 'host', 'platform', 'checks', 'capabilities', 'ready', 'target_acceptance', 'target_gates']) || r.schema_version !== '1' || typeof r.ready !== 'boolean' || r.target_acceptance !== 'deferred') return false;
  var h = r.host;
  if (!object(h, ['os_id', 'kernel', 'architecture', 'environment', 'evidence_sources']) || !nullable(h.os_id) || !nullable(h.kernel) || !str(h.architecture) || ['omarchy', 'wsl', 'other-linux', 'unknown'].indexOf(h.environment) < 0 || !strings(h.evidence_sources, 128)) return false;
  var p = r.platform;
  if (!object(p, ['supported_baseline', 'compatibility', 'raw_package_identity', 'upstream_version', 'development_override']) || !str(p.supported_baseline) || !(p.compatibility === null || status(p.compatibility)) || !nullable(p.raw_package_identity) || !nullable(p.upstream_version) || typeof p.development_override !== 'boolean') return false;
  if (!Array.isArray(r.checks) || r.checks.length > 128 || !r.checks.every(function(c) {
    return object(c, ['id', 'required_for', 'status', 'observed', 'expected', 'evidence', 'reason', 'remedy']) && str(c.id) && strings(c.required_for, 5) && c.required_for.every(function(x) { return ['core', 'blender', 'ifc', 'containment', 'desktop'].indexOf(x) >= 0; }) && status(c.status) && nullable(c.observed) && nullable(c.expected) && strings(c.evidence, 128) && str(c.reason) && str(c.remedy);
  })) return false;
  var names = ['core', 'blender', 'ifc', 'containment', 'desktop'];
  if (!object(r.capabilities, names) || !names.every(function(k) { var c = r.capabilities[k]; return object(c, ['ready', 'required_checks']) && typeof c.ready === 'boolean' && strings(c.required_checks, 128); })) return false;
  return Array.isArray(r.target_gates) && r.target_gates.length <= 32 && r.target_gates.every(function(g) { return object(g, ['id', 'status', 'reason']) && str(g.id) && g.status === 'deferred' && str(g.reason); });
}
function complete(p, exitCode, streamsComplete, generation) {
  if (!p || p.failed || p.cancelled || p.generation !== generation || !streamsComplete || (exitCode !== 0 && exitCode !== 1)) return { report: null, reason: 'doctor unavailable' };
  if (p.stderrBytes !== 0) return { report: null, reason: 'doctor stderr not empty' };
  try {
    var report = JSON.parse(p.stdout);
    return validReport(report) ? { report: report, reason: '' } : { report: null, reason: 'invalid doctor report' };
  } catch (e) { return { report: null, reason: 'incomplete doctor report' }; }
}
