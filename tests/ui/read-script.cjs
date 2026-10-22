const fs = require('node:fs');
const path = require('node:path');
// Keep synthetic fixtures identical while exercising either source or generated JS.
module.exports = name => fs.readFileSync(path.join(
  process.env.RUSTDL_JS_TEST_DIR || path.join(__dirname, '../../assets/js'), name
), 'utf8');
