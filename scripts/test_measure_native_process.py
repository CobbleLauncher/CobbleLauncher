# SPDX-License-Identifier: MIT
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


class NativeSamplerTests(unittest.TestCase):
    @unittest.skipUnless(sys.platform == 'win32', 'Windows working-set APIs')
    def test_owned_helper_samples_private_resident_pages_and_exits(self):
        helper = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(20)'], creationflags=subprocess.CREATE_NO_WINDOW)
        try:
            with tempfile.TemporaryDirectory(prefix='cobble-native-sampler-') as folder:
                report = Path(folder) / 'measurement.json'
                result = subprocess.run([sys.executable, str(Path(__file__).with_name('measure-native-process.py')),
                                         '--pid', str(helper.pid), '--seconds', '1.2', '--interval', '.2', '--report', str(report)],
                                        check=True, capture_output=True, text=True)
                self.assertIn('private_working_set_median_bytes', result.stdout)
                data = json.loads(report.read_text())
                self.assertGreater(data['private_working_set_median_bytes'], 0)
                self.assertIsNone(data['process_to_home_ready_ms'])
                self.assertGreaterEqual(len(data['samples']), 3)
                self.assertTrue(all(row['process_count'] >= 1 and row['complete'] for row in data['samples']), data['samples'])
        finally:
            helper.terminate()
            helper.wait(timeout=5)
        self.assertIsNotNone(helper.returncode)


if __name__ == '__main__':
    unittest.main()
