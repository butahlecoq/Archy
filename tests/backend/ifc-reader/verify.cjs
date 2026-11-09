const fs = require('node:fs');
const { inspectBuffer } = require('./reader.cjs');

function readRegular(file, limit) {
  const fd = fs.openSync(file, fs.constants.O_RDONLY | fs.constants.O_NONBLOCK | fs.constants.O_NOFOLLOW);
  try {
    const stat = fs.fstatSync(fd);
    if (!stat.isFile() || stat.size <= 0 || stat.size > limit) throw new Error('input is not a bounded regular file');
    const buffer = Buffer.alloc(stat.size);
    let offset = 0;
    while (offset < buffer.length) {
      const read = fs.readSync(fd, buffer, offset, buffer.length - offset, null);
      if (read === 0) throw new Error('input changed while reading');
      offset += read;
    }
    return buffer;
  } finally {
    fs.closeSync(fd);
  }
}

async function main() {
  if (process.argv.length !== 6) throw new Error('usage: node verify.cjs IFC_PATH EXPECTED_JSON CASE_FILE SOURCE_PATH');
  const expectedFile = JSON.parse(readRegular(process.argv[3], 64 * 1024).toString('utf8'));
  if (expectedFile.schema_version !== 'ifc-reader-fixture-v1' || !Array.isArray(expectedFile.cases)) {
    throw new Error('invalid expected fixture version');
  }
  const matches = expectedFile.cases.filter(item => item.file === process.argv[4]);
  if (matches.length !== 1) throw new Error('expected case is missing or ambiguous');
  const result = await inspectBuffer(readRegular(process.argv[2], 8 * 1024 * 1024), matches[0],
    readRegular(process.argv[5], 64 * 1024));
  process.stdout.write(JSON.stringify(result) + '\n');
}

main().catch(error => {
  process.exitCode = 1;
  const message = String(error.message || error).slice(0, 512);
  process.stdout.write(JSON.stringify({ status: 'error', message }) + '\n');
});
