// Build tool only. Generated scripts retain host property names and template markers.
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const {pathToFileURL} = require('node:url');
const {spawnSync} = require('node:child_process');
const root = path.resolve(__dirname, '..');
const sourceDir = path.join(root, 'assets/js');
const outputDir = path.resolve(process.argv[2] || path.join(root, 'target/minified-js'));
const version = '0.155.0';
const markers = code => [...code.matchAll(/__RUSTDL_[A-Z0-9_]+__/g)].map(match => match[0]).sort();

async function main() {
  if (outputDir === sourceDir || sourceDir.startsWith(outputDir + path.sep)) {
    throw new Error('Generated output must not overwrite the JavaScript sources');
  }
  let minify;
  if (!process.env.OXC_MINIFY_BIN) {
    // Avoid loading an Android native binding that may require unsupported LSE atomics.
    if (process.platform === 'android') process.env.NAPI_RS_FORCE_WASI = 'error';
    const packageDir = path.join(root, 'tests/ui/node_modules/oxc-minify');
    const installed = JSON.parse(fs.readFileSync(path.join(packageDir, 'package.json'), 'utf8'));
    if (installed.version !== version) throw new Error(`Expected oxc-minify ${version}; run npm --prefix tests/ui ci`);
    ({minifySync: minify} = await import(pathToFileURL(path.join(packageDir, 'index.js')).href));
  }
  const generated = [];
  for (const name of fs.readdirSync(sourceDir).filter(name => name.endsWith('.js')).sort()) {
    const filename = path.join(sourceDir, name);
    const source = fs.readFileSync(filename, 'utf8');
    let code;
    if (process.env.OXC_MINIFY_BIN) {
      // Compatible with the minifier example in the pinned Oxc checkout.
      const result = spawnSync(process.env.OXC_MINIFY_BIN, [filename, '--mangle', '--nospace'], {encoding: 'utf8', maxBuffer: 4 * 1024 * 1024});
      if (result.error) throw result.error;
      if (result.status !== 0) throw new Error(`${name}: minifier failed (${result.signal || result.status}): ${result.stderr}`);
      code = result.stdout;
    } else {
      const result = minify(name, source, {module: false, compress: true, mangle: {toplevel: false}, codegen: {removeWhitespace: true, legalComments: 'inline'}});
      if (result.errors.length) throw new Error(`${name}: ${JSON.stringify(result.errors)}`);
      code = result.code;
    }
    new vm.Script(code, {filename: name});
    if (JSON.stringify(markers(source)) !== JSON.stringify(markers(code))) throw new Error(`${name}: template markers changed`);
    if (/<\/script/i.test(code)) throw new Error(`${name}: unsafe inline script closing tag`);
    // Explicit separators keep independently embedded scripts safe to concatenate.
    code = code.trimEnd() + '\n;\n';
    generated.push({name, sourceBytes: Buffer.byteLength(source), outputBytes: Buffer.byteLength(code), code});
  }
  fs.mkdirSync(outputDir, {recursive: true});
  for (const {name, code} of generated) fs.writeFileSync(path.join(outputDir, name), code);
  const report = {version, files: generated.map(({name, sourceBytes, outputBytes}) => ({name, sourceBytes, outputBytes}))};
  fs.writeFileSync(path.join(outputDir, 'sizes.json'), JSON.stringify(report, null, 2) + '\n');
  const before = generated.reduce((sum, file) => sum + file.sourceBytes, 0);
  const after = generated.reduce((sum, file) => sum + file.outputBytes, 0);
  console.log(`JavaScript: ${before} -> ${after} bytes (${((1 - after / before) * 100).toFixed(1)}% smaller), ${generated.length} files`);
}
main().catch(error => { console.error(error.message); process.exitCode = 1; });
