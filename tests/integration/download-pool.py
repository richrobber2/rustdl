#!/usr/bin/env python3
"""Exercise 500 real local HTTP downloads, queue controls, retries and restart.

Run after `cargo build --release`: python3 tests/integration/download-pool.py
Uses an isolated temporary library and synthetic bytes; no external requests.
"""
import collections
import http.server
import json
import pathlib
import socket
import subprocess
import tempfile
import threading
import time
import urllib.parse
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[2]
PAYLOAD = b"rustdl worker pool fixture\n" * 256
RELEASE = threading.Event()
LOCK = threading.Lock()
COUNTS = collections.Counter()
ACTIVE = collections.Counter()
RANGES = []
FAIL_ONCE = set()


class Source(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        key = self.path.rsplit("/", 1)[-1]
        with LOCK:
            COUNTS["requests"] += 1
            ACTIVE[key] += 1
            COUNTS["peakSourceRequests"] = max(COUNTS["peakSourceRequests"], sum(ACTIVE.values()))
            COUNTS["peakPerFile"] = max(COUNTS["peakPerFile"], ACTIVE[key])
            fail = key in FAIL_ONCE
            FAIL_ONCE.discard(key)
        try:
            if fail:
                self.send_error(503, "intentional test failure")
                return
            start = int(self.headers.get("Range", "bytes=0-").split("=")[1].split("-")[0])
            if start:
                with LOCK:
                    RANGES.append((key, start))
            self.send_response(206 if start else 200)
            self.send_header("Content-Length", str(len(PAYLOAD) - start))
            if start:
                self.send_header("Content-Range", f"bytes {start}-{len(PAYLOAD)-1}/{len(PAYLOAD)}")
            self.end_headers()
            held = not RELEASE.is_set()
            self.wfile.write(PAYLOAD[start:start + 1024])
            self.wfile.flush()
            if not RELEASE.wait(30):
                raise TimeoutError("test did not release HTTP fixture")
            time.sleep(0.025)
            if held:
                self.wfile.write(PAYLOAD[start + 1024:start + 2048])
                self.wfile.flush()
                time.sleep(0.05)  # Let the client observe pause/cancel mid-body.
                self.wfile.write(PAYLOAD[start + 2048:])
            else:
                self.wfile.write(PAYLOAD[start + 1024:])
        except (BrokenPipeError, ConnectionResetError):
            pass  # Paused and cancelled clients close their response.
        finally:
            with LOCK:
                ACTIVE[key] -= 1

    def log_message(self, *_):
        pass


def main():
    source = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Source)
    threading.Thread(target=source.serve_forever, daemon=True).start()
    report = {"items": 500, "peakThreads": 0, "peakRssKiB": 0, "peakVirtualKiB": 0, "peakDownloadWorkers": 0, "peakActiveDownloads": 0}
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        port = sock.getsockname()[1]
    base = f"http://127.0.0.1:{port}"
    process = None
    with tempfile.TemporaryDirectory(prefix="download-pool-", dir=ROOT / "target") as temp:
        directory = pathlib.Path(temp)
        jobs = {
            f"youtube-poolxx{i:05}.mp4": {
                "phase": "Queued", "downloaded": 0, "total": len(PAYLOAD),
                "error": None, "source_url": None,
                "media_url": f"http://127.0.0.1:{source.server_port}/fixture/{i}",
            } for i in range(500)
        }
        (directory / ".queue.json").write_text(json.dumps(jobs))
        log = (ROOT / "target/download-pool-integration.log").open("w")

        def launch():
            return subprocess.Popen(
                [str(ROOT / "target/release/rustdl"), "serve", "--bind", f"127.0.0.1:{port}",
                 "--output-dir", str(directory)], stdout=log, stderr=log)

        def state():
            assert process.poll() is None, "CLI exited unexpectedly"
            status = pathlib.Path(f"/proc/{process.pid}/status").read_text()
            fields = {line.split(":")[0]: int(line.split()[1]) for line in status.splitlines()
                      if line.startswith(("Threads:", "VmRSS:", "VmSize:"))}
            for metric, field in [("peakThreads", "Threads"), ("peakRssKiB", "VmRSS"),
                                  ("peakVirtualKiB", "VmSize")]:
                report[metric] = max(report[metric], fields.get(field, 0))
            worker_count = 0
            for path in pathlib.Path(f"/proc/{process.pid}/task").glob("*/comm"):
                try:
                    worker_count += path.read_text().strip().startswith("download-")
                except FileNotFoundError:
                    pass
            report["peakDownloadWorkers"] = max(report["peakDownloadWorkers"], worker_count)
            with urllib.request.urlopen(base + "/__app/state.json", timeout=2) as response:
                current = {job["filename"]: job for job in json.load(response)["jobs"]}
                report["peakActiveDownloads"] = max(report["peakActiveDownloads"], sum(
                    job["phase"] in ("starting", "downloading") for job in current.values()))
                return current

        def until(predicate, timeout=90):
            deadline = time.monotonic() + timeout
            latest = {}
            while time.monotonic() < deadline:
                try:
                    latest = state()
                    if predicate(latest):
                        return latest
                except OSError:
                    pass
                time.sleep(0.05)
            raise AssertionError(f"queue timed out: {collections.Counter(j['phase'] for j in latest.values())}")

        def action(filename, value):
            query = urllib.parse.urlencode({"file": filename, "action": value})
            with urllib.request.urlopen(base + "/queue/action?" + query, timeout=5) as response:
                assert response.status == 200

        def stop():
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()

        started = time.monotonic()
        try:
            process = launch()
            current = until(lambda s: sum(j["phase"] == "downloading" and j["downloaded"] > 0
                                          for j in s.values()) == 2)
            active = [name for name, job in current.items() if job["phase"] == "downloading"]
            queued = [name for name, job in current.items() if job["phase"] == "queued"]
            paused = [active[0], queued[-1]]
            cancelled = [active[1], queued[-2]]
            failed = queued[-3]
            with LOCK:
                FAIL_ONCE.add(jobs[failed]["media_url"].rsplit("/", 1)[-1])
            for name in paused:
                action(name, "pause")
            for name in cancelled:
                action(name, "cancel")
            RELEASE.set()
            current = until(lambda s: collections.Counter(j["phase"] for j in s.values()) ==
                            {"ready": 495, "paused": 2, "cancelled": 2, "failed": 1})
            assert 0 < (directory / (paused[0] + ".part")).stat().st_size < len(PAYLOAD)
            assert not (directory / (cancelled[0] + ".part")).exists()
            assert current[failed]["error"]
            action(failed, "resume")
            until(lambda s: s[failed]["phase"] == "ready")
            report["controlsPassed"] = True

            # A process restart must retain paused/cancelled/failed state. Change
            # the failed item to an interrupted transfer to test automatic restore.
            until(lambda _: json.loads((directory / ".queue.json").read_text())[failed]["phase"] == "Ready")
            stop()
            persisted = json.loads((directory / ".queue.json").read_text())
            (directory / failed).unlink()  # Remove only this test's synthetic file.
            persisted[failed]["phase"] = "Downloading"
            (directory / ".queue.json").write_text(json.dumps(persisted))
            process = launch()
            current = until(lambda s: s.get(failed, {}).get("phase") == "ready")
            assert all(current[name]["phase"] == "paused" for name in paused)
            assert all(current[name]["phase"] == "cancelled" for name in cancelled)
            for name in paused + cancelled:
                action(name, "resume")
            until(lambda s: len(s) == 500 and all(j["phase"] == "ready" for j in s.values()))
            report["restartPassed"] = True
            report["verifiedFiles"] = sum((directory / name).read_bytes() == PAYLOAD for name in jobs)
            report["rangeResumePassed"] = any(key == jobs[paused[0]]["media_url"].rsplit("/", 1)[-1]
                                              and offset > 0 for key, offset in RANGES)
            until(lambda _: all(j["phase"] == "Ready" for j in
                                json.loads((directory / ".queue.json").read_text()).values()))
            report.update(COUNTS)
            report["elapsedSeconds"] = round(time.monotonic() - started, 3)
            assert report["verifiedFiles"] == 500
            assert report["rangeResumePassed"]
            assert report["peakDownloadWorkers"] == 3
            # Source handlers may briefly outlive a paused/cancelled client.
            # Count client transfers, not those closing server connections.
            assert report["peakActiveDownloads"] <= 2
            assert report["peakPerFile"] == 1
            assert report["peakThreads"] < 50, "queue still creates too many threads"
            report["passed"] = True
        finally:
            RELEASE.set()
            if process is not None and process.poll() is None:
                stop()
            source.shutdown()
            log.close()
            (ROOT / "target/download-pool-integration.json").write_text(json.dumps(report, indent=2) + "\n")
            print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
