# Security checks

These checks run on demand and in `.github/workflows/security.yml`. They add no
runtime component to RustDL.

## Run on Linux

Requires Node 24, Git, curl and tar. No npm install is needed for these scripts.

```sh
node scripts/install-security-tools.cjs
node scripts/security-scan.cjs --test-rules
node scripts/security-scan.cjs
```

The installer downloads Gitleaks 8.30.1, OSV-Scanner 2.7.0 and Opengrep Core
1.31.0 into `target/security-tools`, verifying each upstream release archive or
binary against a pinned SHA-256 digest before installation. Digests come from
the upstream release checksum files and GitHub release-asset metadata. Update
versions and digests together after reviewing upstream releases.

The installer can also download the Linux ARM64 artifacts on Android. Running
them on Android needs compatible binaries or wrappers; the upstream Linux executables
can depend on Linux loaders or syscalls unavailable to a normal Android app.
Set `OSV_SCANNER_BIN`, `GITLEAKS_BIN` and `OPENGREP_CORE_BIN` to executable paths.
The repository scripts do not grant root or change device security settings.

## Scope and policy

- OSV checks the engine and native UI Cargo lockfiles and the UI npm lockfile.
  Its Rust call analysis is explicitly disabled, so the scanner does not execute
  project build scripts. Online queries disclose dependency identities/versions
  to the vulnerability database. Lockfile presence does not prove runtime reachability.
- Gitleaks checks all available Git refs and a text-only snapshot of tracked and
  unignored pending files. Ignored files, symlinks, media and files over 2 MiB are
  excluded; oversized eligible source files cause an incomplete scan. Full Git
  history requires an unshallow checkout, which the security workflow requests.
- Opengrep checks JavaScript in `assets/js`, Java in `android`, and Rust in `src`
  and `native-ui/src`. Six local rules cover TLS-error bypasses, universal file
  access, permissive hostname verification, dynamic JavaScript execution,
  URL-to-HTML taint and interpolated Rust shell commands. This is a focused rule
  set, not a complete vulnerability assessment. Rust unit tests within source
  files may require manual triage of matches.
- Known dependency vulnerabilities, secret findings and ERROR-level source
  findings fail the command. Informational `unmaintained` advisories identified
  by RustSec metadata and WARNING-level source matches remain visible for review.
  They are not silently allowlisted. Scanner failures, parser errors, skipped
  rules, missing coverage and fixture mismatches fail as incomplete scans.
- Opengrep uses one worker, a 512 MiB memory limit and a 10-second rule timeout
  per file. Source analysis is local; no remote rule registry or source upload is used.

## Reports

`target/security-reports` is private (directory mode 0700, files mode 0600).
Gitleaks runs with full redaction. The public-facing summary contains only
advisory IDs, package versions, source paths/lines and fixed rule messages;
it excludes secret values and matched source snippets. Raw scanner output and
failure diagnostics stay in the private report directory. Nothing reads the
user's downloaded media or Android app data.

Review `summary.json` and the private logs before treating a failed or partial
scan as a clean result. Exit status 0 means no blocking finding, 1 means blocking
findings, and 2 means incomplete scanning. Maintenance notices can remain with
status 0; this is not a claim that every dependency is maintained or safe.
