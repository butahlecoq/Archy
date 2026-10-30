const fs = require('node:fs');
const { inspectBuffer, checkPackage } = require('./reader.cjs');

const MAX = 8 * 1024 * 1024 + 1024 * 1024 + 16384 + 12;
function readFrame() {
  const chunks = [];
  let size = 0;
  const chunk = Buffer.alloc(65536);
  for (;;) {
    const count = fs.readSync(0, chunk, 0, chunk.length, null);
    if (!count) break;
    size += count;
    if (size > MAX) throw new Error('verifier input exceeds bound');
    chunks.push(Buffer.from(chunk.subarray(0, count)));
  }
  const frame = Buffer.concat(chunks);
  if (frame.length < 12) throw new Error('short verifier frame');
  const sizes = [0, 4, 8].map(offset => frame.readUInt32BE(offset));
  if (sizes[0] > 16384 || sizes[1] > 1024 * 1024 || sizes[2] > 8 * 1024 * 1024 ||
      sizes.reduce((sum, value) => sum + value, 12) !== frame.length) {
    throw new Error('invalid verifier frame lengths');
  }
  let offset = 12;
  return sizes.map(size => {
    const value = frame.subarray(offset, offset + size);
    offset += size;
    return value;
  });
}

async function main() {
  try {
    checkPackage();
  } catch (error) {
    const failure = new Error(String(error.message || error));
    failure.runtimeUnavailable = true;
    throw failure;
  }
  const [expectedBytes, source, model] = readFrame();
  const expected = JSON.parse(expectedBytes.toString('utf8'));
  const result = await inspectBuffer(model, expected, source);
  if (result.status !== 'ok' || result.engine !== 'web-ifc') throw new Error('reader did not verify model');
  process.stdout.write(JSON.stringify({ status: 'ok', engine: result.engine,
    engine_version: result.engine_version, source_digest: result.source_digest }) + '\n');
}
main().catch(error => {
  process.exitCode = 1;
  process.stdout.write(JSON.stringify({ status: 'error',
    code: error.runtimeUnavailable ? 'RUNTIME_UNAVAILABLE' : 'ARTIFACT_INVALID',
    message: String(error.message || error).slice(0, 256) }) + '\n');
});
