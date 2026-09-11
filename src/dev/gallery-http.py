"""Benchmark a separate local server using synthetic filenames, never user media."""
import hashlib
import json
import math
from pathlib import Path
import socket
import statistics
import subprocess
import tempfile
import time
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
DEST = ROOT / 'target/gallery-performance'
DEST.mkdir(parents=True, exist_ok=True)
BASE = 'http://127.0.0.1:39080'

def fetch(path):
    start = time.perf_counter()
    with urllib.request.urlopen(BASE + path, timeout=15) as response:
        headers = time.perf_counter()
        data = response.read()
    end = time.perf_counter()
    return data, {'ttfb_ms': (headers-start)*1000, 'total_ms': (end-start)*1000, 'bytes': len(data)}

def metrics():
    return json.loads(fetch('/__app/gallery-metrics.json')[0])

results = []
for count in [0, 38, 100, 1000, 5000]:
    with tempfile.TemporaryDirectory(prefix='synthetic-', dir=DEST) as directory:
        folder = Path(directory)
        # Filename/metadata fixtures only; no request ever opens these as media.
        for i in range(count):
            (folder / f'{i+1}-1.m4a').write_bytes(b'synthetic')
        with open(DEST / 'synthetic-server.log', 'w') as log:
            process = subprocess.Popen([str(ROOT / 'target/release/rustdl'), 'serve', '--bind', '127.0.0.1:39080', '--output-dir', directory], stdout=log, stderr=log)
            try:
                for attempt in range(100):
                    if process.poll() is not None:
                        raise RuntimeError('Synthetic server exited; inspect synthetic-server.log')
                    try:
                        with socket.create_connection(('127.0.0.1',39080), timeout=.1):
                            break
                    except OSError:
                        time.sleep(.05)
                else:
                    raise RuntimeError('Synthetic server did not start')
                data, first = fetch('/')
                first_metrics = metrics()
                assert first_metrics['lastItemCount'] == count, first_metrics
                samples = []
                render = []
                for _ in range(30):
                    data, timing = fetch('/')
                    samples.append(timing)
                    render.append(metrics()['lastRenderMicros']/1000)
                    time.sleep(.025)
                (folder / f'{count+1}-1.m4a').write_bytes(b'synthetic')
                time.sleep(.5)
                _, after_import = fetch('/')
                assert metrics()['lastItemCount'] == count+1
                row = {'items':count, 'first_request':first, 'first_render_ms':first_metrics['lastRenderMicros']/1000,
                       'warm_samples':len(samples), 'warm_median_ms':statistics.median(x['total_ms'] for x in samples),
                       'warm_p95_ms':sorted(x['total_ms'] for x in samples)[math.ceil(.95*len(samples))-1],
                       'warm_ttfb_median_ms':statistics.median(x['ttfb_ms'] for x in samples),
                       'warm_render_median_ms':statistics.median(render), 'after_import_ms':after_import['total_ms'],
                       'response_sha256':hashlib.sha256(data).hexdigest()}
                results.append(row)
                print(json.dumps(row), flush=True)
            finally:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
(DEST/'http.json').write_text(json.dumps({'environment':'Release server on this phone; localhost; synthetic filenames; no images or media requested; first request is not a disk-cold boot', 'results':results},indent=2))
