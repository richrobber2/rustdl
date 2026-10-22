// On-demand source/dependency checks. Raw reports stay private; summaries omit code/secrets.
const fs = require('node:fs');
const path = require('node:path');
const {spawnSync} = require('node:child_process');
const root = path.resolve(__dirname, '..');
const output = path.join(root, 'target/security-reports');
process.umask(0o077);
fs.mkdirSync(output, {recursive: true, mode: 0o700});
fs.chmodSync(output, 0o700);
const tool = (variable, name) => process.env[variable] || path.join(root, 'target/security-tools', name);
function run(name, binary, args) {
  const out = fs.openSync(path.join(output, name + '.json'), 'w', 0o600);
  const err = fs.openSync(path.join(output, name + '.log'), 'w', 0o600);
  try {
    const result = spawnSync(binary, args, {cwd: root, stdio: ['ignore', out, err]});
    if (result.error) throw result.error;
    return result.status;
  } finally { fs.closeSync(out); fs.closeSync(err); }
}
function read(name) { return JSON.parse(fs.readFileSync(path.join(output, name + '.json'), 'utf8')); }
function files() {
  const result = spawnSync('git', ['ls-files', '--cached', '--others', '--exclude-standard', '-z'], {cwd: root, encoding: 'utf8'});
  if (result.status !== 0) throw new Error('Unable to enumerate repository files');
  return [...new Set(result.stdout.split('\0').filter(Boolean))].filter(name => fs.lstatSync(path.join(root, name), {throwIfNoEntry: false})?.isFile());
}
const rules = JSON.parse(fs.readFileSync(path.join(root, '.security/opengrep.json'), 'utf8')).rules;
const policy = new Map(rules.map(rule => [rule.id, rule]));
const summary = {dependencies: [], secrets: [], source: [], errors: [], coverage: {}};
function sourceScan(name, language, targets) {
  const targetFile = path.join(output, name + '-targets.json');
  fs.writeFileSync(targetFile, JSON.stringify(targets.map(file => ['CodeTarget', {path: file, analyzer: language, products: ['sast']}])));
  const status = run(name, tool('OPENGREP_CORE_BIN', 'opengrep-core'), ['-targets', targetFile, '-rules', '.security/opengrep.json', '-json_nodots', '-j', '1', '-max_memory', '512', '-timeout', '10']);
  const data = read(name);
  if (status !== 0 || data.errors?.length || data.skipped_rules?.length) summary.errors.push({tool: name, status, parseErrors: data.errors?.length || 0, skippedRules: data.skipped_rules?.length || 0});
  summary.coverage[language] = data.paths?.scanned?.length || 0;
  if (summary.coverage[language] !== targets.length) summary.errors.push({tool: name, expectedFiles: targets.length, scannedFiles: summary.coverage[language]});
  for (const result of data.results || []) {
    const rule = policy.get(result.check_id);
    summary.source.push({rule: result.check_id, severity: rule?.severity || 'ERROR', file: result.path, line: result.start.line, message: rule?.message || 'Unknown rule; review raw report.'});
  }
  return data;
}
async function main() {
  fs.rmSync(path.join(output, 'failure.log'), {force: true});
  const all = files();
  if (process.argv.includes('--test-rules')) {
    for (const [language, extension] of [['javascript', 'js'], ['java', 'java'], ['rust', 'rs']]) {
      const name = '.security/tests/unsafe.' + extension;
      const data = sourceScan('test-' + language, language, [name]);
      const expected = [...fs.readFileSync(path.join(root, name), 'utf8').matchAll(/\/\/ ruleid: ([\w.-]+)/g)].map(match => ({id: match[1], line: fs.readFileSync(path.join(root, name), 'utf8').slice(0, match.index).split('\n').length + 1}));
      const actual = (data.results || []).map(result => ({id: result.check_id, line: result.start.line}));
      const sort = values => values.sort((a, b) => a.id.localeCompare(b.id) || a.line - b.line);
      if (JSON.stringify(sort(expected)) !== JSON.stringify(sort(actual))) throw new Error('Rule fixture mismatch for ' + language);
    }
    if (summary.errors.length) throw new Error('Rule tests were incomplete');
    console.log('All six rules matched positive fixtures and rejected negative fixtures.');
    return;
  }
  summary.startedAt = new Date().toISOString();
  summary.complete = false;
  fs.writeFileSync(path.join(output, 'summary.json'), JSON.stringify(summary, null, 2) + '\n', {mode: 0o600});
  const history = spawnSync('git', ['rev-parse', '--is-shallow-repository'], {cwd: root, encoding: 'utf8'});
  summary.coverage.shallowHistory = history.stdout?.trim() !== 'false';
  if (history.status !== 0 || summary.coverage.shallowHistory) summary.errors.push({tool: 'gitleaks-history', reason: 'full Git history unavailable'});
  const osvStatus = run('osv', tool('OSV_SCANNER_BIN', 'osv-scanner'), ['scan', 'source', '--lockfile', 'Cargo.lock', '--lockfile', 'native-ui/Cargo.lock', '--lockfile', 'tests/ui/package-lock.json', '--no-call-analysis=rust', '--all-packages', '--format', 'json']);
  if (![0, 1].includes(osvStatus)) summary.errors.push({tool: 'osv', status: osvStatus});
  const osv = read('osv');
  summary.coverage.lockfiles = (osv.results || []).map(result => ({file: path.relative(root, result.source.path), packages: result.packages?.length || 0}));
  if (summary.coverage.lockfiles.length !== 3) summary.errors.push({tool: 'osv', reason: 'expected three lockfile results'});
  for (const result of osv.results || []) for (const pkg of result.packages || []) {
    const groups = pkg.groups || (pkg.vulnerabilities || []).map(vulnerability => ({ids: [vulnerability.id]}));
    for (const group of groups) {
      const records = (pkg.vulnerabilities || []).filter(vulnerability => group.ids.includes(vulnerability.id));
      const maintenance = records.length > 0 && records.every(vulnerability => (vulnerability.affected || []).some(affected => affected.database_specific?.informational === 'unmaintained'));
      summary.dependencies.push({file: path.relative(root, result.source.path), package: pkg.package.name, version: pkg.package.version, ids: group.ids, kind: maintenance ? 'maintenance' : 'vulnerability'});
    }
  }
  const stage = fs.mkdtempSync(path.join(output, 'worktree-'));
  try {
    const extensions = new Set(['.rs', '.java', '.js', '.cjs', '.mjs', '.json', '.toml', '.yml', '.yaml', '.sh', '.md', '.txt', '.html', '.css', '.xml', '.py', '.pem', '.key']);
    let count = 0;
    for (const name of all) {
      if (!extensions.has(path.extname(name)) && !['Makefile', 'Cargo.lock', 'Dockerfile', '.gitignore', '.gitattributes', '.editorconfig'].includes(path.basename(name)) && !name.startsWith('.githooks/')) continue;
      const original = path.join(root, name);
      if (fs.statSync(original).size > 2 * 1024 * 1024) { summary.errors.push({tool: 'gitleaks', file: name, reason: 'source exceeds scan limit'}); continue; }
      const target = path.join(stage, name);
      fs.mkdirSync(path.dirname(target), {recursive: true}); fs.copyFileSync(original, target); count++;
    }
    summary.coverage.secretSourceFiles = count;
    for (const [name, args] of [['gitleaks-history', ['git', root, '--log-opts=--all']], ['gitleaks-worktree', ['dir', stage]]]) {
      const status = run(name, tool('GITLEAKS_BIN', 'gitleaks'), [...args, '--redact=100', '--no-banner', '--no-color', '--report-format=json', '--report-path=-']);
      if (![0, 1].includes(status)) summary.errors.push({tool: name, status});
      for (const finding of read(name) || []) summary.secrets.push({scan: name, rule: finding.RuleID, file: finding.File.startsWith(stage) ? path.relative(stage, finding.File) : finding.File, line: finding.StartLine});
    }
  } finally { fs.rmSync(stage, {recursive: true, force: true}); }
  for (const [language, extension, prefixes] of [['javascript', '.js', ['assets/js/']], ['java', '.java', ['android/']], ['rust', '.rs', ['src/', 'native-ui/src/']]]) {
    sourceScan('opengrep-' + language, language, all.filter(name => name.endsWith(extension) && prefixes.some(prefix => name.startsWith(prefix))));
  }
  summary.complete = summary.errors.length === 0;
  fs.writeFileSync(path.join(output, 'summary.json'), JSON.stringify(summary, null, 2) + '\n', {mode: 0o600});
  const vulnerabilities = summary.dependencies.filter(finding => finding.kind === 'vulnerability');
  console.log(JSON.stringify(summary, null, 2));
  console.log(`Security checks: ${vulnerabilities.length} dependency vulnerability groups, ${summary.dependencies.length - vulnerabilities.length} maintenance notices, ${summary.secrets.length} secret findings, ${summary.source.length} source findings, ${summary.errors.length} scan errors.`);
  if (summary.errors.length) process.exitCode = 2;
  else if (vulnerabilities.length || summary.secrets.length || summary.source.some(finding => finding.severity === 'ERROR')) process.exitCode = 1;
}
main().catch(error => {
  fs.writeFileSync(path.join(output, 'failure.log'), String(error.stack), {mode: 0o600});
  summary.errors.push({tool: 'pipeline', error: error.name});
  fs.writeFileSync(path.join(output, 'summary.json'), JSON.stringify(summary, null, 2) + '\n', {mode: 0o600});
  console.error('Security scan incomplete. Check private reports in target/security-reports.');
  process.exitCode = 2;
});
