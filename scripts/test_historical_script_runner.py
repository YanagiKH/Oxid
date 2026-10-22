"""Controls for genuine historical evidence and exact replay discovery."""
import copy
from pathlib import Path
import tempfile
import unittest

import run_historical_script_tests as runner


def metadata():
    run = {"id": runner.RUN, "run_attempt": runner.ATTEMPT, "workflow_id": runner.WORKFLOW,
           "path": ".github/workflows/ci.yml", "head_sha": runner.BASELINE,
           "event": "schedule", "status": "completed", "conclusion": "success",
           "repository": {"full_name": runner.REPOSITORY},
           "head_repository": {"full_name": runner.REPOSITORY}}
    job = {"id": runner.JOB, "run_id": runner.RUN, "run_attempt": runner.ATTEMPT,
           "head_sha": runner.BASELINE, "name": "Full repository verification",
           "status": "completed", "conclusion": "success",
           "steps": [{"name": "Test feature status metadata validation",
                      "status": "completed", "conclusion": "success"}]}
    return run, job


class GenuineHistoricalEvidenceTests(unittest.TestCase):
    def test_exact_completed_original_run_is_accepted(self):
        run, job = metadata()
        result = runner.validate_history(run, job)
        self.assertEqual(result["head_sha"], runner.BASELINE)
        self.assertEqual(result["event"], "schedule")
        self.assertEqual(result["job_id"], runner.JOB)

    def test_wrong_identity_attempt_source_or_result_is_rejected(self):
        run, job = metadata()
        for side, original in (("run", run), ("job", job)):
            for key in original:
                if key in ("steps", "repository", "head_repository"):
                    continue
                with self.subTest(side=side, key=key):
                    changed = copy.deepcopy(original)
                    changed[key] = "wrong"
                    with self.assertRaises(ValueError):
                        runner.validate_history(changed if side == "run" else run,
                                                changed if side == "job" else job)
        for status, conclusion in (("in_progress", None), ("completed", "failure"),
                                   ("completed", "skipped"), ("completed", "cancelled")):
            changed = {**run, "status": status, "conclusion": conclusion}
            with self.assertRaises(ValueError):
                runner.validate_history(changed, job)

    def test_missing_duplicate_skipped_or_failed_step_is_rejected(self):
        run, job = metadata()
        original = job["steps"][0]
        for steps in (None, [], [original, original], [{**original, "name": "other"}],
                      [{**original, "status": "queued"}], [{**original, "conclusion": "skipped"}],
                      [{**original, "conclusion": "failure"}]):
            with self.subTest(steps=steps), self.assertRaises(ValueError):
                runner.validate_history(run, {**job, "steps": steps})

    def test_other_repository_cannot_supply_certificate(self):
        run, job = metadata()
        for field in ("repository", "head_repository"):
            for value in ({"full_name": "someone/Oxid"}, {}, None):
                with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                    runner.validate_history({**run, field: value}, job)

    def test_discovery_excludes_only_exact_hosted_module(self):
        with tempfile.TemporaryDirectory(prefix="oxid-historical-loader-") as temporary:
            root = Path(temporary)
            loader = runner.HistoricalLoader(root)
            for path, expected in ((root / "test_published_formatter_source.py", False),
                                   (root / "test_published_formatter_source_extra.py", True),
                                   (root / "test_future.py", True),
                                   (root / "nested/test_published_formatter_source.py", True),
                                   (root / "helper.py", False)):
                with self.subTest(path=path):
                    self.assertEqual(loader._match_path(path.name, str(path), "test_*.py"), expected)


if __name__ == "__main__":
    unittest.main()
