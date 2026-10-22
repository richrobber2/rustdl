// Pinned upstream release artifacts; no remote shell scripts or global installation.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const {spawnSync} = require('node:child_process');
const root = path.resolve(__dirname, '..');
const output = path.join(root, 'target/security-tools');
const artifacts = {
  x64: [
    ['gitleaks', 'https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_linux_x64.tar.gz', '551f6fc83ea457d62a0d98237cbad105af8d557003051f41f3e7ca7b3f2470eb', 'gitleaks'],
    ['osv-scanner', 'https://github.com/google/osv-scanner/releases/download/v2.7.0/osv-scanner_linux_amd64', '160c6253e2922e9e0a6a9771c5c4c40a4b3c6090b34a3cdd5e80c96cc2e4218b'],
    ['opengrep-core', 'https://github.com/opengrep/opengrep/releases/download/v1.31.0/opengrep-core_linux_x86.tar.gz', '02328d41e710a0305d7407de255bf556f7901f515b6cc3637f68999c54800f6c', 'opengrep-core']
  ],
  arm64: [
    ['gitleaks', 'https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_linux_arm64.tar.gz', 'e4a487ee7ccd7d3a7f7ec08657610aa3606637dab924210b3aee62570fb4b080', 'gitleaks'],
    ['osv-scanner', 'https://github.com/google/osv-scanner/releases/download/v2.7.0/osv-scanner_linux_arm64', '4061ef4c021ed3c47f203b818f337e351336cc5cd67c0274fc36bd1dfc728dd7'],
    ['opengrep-core', 'https://github.com/opengrep/opengrep/releases/download/v1.31.0/opengrep-core_linux_aarch64.tar.gz', 'c6af82543b301fa3937c249913da400185d0c2cdd777d0c956875315aed388e2', 'opengrep-core']
  ]
};
function command(binary, args) {
  const result = spawnSync(binary, args, {stdio: 'inherit'});
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(binary + ' failed');
}
async function main() {
  if (!['linux', 'android'].includes(process.platform) || !artifacts[process.arch]) throw new Error('Installer downloads Linux x64/ARM64 tools. Other platforms require compatible tool overrides.');
  fs.mkdirSync(output, {recursive: true});
  for (const [name, url, expected, member] of artifacts[process.arch]) {
    const download = path.join(output, name + '.download');
    try {
      command('curl', ['--fail', '--location', '--retry', '3', '--retry-all-errors', '--connect-timeout', '15', '--max-time', '240', '--silent', '--show-error', '--output', download, url]);
      const hash = crypto.createHash('sha256');
      for await (const chunk of fs.createReadStream(download)) hash.update(chunk);
      if (hash.digest('hex') !== expected) throw new Error('Checksum mismatch for ' + name);
      if (member) command('tar', ['-xzf', download, '-C', output, member]);
      else fs.copyFileSync(download, path.join(output, name));
      fs.chmodSync(path.join(output, name), 0o700);
      console.log('Installed verified ' + name);
    } finally { fs.rmSync(download, {force: true}); }
  }
  if (process.platform === 'android') console.log('Linux tools downloaded. Android may need compatible execution wrappers via the *_BIN overrides.');
}
main().catch(error => { console.error(error.message); process.exitCode = 2; });
