"""Verify locally signed APKs before publishing a release from a staging branch."""
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tomllib

directory = Path(sys.argv[1])
metadata = json.loads((directory / "publish.json").read_text())
version = tomllib.loads(Path("Cargo.toml").read_text())["package"]["version"]
major, minor, patch = map(int, version.split("."))
version_code = str(major * 1_000_000 + minor * 1_000 + patch)
assert metadata["tag"] == "v" + version
assert re.fullmatch(r"[0-9a-f]{40}", metadata["source_commit"])
if os.environ.get("GITHUB_REF_NAME"):
    assert os.environ["GITHUB_REF_NAME"] == "release-upload/" + metadata["tag"]
expected_files = {"rustdl-next.apk", "SHA256SUMS", "notes.md"}
assert set(metadata["sha256"]) == expected_files
apk_path = directory / "rustdl-next.apk"
if not apk_path.exists():
    parts = sorted(directory.glob("rustdl-next.apk.part[0-9][0-9]"))
    assert parts, "Missing APK upload parts"
    assert [p.name for p in parts] == [f"rustdl-next.apk.part{i:02d}" for i in range(len(parts))]
    with apk_path.open("wb") as output:
        for part in parts:
            output.write(part.read_bytes())
for name, expected in metadata["sha256"].items():
    assert hashlib.sha256((directory / name).read_bytes()).hexdigest() == expected, name

sdk = os.environ.get("ANDROID_HOME") or os.environ.get("ANDROID_SDK_ROOT")
build_tools = sorted((Path(sdk) / "build-tools").glob("*"))[-1] if sdk else None
def tool(name):
    return str(build_tools / name) if build_tools else name

for name, package in [("rustdl-next.apk", "app.rustdl.next")]:
    apk = str(directory / name)
    badging = subprocess.check_output([tool("aapt2"), "dump", "badging", apk], text=True)
    first_line = badging.splitlines()[0]
    assert f"name='{package}'" in first_line
    assert f"versionName='{version}'" in first_line
    assert f"versionCode='{version_code}'" in first_line
    signing = subprocess.check_output([tool("apksigner"), "verify", "--min-sdk-version", "29", "--print-certs", apk], text=True)
    certificate = re.search(r"(?:Signer #1|V3\.0 Signer):? certificate SHA-256 digest: ([0-9a-f]+)", signing)
    assert certificate and certificate.group(1) == metadata["signing_certificate_sha256"]

checksums = (directory / "SHA256SUMS").read_text().splitlines()
assert checksums == [f"{metadata['sha256']['rustdl-next.apk']}  rustdl-next.apk"]
print(f"Verified v{version}: Next package ID, version, signature and hashes")
