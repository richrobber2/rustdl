# Initial security audit — 2026-10-22 UTC

Scope: RustDL source and dependency lockfiles. This does not assess the entire
Android OS or certify the installed APK. The patches below apply to the source
lockfiles and take effect in a newly built application.

## Findings and patches

The initial OSV scan identified security advisories in the TLS/certificate and
QUIC dependencies. Cargo resolved these compatible patch updates:

| Package | Before | After | Lockfile |
| --- | --- | --- | --- |
| rustls | 0.23.43 | 0.23.46 | engine and native UI |
| rustls-webpki | 0.103.15 | 0.103.16 | engine and native UI |
| webpki-roots | 1.0.9 | 1.0.10 | engine |
| quinn-proto | 0.11.17 | 0.11.18 | engine and native UI |

The advisory groups cover TLS handshake encryption levels, session-secret Debug
output, certificate decompression allocation, later-connection key-exchange
downgrades, certificate trust and QUIC panic conditions. Sources are the OSV
records [Rustls](https://osv.dev/vulnerability/RUSTSEC-2026-0285),
[session diagnostics](https://osv.dev/vulnerability/RUSTSEC-2026-0346),
[certificate decompression](https://osv.dev/vulnerability/RUSTSEC-2026-0348),
[key exchange](https://osv.dev/vulnerability/RUSTSEC-2026-0349),
[WebPKI](https://osv.dev/vulnerability/RUSTSEC-2026-0382),
[root certificates](https://osv.dev/vulnerability/RUSTSEC-2026-0381) and
[Quinn](https://osv.dev/vulnerability/RUSTSEC-2026-0338).

Quinn is absent from the engine's current Android normal/build dependency graph;
its presence in a lockfile alone was not proof of an active runtime path. The
patch still protects later configurations that enable it. No feature or TLS
backend was changed. Cargo also changed the native UI tempfile dependency edge
from the already-locked getrandom 0.4.3 to 0.3.4 during resolution.

The post-update scan found no blocking dependency vulnerability groups.

## Remaining maintenance notices

OSV/RustSec still marks these native UI dependencies as unmaintained. These are
reported warnings rather than silently ignored records or confirmed exploitable
vulnerabilities. Replacing them requires upstream GPUI/dependency work and review
of platform reachability.

| Dependency | Version | Advisory |
| --- | --- | --- |
| instant | 0.1.13 | RUSTSEC-2024-0384 |
| paste | 1.0.15 | RUSTSEC-2024-0436 |
| rustls-pemfile | 2.2.0 | RUSTSEC-2025-0134 |
| rustybuzz | 0.20.1 | RUSTSEC-2026-0206 |
| ttf-parser | 0.25.1 | RUSTSEC-2026-0192 |
| zbus_names | 4.3.4 | RUSTSEC-2026-0371 |
| zvariant | 5.15.0 | RUSTSEC-2026-0370 |
| zvariant_derive | 5.15.0 | RUSTSEC-2026-0372 |

## Validation

- Gitleaks 8.30.1: no findings in all 17 available commits or the eligible
  tracked/unignored working-tree source/config snapshot. Ignored private files
  and media were not inspected. Secret values are fully redacted by the tool.
- Opengrep Core 1.31.0: all six local rules pass positive/negative synthetic
  fixtures. The production scan covers 20 JavaScript, 33 Java and 90 Rust files;
  it reports no matches or scanner/parser errors for this focused rule set.
- OSV-Scanner 2.7.0: engine Cargo, native UI Cargo and UI npm lockfiles scanned.
  Rust call analysis was disabled; no project build scripts ran as scanner root.
- Engine Android library tests after the dependency patches: 95 passed,
  3 intentionally ignored network-policy cases.
- Native UI Cargo resolution and `cargo metadata --offline --locked --no-deps`
  succeeded. A complete native GPUI/APK build was not performed in this audit.
- Node tooling lint and rule fixtures pass. CI configuration was added locally;
  GitHub Actions has not been run for these unpushed changes.

See [Security checks](README.md) for the repeatable commands, exact scope,
failure policy and private report location. Findings are evidence for review,
not proof that other vulnerabilities or embedded secrets cannot exist.
