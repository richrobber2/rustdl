#!/usr/bin/env python3
"""Capture isolated synthetic GPUI fixtures through a specific authorized ADB endpoint."""
import argparse
import os
import pathlib
import subprocess
import time

ROOT = pathlib.Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument("--serial", required=True)
parser.add_argument("--screen", choices=("home", "history", "settings", "anime"), default="home")
parser.add_argument("--output", required=True, type=pathlib.Path)
args = parser.parse_args()
adb_root = ROOT / "target/adb-check/extracted/data/data/com.termux/files/usr"
env = dict(os.environ, LD_LIBRARY_PATH=str(adb_root / "lib"))
prefix = [str(adb_root / "bin/adb"), "-P", "5038", "-s", args.serial]
def adb(*command):
    return subprocess.check_output(prefix + list(command), env=env)
adb("shell", "am", "start", "-W", "-n", "app.rustdl.next/app.rustdl.NativeVisualActivity",
    "--es", "screen", args.screen)
expected = ("readyScreen=private-synthetic:" + args.screen).encode()
def readiness():
    return adb("shell","content","call","--uri","content://app.rustdl.next.visual-review","--method","ready").strip()
for _ in range(100):
    try:
        ready = readiness()
        if expected in ready:
            break
    except subprocess.CalledProcessError:
        pass
    time.sleep(0.1)
else:
    raise SystemExit("No protected synthetic frame acknowledgment; capture refused.")
data = adb("exec-out", "screencap", "-p")
if expected not in readiness():
    raise SystemExit("Review activity lost focus; capture discarded.")
if not data.startswith(b"\x89PNG\r\n\x1a\n"):
    raise SystemExit("Invalid screenshot response.")
args.output.parent.mkdir(parents=True, exist_ok=True)
args.output.write_bytes(data)
print(args.output)
