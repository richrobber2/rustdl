#!/usr/bin/env python3
"""Compare synthetic RustDL page benchmarks in alternating fresh processes."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import statistics
import subprocess
import tempfile

WORKLOADS = ('playlist-preview-100', 'playlist-preview-1000', 'playlist-preview-5000')


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def observation(stdout, stderr, workload, samples):
    data = json.loads(stdout)
    if data.get('workload') != workload or data.get('samples') != samples:
        raise ValueError('Benchmark workload/sample identity mismatch')
    for field in ('firstNs', 'medianNs', 'p95Ns', 'minNs', 'maxNs', 'checksum'):
        value = data.get(field)
        if type(value) is not int or value < 0:
            raise ValueError('Invalid benchmark field: ' + field)
    if not data['minNs'] <= data['medianNs'] <= data['p95Ns'] <= data['maxNs']:
        raise ValueError('Invalid timing quantile order')
    match = re.search(r'(?:Max RSS \(KiB\)|Maximum resident set size \(kbytes\)):\s*(\d+)', stderr)
    if not match or int(match[1]) <= 0:
        raise ValueError('Missing positive peak RSS measurement')
    data['peakRssBytes'] = int(match[1]) * 1024
    return data


def summarize(pairs):
    result = {}
    for field in ('firstNs', 'medianNs', 'p95Ns', 'peakRssBytes'):
        before = [pair['before'][field] for pair in pairs]
        after = [pair['after'][field] for pair in pairs]
        delta = [a-b for a, b in zip(after, before)]
        result[field] = {'beforeMedian': statistics.median(before),
                         'afterMedian': statistics.median(after),
                         'pairedDeltaMedian': statistics.median(delta),
                         'pairedDeltaRange': [min(delta), max(delta)]}
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--before', type=Path, required=True, help='Saved normal-allocator rustdl-page-bench binary')
    parser.add_argument('--after', type=Path, required=True)
    parser.add_argument('--before-build', required=True, help='Baseline revision, profile and features')
    parser.add_argument('--after-build', required=True, help='Candidate revision, profile and features')
    parser.add_argument('--repeats', type=int, default=3)
    parser.add_argument('--samples', type=int, default=11)
    parser.add_argument('--timeout', type=int, default=90)
    parser.add_argument('--workloads', nargs='+', choices=WORKLOADS, default=list(WORKLOADS))
    parser.add_argument('--root-helper', type=Path, help='Android root.sh helper for Toybox time/timeout')
    parser.add_argument('--output-parent', type=Path, default=Path('target/page-comparisons'))
    args = parser.parse_args()
    if not 2 <= args.repeats <= 20 or not 1 <= args.samples <= 1000 or not 1 <= args.timeout <= 600:
        parser.error('repeats: 2..20; samples: 1..1000; timeout: 1..600 seconds')
    binaries = {key: getattr(args, key).resolve(strict=True) for key in ('before', 'after')}
    env = dict(os.environ, LC_ALL='C')
    if args.root_helper:
        helper = args.root_helper.resolve(strict=True)
        prefix = Path('/data/data/com.termux/files/usr')
        env['LD_LIBRARY_PATH'] = str(prefix/'lib') + ':' + env.get('LD_LIBRARY_PATH', '')
        command_prefix = ['sh', str(helper), '/system/bin/timeout', '-k', '5', str(args.timeout),
                          '/system/bin/time', '-v', str(prefix/'bin/env'), 'LC_ALL=C',
                          'LD_LIBRARY_PATH=' + env['LD_LIBRARY_PATH']]
    else:
        command_prefix = ['/usr/bin/timeout', '-k', '5', str(args.timeout), '/usr/bin/time', '-v']
    os.umask(0o077)
    args.output_parent.mkdir(parents=True, exist_ok=True)
    output = Path(tempfile.mkdtemp(prefix='render-', dir=args.output_parent)).resolve()
    report = {'complete': False, 'binarySha256': {k: sha(v) for k, v in binaries.items()},
              'binaries': {k: str(v) for k, v in binaries.items()},
              'buildDescriptions': {'before': args.before_build, 'after': args.after_build},
              'environment': {'system': platform.system(), 'kernel': platform.release(), 'machine': platform.machine(),
                              'measurementBackend': 'Android root Toybox' if args.root_helper else 'GNU time'},
              'samples': args.samples, 'repeats': args.repeats, 'workloads': {},
              'limitations': ['Synthetic server-side page generation; not WebView layout, painting or interaction latency.',
                              'Timing includes output validation and hashing; excludes fixture construction.',
                              'Peak RSS includes process startup, fixtures and all renders; not live heap or PSS.',
                              'Normal-allocator binaries required; build descriptions are caller supplied, not verified.',
                              'Observed paired ranges are not confidence intervals; use separate DHAT runs for allocations.']}
    try:
        for workload in dict.fromkeys(args.workloads):
            pairs = report['workloads'][workload] = []
            expected = None
            for repeat in range(args.repeats):
                pair = {}
                pairs.append(pair)
                order = ('before', 'after') if repeat % 2 == 0 else ('after', 'before')
                for label in order:
                    run = subprocess.run(command_prefix + [str(binaries[label]), workload, str(args.samples)],
                                         env=env, capture_output=True, text=True, timeout=args.timeout+15)
                    stem = output / f'{workload}-{repeat}-{label}'
                    stem.with_suffix('.stdout').write_text(run.stdout)
                    stem.with_suffix('.stderr').write_text(run.stderr)
                    run.check_returncode()
                    data = observation(run.stdout, run.stderr, workload, args.samples)
                    pair[label] = data
                    if expected is None:
                        expected = data['checksum']
                    if data['checksum'] != expected:
                        raise ValueError('Output checksum mismatch: ' + workload)
                (output/'summary.json').write_text(json.dumps(report, indent=2)+'\n')
        if any(sha(v) != report['binarySha256'][k] for k, v in binaries.items()):
            raise ValueError('Binary changed during measurement')
        report['summary'] = {k: summarize(v) for k, v in report['workloads'].items()}
        report['complete'] = True
    except Exception as error:
        report['error'] = str(error)
        raise
    finally:
        (output/'summary.json').write_text(json.dumps(report, indent=2)+'\n')
        print(output/'summary.json', flush=True)


if __name__ == '__main__':
    main()
