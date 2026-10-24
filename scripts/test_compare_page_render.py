import contextlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('compare', Path(__file__).with_name('compare-page-render.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


def payload(checksum=42):
    return json.dumps(dict(workload='playlist-preview-100', samples=11,
                           firstNs=100, medianNs=50, p95Ns=80, minNs=30, maxNs=90, checksum=checksum))


class ComparisonTests(unittest.TestCase):
    def test_both_rss_formats(self):
        for label in ('Max RSS (KiB)', 'Maximum resident set size (kbytes)'):
            self.assertEqual(module.observation(payload(), label + ': 123', 'playlist-preview-100', 11)['peakRssBytes'], 123*1024)

    def test_paired_deltas_allow_regressions(self):
        fields = ('firstNs', 'medianNs', 'p95Ns', 'peakRssBytes')
        pairs = [{'before': dict.fromkeys(fields, b), 'after': dict.fromkeys(fields, a)}
                 for b, a in ((100, 130), (200, 180), (150, 160))]
        result = module.summarize(pairs)['peakRssBytes']
        self.assertEqual(result['pairedDeltaMedian'], 10)
        self.assertEqual(result['pairedDeltaRange'], [-20, 30])

    def test_invalid_observation(self):
        for stdout, stderr, samples in ((payload(), '', 11), (payload(), 'Max RSS (KiB): 12', 12),
                                         (payload().replace('"medianNs": 50', '"medianNs": -1'), 'Max RSS (KiB): 12', 11)):
            with self.assertRaises(ValueError):
                module.observation(stdout, stderr, 'playlist-preview-100', samples)

    def run_fixture(self, mismatch=False, failure=False):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            for name in ('before', 'after'):
                (base/name).write_text(name)
            commands = []
            def run(command, **kwargs):
                commands.append(command)
                return subprocess.CompletedProcess(command, 1 if failure else 0,
                    payload(43 if mismatch and len(commands) == 2 else 42), 'Max RSS (KiB): 123')
            argv = ['compare', '--before', str(base/'before'), '--after', str(base/'after'),
                    '--before-build', 'fixture', '--after-build', 'fixture', '--repeats', '2',
                    '--workloads', 'playlist-preview-100', '--output-parent', str(base/'reports')]
            with patch.object(sys, 'argv', argv), patch.object(module.subprocess, 'run', side_effect=run), contextlib.redirect_stdout(io.StringIO()):
                if mismatch or failure:
                    with self.assertRaises((ValueError, subprocess.CalledProcessError)):
                        module.main()
                else:
                    module.main()
            report = json.loads(next((base/'reports').glob('*/summary.json')).read_text())
            return report, [Path(c[-3]).name for c in commands]

    def test_alternation_and_summary(self):
        report, order = self.run_fixture()
        self.assertTrue(report['complete'])
        self.assertEqual(order, ['before', 'after', 'after', 'before'])
        self.assertEqual(report['summary']['playlist-preview-100']['peakRssBytes']['pairedDeltaMedian'], 0)

    def test_mismatch_is_incomplete(self):
        report, _ = self.run_fixture(mismatch=True)
        self.assertFalse(report['complete'])
        self.assertIn('checksum mismatch', report['error'])
        self.assertNotIn('summary', report)

    def test_failed_process_is_incomplete(self):
        report, _ = self.run_fixture(failure=True)
        self.assertFalse(report['complete'])
        self.assertNotIn('summary', report)


if __name__ == '__main__':
    unittest.main()
