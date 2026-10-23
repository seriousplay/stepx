"""Foreign-key and concurrent-write tests for the single Play BLOB override."""

import importlib.util
import json
from pathlib import Path
import sqlite3
import tempfile
import unittest


SOURCE = Path(__file__).resolve().parents[2] / "scripts/google_play_compat_store_db.py"
SPEC = importlib.util.spec_from_file_location("store_compat_test", SOURCE)
compat = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(compat)

SCHEMA = """
PRAGMA foreign_keys=ON;
PRAGMA user_version=1035;
CREATE TABLE accounts(account_id INTEGER PRIMARY KEY,name TEXT UNIQUE NOT NULL);
CREATE TABLE config_packages(config_package_id INTEGER PRIMARY KEY,name TEXT UNIQUE NOT NULL,
 committed_experiment_state_id INTEGER REFERENCES experiment_states(experiment_state_id));
CREATE TABLE experiment_states(experiment_state_id INTEGER PRIMARY KEY,account_id INTEGER NOT NULL
 REFERENCES accounts(account_id),config_package_id INTEGER NOT NULL REFERENCES config_packages(config_package_id),
 server_token TEXT DEFAULT 'private-server-token');
CREATE TABLE flag_overrides(override_id INTEGER PRIMARY KEY,
 config_package_id INTEGER REFERENCES config_packages(config_package_id),config_package_name TEXT,
 account_id INTEGER NOT NULL REFERENCES accounts(account_id),active INTEGER DEFAULT 1,
 name TEXT NOT NULL,value NOT NULL,type INTEGER NOT NULL,source INTEGER DEFAULT 0,
 UNIQUE(config_package_id,account_id,active,name),UNIQUE(config_package_name,account_id,active,name),
 CHECK(active IS NULL OR active IS 1),CHECK((config_package_id IS NULL)!=(config_package_name IS NULL)));
CREATE TABLE experiment_states_to_overrides(experiment_state_id INTEGER NOT NULL
 REFERENCES experiment_states(experiment_state_id),override_id INTEGER NOT NULL
 REFERENCES flag_overrides(override_id),PRIMARY KEY(experiment_state_id,override_id)) WITHOUT ROWID;
CREATE TABLE flag_overrides_to_commit(override_id INTEGER NOT NULL REFERENCES flag_overrides(override_id),
 config_package_id INTEGER NOT NULL REFERENCES config_packages(config_package_id),account_id INTEGER NOT NULL
 REFERENCES accounts(account_id),PRIMARY KEY(override_id,config_package_id,account_id)) WITHOUT ROWID;
INSERT INTO accounts VALUES(0,''),(9,'private-account@example.invalid'),(19,'unrelated@example.invalid');
INSERT INTO config_packages VALUES(17,'com.google.android.finsky.regular',NULL),(18,'unrelated.namespace',NULL);
INSERT INTO experiment_states(experiment_state_id,account_id,config_package_id)
 VALUES(72,9,17),(73,9,17),(74,0,17),(75,19,18),(76,19,17);
UPDATE config_packages SET committed_experiment_state_id=72 WHERE config_package_id=17;
UPDATE config_packages SET committed_experiment_state_id=75 WHERE config_package_id=18;
CREATE TABLE unrelated(value TEXT);
INSERT INTO unrelated VALUES('original');
"""


class PlayStoreOverrideTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.path = Path(self.directory.name) / "phenotype.db"
        self.db = sqlite3.connect(self.path)
        self.addCleanup(self.db.close)
        self.db.executescript(SCHEMA)

    def existing(self, value=b"original activity list", account=9, linked=True, active=1):
        cursor = self.db.execute("INSERT INTO flag_overrides "
            "(config_package_id,account_id,active,name,value,type) VALUES (17,?,?,?,?,5)",
            (account, active, compat.FLAG_NAME, value))
        identity = cursor.lastrowid
        if linked:
            self.db.execute("INSERT INTO experiment_states_to_overrides VALUES(72,?)", (identity,))
        self.db.commit()
        return identity

    def test_plan_read_only_does_not_read_account_names_or_tokens(self):
        before = list(self.db.iterdump())

        def authorize(action, table, column, database, trigger):
            if action == sqlite3.SQLITE_READ and ((table == "accounts" and column == "name")
                                                or column == "server_token"):
                return sqlite3.SQLITE_DENY
            return sqlite3.SQLITE_OK

        self.db.set_authorizer(authorize)
        baseline = compat.plan(self.db)
        self.db.set_authorizer(None)
        self.assertEqual(baseline["state"]["account_id"], 9)
        self.assertNotIn("private-account", json.dumps(baseline))
        self.assertNotIn("private-server-token", json.dumps(baseline))
        self.assertEqual(list(self.db.iterdump()), before)

    def test_apply_links_empty_blob_to_exact_committed_account_only(self):
        receipt = compat.apply(self.db, compat.plan(self.db))
        self.assertTrue(compat.is_active(receipt["after"]))
        row = self.db.execute("SELECT account_id,type,typeof(value),length(value),source "
                              "FROM flag_overrides").fetchone()
        self.assertEqual(row, (9, 5, "blob", 0, 0))
        self.assertEqual(self.db.execute("SELECT experiment_state_id FROM experiment_states_to_overrides").fetchall(), [(72,)])
        self.assertEqual(self.db.execute("SELECT count(*) FROM flag_overrides_to_commit").fetchone()[0], 0)
        self.assertEqual(self.db.execute("PRAGMA foreign_key_check").fetchall(), [])

    def test_serialized_receipt_retry_and_rollback_are_idempotent(self):
        baseline = compat.plan(self.db)
        receipt = json.loads(json.dumps(compat.apply(self.db, baseline)))
        self.assertEqual(compat.apply(self.db, baseline, receipt=receipt), receipt)
        self.assertTrue(compat.rollback(self.db, receipt)["rolled_back"])
        self.assertTrue(compat.rollback(self.db, receipt)["already_restored"])
        self.assertEqual(compat.plan(self.db), baseline)

    def test_existing_empty_override_is_not_owned_or_deleted(self):
        identity = self.existing(value=b"")
        receipt = compat.apply(self.db, compat.plan(self.db))
        self.assertFalse(receipt["changed"])
        compat.rollback(self.db, receipt)
        self.assertEqual(self.db.execute("SELECT override_id FROM flag_overrides").fetchone()[0], identity)

    def test_existing_binary_value_and_all_original_links_restore_exactly(self):
        original = b"\x00\xff\x80original\x00"
        identity = self.existing(value=original, linked=False)
        self.db.execute("INSERT INTO experiment_states_to_overrides VALUES(73,?)", (identity,))
        self.db.commit()
        baseline = compat.plan(self.db)
        receipt = compat.apply(self.db, baseline)
        self.assertTrue(receipt["changed_value"])
        self.assertTrue(receipt["added_link"])
        compat.rollback(self.db, json.loads(json.dumps(receipt)))
        self.assertEqual(compat.plan(self.db), baseline)
        self.assertEqual(self.db.execute("SELECT value FROM flag_overrides").fetchone()[0], original)

    def test_reserved_or_mismatched_account_override_is_never_claimed(self):
        for account in (0, 19):
            with self.subTest(account=account):
                identity = self.existing(account=account)
                with self.assertRaises(compat.CompatibilityError):
                    compat.plan(self.db)
                self.db.execute("DELETE FROM experiment_states_to_overrides WHERE override_id=?", (identity,))
                self.db.execute("DELETE FROM flag_overrides WHERE override_id=?", (identity,))
                self.db.commit()

    def test_same_named_committed_and_reserved_rows_reject_ambiguous_precedence(self):
        self.existing(account=9)
        self.existing(account=0, linked=False)
        with self.assertRaisesRegex(compat.CompatibilityError, "Ambiguous"):
            compat.plan(self.db)

    def test_historical_linked_override_plus_active_row_rejected(self):
        self.existing(active=None)
        self.existing(linked=False)
        with self.assertRaisesRegex(compat.CompatibilityError, "Ambiguous"):
            compat.plan(self.db)

    def test_unrelated_account_and_namespace_survive_apply_and_rollback(self):
        identity = self.existing(account=19, linked=False)
        baseline = compat.plan(self.db)
        receipt = compat.apply(self.db, baseline)
        self.db.execute("UPDATE flag_overrides SET value=? WHERE override_id=?", (b"external edit", identity))
        self.db.execute("INSERT INTO unrelated VALUES('new external')")
        self.db.commit()
        compat.rollback(self.db, receipt)
        self.assertEqual(self.db.execute("SELECT value FROM flag_overrides WHERE override_id=?", (identity,)).fetchone()[0], b"external edit")
        self.assertEqual(self.db.execute("SELECT count(*) FROM unrelated").fetchone()[0], 2)

    def test_stale_plan_rejects_committed_state_change_and_competing_insert(self):
        baseline = compat.plan(self.db)
        self.db.execute("UPDATE config_packages SET committed_experiment_state_id=73 WHERE config_package_id=17")
        self.db.commit()
        with self.assertRaisesRegex(compat.CompatibilityError, "changed externally"):
            compat.apply(self.db, baseline)
        self.db.execute("UPDATE config_packages SET committed_experiment_state_id=72 WHERE config_package_id=17")
        self.db.commit()
        self.existing(value=b"")
        with self.assertRaisesRegex(compat.CompatibilityError, "changed externally"):
            compat.apply(self.db, baseline)

    def test_rollback_rejects_external_value_or_reference_changes(self):
        receipt = compat.apply(self.db, compat.plan(self.db))
        identity = receipt["after"]["override"]["override_id"]
        self.db.execute("UPDATE flag_overrides SET value=? WHERE override_id=?", (b"external", identity))
        self.db.commit()
        with self.assertRaises(compat.CompatibilityError):
            compat.rollback(self.db, receipt)
        self.db.execute("UPDATE flag_overrides SET value=? WHERE override_id=?", (b"", identity))
        self.db.execute("INSERT INTO experiment_states_to_overrides VALUES(73,?)", (identity,))
        self.db.commit()
        with self.assertRaises(compat.CompatibilityError):
            compat.rollback(self.db, receipt)

    def test_verified_server_fetch_sql_inherits_source_zero_without_pending_row(self):
        receipt = compat.apply(self.db, compat.plan(self.db))
        # Exact operations found in current Play blqu.F(): retire server drafts,
        # then carry active references from the account's old state into its new
        # state. This verifies the adapter's data is compatible with that SQL.
        self.db.execute("UPDATE flag_overrides SET active=NULL WHERE override_id IN "
                        "(SELECT override_id FROM experiment_states_to_overrides WHERE experiment_state_id=72) "
                        "AND source=1 AND active IS 1")
        self.db.execute("INSERT INTO experiment_states_to_overrides (experiment_state_id,override_id) "
                        "SELECT 73,override_id FROM experiment_states_to_overrides INNER JOIN flag_overrides "
                        "USING (override_id) WHERE experiment_state_id=72 AND active IS 1")
        self.db.execute("UPDATE config_packages SET committed_experiment_state_id=73 WHERE config_package_id=17")
        self.db.commit()
        self.assertTrue(compat.is_active(compat.plan(self.db)))
        self.assertEqual(self.db.execute("SELECT count(*) FROM flag_overrides_to_commit").fetchone()[0], 0)
        with self.assertRaises(compat.CompatibilityError):
            compat.rollback(self.db, receipt)
    def test_missing_state_and_wrong_blob_storage_rejected(self):
        self.db.execute("UPDATE config_packages SET committed_experiment_state_id=NULL WHERE config_package_id=17")
        self.db.commit()
        with self.assertRaisesRegex(compat.CompatibilityError, "no committed state"):
            compat.plan(self.db)
        self.db.execute("UPDATE config_packages SET committed_experiment_state_id=72 WHERE config_package_id=17")
        self.db.commit()
        self.existing(value="")
        with self.assertRaisesRegex(compat.CompatibilityError, "BLOB"):
            compat.plan(self.db)

    def test_before_commit_has_exact_id_and_failed_journal_rolls_back(self):
        saved = []

        def callback(receipt):
            saved.append(json.loads(json.dumps(receipt)))
            with sqlite3.connect(self.path) as other:
                self.assertEqual(other.execute("SELECT count(*) FROM flag_overrides").fetchone()[0], 0)
            self.assertTrue(compat.is_active(receipt["after"]))
            raise OSError("journal fsync failed")

        baseline = compat.plan(self.db)
        with self.assertRaises(OSError):
            compat.apply(self.db, baseline, before_commit=callback)
        self.assertEqual(compat.plan(self.db), baseline)
        self.assertTrue(compat.rollback(self.db, saved[0])["already_restored"])

    def test_source_executes_standalone_with_row_factory(self):
        namespace = {"__name__": "bounded_play_worker"}
        exec(compile(SOURCE.read_text(), str(SOURCE), "exec"), namespace)
        self.db.row_factory = sqlite3.Row
        baseline = namespace["plan"](self.db)
        receipt = namespace["apply"](self.db, baseline)
        self.assertTrue(namespace["is_active"](receipt["after"]))
        namespace["rollback"](self.db, receipt)


if __name__ == "__main__":
    unittest.main()
