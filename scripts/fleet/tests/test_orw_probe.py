#!/usr/bin/env python3
"""
Regression test suite for scripts/fleet/orw-probe.sh.
Verifies drift detection contract:
  - Exit 0 when local pin matches latest upstream (UP_TO_DATE)
  - Exit 1 when local pin differs from latest upstream (DRIFT_DETECTED)
  - Exit 2 when registry lookup fails or returns empty/invalid (REFUSED)
"""

import json
import os
import stat
import subprocess
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
ORW_PROBE_SCRIPT = REPO_ROOT / "scripts" / "fleet" / "orw-probe.sh"


class TestOrwProbe(unittest.TestCase):
    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.test_root = Path(self.temp_dir.name)
        self.bin_dir = self.test_root / "bin"
        self.bin_dir.mkdir(parents=True)
        self.config_dir = self.test_root / ".config" / "opencode"
        self.config_dir.mkdir(parents=True)

    def tearDown(self):
        self.temp_dir.cleanup()

    def _create_mock_curl(self, stdout_payload: str, exit_code: int = 0):
        mock_curl = self.bin_dir / "curl"
        with open(mock_curl, "w") as f:
            f.write(f"""#!/bin/sh
if [ "{exit_code}" != "0" ]; then
    exit {exit_code}
fi
cat << 'OUT'
{stdout_payload}
OUT
exit 0
""")
        mock_curl.chmod(mock_curl.stat().st_mode | stat.S_IEXEC)

    def _write_opencode_json(self, version_pin: str):
        config_path = self.config_dir / "opencode.json"
        with open(config_path, "w") as f:
            json.dump({
                "plugin": [
                    f"@cortexkit/opencode-magic-context@{version_pin}"
                ]
            }, f, indent=2)

    def _make_checkout(self, sha_msg="init"):
        repo = self.test_root / "mc-repo"
        repo.mkdir(exist_ok=True)
        git = ["git", "-C", str(repo), "-c", "user.name=t", "-c", "user.email=t@t"]
        subprocess.run(["git", "init", "-q", str(repo)], check=True)
        subprocess.run(git + ["commit", "-q", "--allow-empty", "-m", sha_msg], check=True)
        return repo, subprocess.check_output(git + ["rev-parse", "HEAD"], text=True).strip()

    def _make_ck_mc(self, sha: str):
        binary = self.bin_dir / "ck-mc"
        binary.write_text(f"#!/bin/sh\necho 'ck-mc 0.1.0 ({sha})'\n")
        binary.chmod(binary.stat().st_mode | stat.S_IEXEC)
        return binary

    def _run_probe(self, ck_mc_bin=None, mc_repo=None):
        env = os.environ.copy()
        env["PATH"] = f"{self.bin_dir}:{env.get('PATH', '')}"
        env["HOME"] = str(self.test_root)
        env["CK_MC_BIN"] = str(ck_mc_bin or self.test_root / "absent-ck-mc")
        env["MC_REPO"] = str(mc_repo or self.test_root / "absent-repo")
        return subprocess.run(
            ["bash", str(ORW_PROBE_SCRIPT)],
            capture_output=True,
            text=True,
            env=env,
        )

    def test_up_to_date_exits_0(self):
        self._write_opencode_json("0.47.0")
        self._create_mock_curl('{"dist-tags":{"latest":"0.47.0"}}')
        proc = self._run_probe()
        self.assertEqual(proc.returncode, 0, f"Expected exit 0, got {proc.returncode}. Output:\n{proc.stdout}\n{proc.stderr}")
        self.assertIn("STATUS: UP_TO_DATE (0.47.0)", proc.stdout)

    def test_drift_detected_exits_1(self):
        self._write_opencode_json("0.46.1")
        self._create_mock_curl('{"dist-tags":{"latest":"0.47.0"}}')
        proc = self._run_probe()
        self.assertEqual(proc.returncode, 1, f"Expected exit 1, got {proc.returncode}. Output:\n{proc.stdout}\n{proc.stderr}")
        self.assertIn("STATUS: DRIFT_DETECTED (installed: 0.46.1, latest: 0.47.0)", proc.stdout)

    def test_refused_on_network_failure_exits_2(self):
        self._write_opencode_json("0.47.0")
        self._create_mock_curl("", exit_code=7)
        proc = self._run_probe()
        self.assertEqual(proc.returncode, 2, f"Expected exit 2, got {proc.returncode}. Output:\n{proc.stdout}\n{proc.stderr}")
        self.assertIn("orw-probe: REFUSED (npm registry unreachable)", proc.stderr)

    def test_refused_on_empty_payload_exits_2(self):
        self._write_opencode_json("0.47.0")
        self._create_mock_curl("")
        proc = self._run_probe()
        self.assertEqual(proc.returncode, 2, f"Expected exit 2, got {proc.returncode}. Output:\n{proc.stdout}\n{proc.stderr}")
        self.assertIn("orw-probe: REFUSED (npm registry unreachable)", proc.stderr)

    def test_ck_mc_stale_build_is_drift_exit_1(self):
        self._write_opencode_json("0.48.0")
        self._create_mock_curl('{"dist-tags":{"latest":"0.48.0"}}')
        repo, _ = self._make_checkout()
        binary = self._make_ck_mc("2" * 40)
        proc = self._run_probe(ck_mc_bin=binary, mc_repo=repo)
        self.assertEqual(proc.returncode, 1, f"{proc.stdout}\n{proc.stderr}")
        self.assertIn("STATUS: UP_TO_DATE (0.48.0)", proc.stdout)
        self.assertIn("ck-mc: DRIFT_DETECTED (installed build: 222222222", proc.stdout)

    def test_ck_mc_matching_build_exits_0(self):
        self._write_opencode_json("0.48.0")
        self._create_mock_curl('{"dist-tags":{"latest":"0.48.0"}}')
        repo, sha = self._make_checkout()
        binary = self._make_ck_mc(sha)
        proc = self._run_probe(ck_mc_bin=binary, mc_repo=repo)
        self.assertEqual(proc.returncode, 0, f"{proc.stdout}\n{proc.stderr}")
        self.assertIn(f"ck-mc: UP_TO_DATE ({sha[:9]})", proc.stdout)

    def test_ck_mc_missing_binary_is_skipped_not_failed(self):
        self._write_opencode_json("0.48.0")
        self._create_mock_curl('{"dist-tags":{"latest":"0.48.0"}}')
        repo, _ = self._make_checkout()
        proc = self._run_probe(mc_repo=repo)
        self.assertEqual(proc.returncode, 0, f"{proc.stdout}\n{proc.stderr}")
        self.assertIn("ck-mc: SKIPPED (binary not found", proc.stdout)


if __name__ == "__main__":
    unittest.main()
