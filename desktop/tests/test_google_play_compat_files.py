"""Local filesystem acceptance tests; never access a phone or real app data."""

import importlib.util
import json
import os
from pathlib import Path
import stat
import tempfile
import unittest
from unittest import mock


SOURCE = Path(__file__).resolve().parents[2] / "scripts/google_play_compat_files.py"
SPEC = importlib.util.spec_from_file_location("google_play_compat_files_test", SOURCE)
compat = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(compat)


class CacheFilesTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = self.directory.name
        self.gms = Path(self.root) / "com.google.android.gms"
        self.play = Path(self.root) / "com.android.vending"
        self.first = self.gms / "files/phenotype/shared/com.google.android.gms.gmscompliance.pb"
        self.second = self.gms / "files/phenotype/shared/secret/0dPD7nD8E2yOLxoYWJqw_g==.pb"
        self.play_file = self.play / "files/experiment-flags-regular-account-hash"
        for path, content in ((self.first, b"first original"), (self.second, b"second original"),
                              (self.play_file, b"play original")):
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(content)
            path.chmod(0o640)
        self.paths = [str(self.first), str(self.second)]
        self.journal = str(self.gms / "files/stepx-google-compat/cache-journal.json")

    def prepare(self, paths=None, scope="gms"):
        return compat.prepare(scope, self.paths if paths is None else paths, root_base=self.root)

    def apply(self, baseline=None, replacements=None):
        return compat.apply("gms", baseline or self.prepare(), replacements or [b"new first", b"new second"],
                            self.journal, root_base=self.root)

    def rollback(self):
        return compat.rollback("gms", self.journal, root_base=self.root)

    def test_prepare_is_read_only_and_contains_exact_recovery_data(self):
        before = [path.stat() for path in (self.first, self.second)]
        baseline = self.prepare()
        self.assertEqual([compat._original(entry) for entry in baseline["entries"]],
                         [b"first original", b"second original"])
        self.assertEqual([path.stat().st_ino for path in (self.first, self.second)],
                         [info.st_ino for info in before])
        self.assertFalse(Path(self.journal).exists())
        json.dumps(baseline)

    def test_directory_ancestors_use_search_only_descriptors(self):
        original_open = os.open
        with mock.patch.object(compat.os, "open", wraps=original_open) as opened:
            with compat._directory(str(self.first.parent)) as descriptor:
                self.assertTrue(stat.S_ISDIR(os.fstat(descriptor).st_mode))
                os.fsync(descriptor)
        calls = opened.call_args_list
        self.assertGreater(len(calls), 2)
        for call in calls[:-1]:
            self.assertTrue(call.args[1] & os.O_PATH)
        self.assertFalse(calls[-1].args[1] & os.O_PATH)
        self.assertTrue(calls[-1].args[1] & os.O_NOFOLLOW)

    def test_apply_writes_durable_journal_first_and_preserves_metadata(self):
        baseline = self.prepare()
        observed = []
        original = compat._atomic_replace

        def inspect(*args, **kwargs):
            journal = json.loads(Path(self.journal).read_text())
            self.assertEqual(journal["baseline"], baseline)
            self.assertEqual(len(journal["after_sha256"]), 2)
            self.assertEqual(stat.S_IMODE(Path(self.journal).stat().st_mode), 0o600)
            observed.append(True)
            return original(*args, **kwargs)

        with mock.patch.object(compat, "_atomic_replace", side_effect=inspect):
            self.assertTrue(self.apply(baseline)["applied"])
        self.assertEqual(len(observed), 2)
        self.assertEqual(self.first.read_bytes(), b"new first")
        self.assertEqual(self.second.read_bytes(), b"new second")
        self.assertEqual(stat.S_IMODE(self.first.stat().st_mode), 0o640)
        self.assertEqual(list(self.first.parent.glob(".stepx-compat-*")), [])

    def test_multifile_cas_conflict_changes_nothing(self):
        baseline = self.prepare()
        self.second.write_bytes(b"external before apply")
        with self.assertRaises(compat.CacheCompatibilityError):
            self.apply(baseline)
        self.assertEqual(self.first.read_bytes(), b"first original")
        self.assertEqual(self.second.read_bytes(), b"external before apply")
        self.assertFalse(Path(self.journal).exists())

    def test_metadata_cas_conflict(self):
        baseline = self.prepare()
        self.second.chmod(0o600)
        with self.assertRaises(compat.CacheCompatibilityError):
            self.apply(baseline)
        self.assertEqual(self.first.read_bytes(), b"first original")

    def test_selinux_label_conflict_is_detected_before_journal(self):
        with mock.patch.object(compat, "_label", return_value="Y29udGV4dC1h"):
            baseline = self.prepare()
        with mock.patch.object(compat, "_label", return_value="Y29udGV4dC1i"):
            with self.assertRaises(compat.CacheCompatibilityError):
                self.apply(baseline)
        self.assertFalse(Path(self.journal).exists())

    def test_atomic_metadata_failure_keeps_original_and_removes_temp(self):
        baseline = self.prepare()
        with mock.patch.object(compat, "_set_metadata", side_effect=OSError("label denied")):
            with self.assertRaises(OSError):
                compat._atomic_replace(str(self.first), b"replacement",
                                       compat._expected(baseline["entries"][0]), self.root)
        self.assertEqual(self.first.read_bytes(), b"first original")
        self.assertEqual(list(self.first.parent.glob(".stepx-compat-*")), [])

    def test_per_file_cas_after_journal_restores_first_and_keeps_second(self):
        writer = compat._write_journal

        def write_then_external_update(*args, **kwargs):
            writer(*args, **kwargs)
            self.second.write_bytes(b"external after journal")

        with mock.patch.object(compat, "_write_journal", side_effect=write_then_external_update):
            with self.assertRaises(compat.CacheCompatibilityError):
                self.apply()
        self.assertEqual(self.first.read_bytes(), b"first original")
        self.assertEqual(self.second.read_bytes(), b"external after journal")

    def test_second_atomic_failure_restores_only_our_written_files(self):
        original = compat._atomic_replace
        calls = []

        def fail_after_second_rename(*args, **kwargs):
            calls.append(args[0])
            original(*args, **kwargs)
            if len(calls) == 2:
                raise OSError("simulated directory fsync failure after rename")

        with mock.patch.object(compat, "_atomic_replace", side_effect=fail_after_second_rename):
            with self.assertRaises(compat.CacheCompatibilityError) as caught:
                self.apply()
        self.assertEqual(caught.exception.conflict_indices, [])
        self.assertEqual(self.first.read_bytes(), b"first original")
        self.assertEqual(self.second.read_bytes(), b"second original")
        self.assertTrue(Path(self.journal).exists())
        self.assertEqual(self.rollback()["unchanged_indices"], [0, 1])

    def test_failure_recovery_preserves_external_change(self):
        original = compat._atomic_replace
        count = 0

        def conflict_on_second(*args, **kwargs):
            nonlocal count
            count += 1
            if count == 2:
                self.first.write_bytes(b"external after first write")
                raise OSError("simulated failure")
            return original(*args, **kwargs)

        with mock.patch.object(compat, "_atomic_replace", side_effect=conflict_on_second):
            with self.assertRaises(compat.CacheCompatibilityError) as caught:
                self.apply()
        self.assertEqual(caught.exception.conflict_indices, [0])
        self.assertEqual(self.first.read_bytes(), b"external after first write")
        self.assertEqual(self.second.read_bytes(), b"second original")

    def test_rollback_preserves_external_file_and_restores_other_file(self):
        self.apply()
        self.first.write_bytes(b"new external content")
        with self.assertRaises(compat.CacheCompatibilityError) as caught:
            self.rollback()
        self.assertEqual(caught.exception.result["conflict_indices"], [0])
        self.assertEqual(caught.exception.result["restored_indices"], [1])
        self.assertEqual(self.first.read_bytes(), b"new external content")
        self.assertEqual(self.second.read_bytes(), b"second original")

    def test_rollback_is_idempotent_and_preserves_unrelated_file(self):
        unrelated = self.first.parent / "unrelated.pb"
        unrelated.write_bytes(b"unrelated initial")
        self.apply()
        unrelated.write_bytes(b"unrelated external update")
        self.assertEqual(self.rollback()["restored_indices"], [0, 1])
        self.assertEqual(self.rollback()["unchanged_indices"], [0, 1])
        self.assertEqual(unrelated.read_bytes(), b"unrelated external update")

    def test_whitelist_rejects_other_packages_other_files_and_traversal(self):
        invalid = [str(self.gms / "databases/phenotype.db"), str(self.play_file),
                   str(self.first.parent / "other.pb"), str(self.first.parent / "../shared" / self.first.name)]
        for path in invalid:
            with self.subTest(path=path), self.assertRaises(compat.CacheCompatibilityError):
                self.prepare([path])
        with self.assertRaises(compat.CacheCompatibilityError):
            self.prepare([str(self.first), str(self.first)])

    def test_symlink_file_parent_and_hardlink_are_rejected(self):
        original = self.first.read_bytes()
        self.first.unlink()
        self.first.symlink_to(self.second)
        with self.assertRaises((OSError, compat.CacheCompatibilityError)):
            self.prepare()
        self.first.unlink()
        os.link(self.second, self.first)
        with self.assertRaises(compat.CacheCompatibilityError):
            self.prepare()
        self.first.unlink()
        self.first.write_bytes(original)
        link = self.gms / "files/link"
        link.symlink_to(self.second.parent, target_is_directory=True)
        with self.assertRaises((OSError, compat.CacheCompatibilityError)):
            self.prepare([str(link / self.second.name)])

    def test_journal_path_symlink_and_existing_journal_rejected(self):
        baseline = self.prepare()
        with self.assertRaises(compat.CacheCompatibilityError):
            compat.apply("gms", baseline, [b"a", b"b"], str(self.gms / "files/outside.json"),
                         root_base=self.root)
        journal_directory = Path(self.journal).parent
        journal_directory.symlink_to(self.second.parent, target_is_directory=True)
        with self.assertRaises(OSError):
            self.apply(baseline)
        journal_directory.unlink()
        self.apply(baseline)
        raw = Path(self.journal).read_bytes()
        with self.assertRaises(FileExistsError):
            self.apply(self.prepare())
        self.assertEqual(Path(self.journal).read_bytes(), raw)

    def test_journal_write_failure_prevents_any_cache_modification(self):
        with mock.patch.object(compat, "_write_journal", side_effect=OSError("disk full")):
            with self.assertRaises(OSError):
                self.apply()
        self.assertEqual(self.first.read_bytes(), b"first original")
        self.assertEqual(self.second.read_bytes(), b"second original")

    def test_play_scope_independent_and_source_executes_standalone(self):
        namespace = {"__name__": "bounded_cache_worker"}
        exec(compile(SOURCE.read_text(), str(SOURCE), "exec"), namespace)
        baseline = namespace["prepare"]("play", [str(self.play_file)], root_base=self.root)
        journal = str(self.play / "files/stepx-google-compat/cache.json")
        namespace["apply"]("play", baseline, [b"play new"], journal, root_base=self.root)
        self.assertEqual(self.play_file.read_bytes(), b"play new")
        namespace["rollback"]("play", journal, root_base=self.root)
        self.assertEqual(self.play_file.read_bytes(), b"play original")

    def test_play_account_filename_and_pathlib_root_are_supported(self):
        account_path = self.play_file.with_name("experiment-flags-regular-example+local@example.invalid")
        self.play_file.rename(account_path)
        baseline = compat.prepare("play", [str(account_path)], root_base=Path(self.root))
        journal = str(self.play / "files/stepx-google-compat/cache.json")
        compat.apply("play", baseline, [b"new"], journal, root_base=Path(self.root))
        compat.rollback("play", journal, root_base=Path(self.root))
        self.assertEqual(account_path.read_bytes(), b"play original")

    def test_new_journal_directory_normalized_even_when_fsuid_differs(self):
        # euid is only consulted in this simulation; actual creation still uses
        # our test user. The old euid-based check would skip normalization here.
        with mock.patch.object(compat.os, "geteuid", return_value=12345):
            self.apply()
        directory = Path(self.journal).parent
        self.assertEqual(stat.S_IMODE(directory.stat().st_mode), 0o700)
        self.assertTrue(self.rollback()["rolled_back"])


if __name__ == "__main__":
    unittest.main()
