"""Exercise the scoped DB adapter against real SQLite transactions/foreign keys."""
import importlib.util
import json
from pathlib import Path
import sqlite3
import tempfile
import unittest


SOURCE = Path(__file__).resolve().parents[2] / "scripts/google_play_compat_db.py"
SPEC = importlib.util.spec_from_file_location("google_play_compat_db", SOURCE)
compat = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(compat)

CE_SCHEMA = """
PRAGMA foreign_keys=ON;
PRAGMA user_version=1035;
CREATE TABLE accounts(account_id INTEGER PRIMARY KEY, name TEXT UNIQUE NOT NULL);
CREATE TABLE config_packages(config_package_id INTEGER PRIMARY KEY, name TEXT UNIQUE NOT NULL,
 committed_experiment_state_id INTEGER REFERENCES experiment_states(experiment_state_id));
CREATE TABLE experiment_states(experiment_state_id INTEGER PRIMARY KEY, account_id INTEGER NOT NULL
 REFERENCES accounts(account_id), config_package_id INTEGER NOT NULL REFERENCES config_packages(config_package_id),
 server_token TEXT NOT NULL DEFAULT 'private-token');
CREATE TABLE flag_overrides(override_id INTEGER PRIMARY KEY,
 config_package_id INTEGER REFERENCES config_packages(config_package_id) ON DELETE CASCADE,
 config_package_name TEXT, account_id INTEGER NOT NULL REFERENCES accounts(account_id),
 active INTEGER DEFAULT 1, name TEXT NOT NULL, value NOT NULL, type INTEGER NOT NULL, source INTEGER DEFAULT 0,
 UNIQUE(config_package_id,account_id,active,name), UNIQUE(config_package_name,account_id,active,name),
 CHECK(active IS NULL OR active IS 1), CHECK((config_package_id IS NULL)!=(config_package_name IS NULL)));
CREATE TABLE experiment_states_to_overrides(experiment_state_id INTEGER NOT NULL
 REFERENCES experiment_states(experiment_state_id) ON DELETE CASCADE,
 override_id INTEGER NOT NULL REFERENCES flag_overrides(override_id),
 PRIMARY KEY(experiment_state_id,override_id)) WITHOUT ROWID;
CREATE TABLE flag_overrides_to_commit(override_id INTEGER NOT NULL
 REFERENCES flag_overrides(override_id) ON DELETE CASCADE, config_package_id INTEGER NOT NULL
 REFERENCES config_packages(config_package_id), account_id INTEGER NOT NULL REFERENCES accounts(account_id),
 PRIMARY KEY(override_id,config_package_id,account_id)) WITHOUT ROWID;
CREATE TABLE unrelated(data TEXT);
INSERT INTO accounts VALUES(0,''),(4,'private-account@example.test');
INSERT INTO config_packages VALUES(71,'com.google.android.gms.gmscompliance',NULL),(81,'other.namespace',NULL);
INSERT INTO experiment_states(experiment_state_id,account_id,config_package_id) VALUES(550,0,71),(551,0,71),(650,4,81);
UPDATE config_packages SET committed_experiment_state_id=550 WHERE config_package_id=71;
UPDATE config_packages SET committed_experiment_state_id=650 WHERE config_package_id=81;
INSERT INTO flag_overrides(config_package_id,account_id,name,value,type) VALUES(81,4,'1',1,2),(71,0,'different-flag',1,2);
INSERT INTO unrelated VALUES('keep-me');
"""

DE_SCHEMA = """
PRAGMA foreign_keys=ON;
CREATE TABLE config_packages(config_package_id INTEGER PRIMARY KEY, name TEXT UNIQUE NOT NULL);
CREATE TABLE flag_overrides(config_package_id INTEGER NOT NULL REFERENCES config_packages(config_package_id),
 name TEXT NOT NULL, value NOT NULL, type INTEGER NOT NULL, PRIMARY KEY(config_package_id,name)) WITHOUT ROWID;
INSERT INTO config_packages VALUES(29,'com.google.android.gms.gmscompliance'),(30,'other.namespace');
INSERT INTO flag_overrides VALUES(30,'1',1,2);
"""


class ScopedOverrideTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.path = Path(self.directory.name) / "phenotype.db"
        self.db = sqlite3.connect(self.path)
        self.db.executescript(CE_SCHEMA)

    def tearDown(self):
        self.db.close()
        self.directory.cleanup()

    def existing(self, value=1, linked=True):
        cursor = self.db.execute("INSERT INTO flag_overrides(config_package_id,account_id,name,value,type) VALUES(71,0,'1',?,2)", (value,))
        identity = cursor.lastrowid
        if linked:
            self.db.execute("INSERT INTO experiment_states_to_overrides VALUES(550,?)", (identity,))
        self.db.commit()
        return identity

    def test_plan_is_read_only_and_does_not_disclose_other_account_or_token(self):
        before = list(self.db.iterdump())
        baseline = compat.plan(self.db)
        self.assertEqual(baseline["config"]["config_package_id"], 71)
        self.assertEqual(baseline["state"]["experiment_state_id"], 550)
        self.assertNotIn("private-account", json.dumps(baseline))
        self.assertNotIn("private-token", json.dumps(baseline))
        self.assertEqual(list(self.db.iterdump()), before)

    def test_apply_creates_only_scoped_override_and_links_existing_state(self):
        baseline = compat.plan(self.db)
        receipt = compat.apply(self.db, baseline)
        row = receipt["after"]["override"]
        self.assertEqual((row["config_package_id"], row["account_id"], row["name"], row["type"], row["value"]), (71, 0, "1", 2, 0))
        self.assertEqual(receipt["after"]["links"], [{"experiment_state_id": 550, "override_id": row["override_id"]}])
        self.assertEqual(self.db.execute("SELECT count(*) FROM experiment_states").fetchone()[0], 3)
        self.assertEqual(self.db.execute("PRAGMA foreign_key_check").fetchall(), [])

    def test_retry_with_receipt_is_idempotent_and_preserves_ownership(self):
        baseline = compat.plan(self.db)
        first = compat.apply(self.db, baseline)
        serializable = json.loads(json.dumps(first))
        self.assertEqual(compat.apply(self.db, baseline, receipt=serializable), first)
        self.assertEqual(self.db.execute("SELECT count(*) FROM flag_overrides WHERE config_package_id=71 AND name='1'").fetchone()[0], 1)
        self.assertTrue(compat.rollback(self.db, serializable)["rolled_back"])
        self.assertTrue(compat.rollback(self.db, serializable)["already_restored"])

    def test_existing_desired_value_is_not_owned(self):
        identity = self.existing(value=0)
        receipt = compat.apply(self.db, compat.plan(self.db))
        self.assertFalse(receipt["changed"])
        self.assertFalse(compat.rollback(self.db, receipt)["rolled_back"])
        self.assertEqual(self.db.execute("SELECT value FROM flag_overrides WHERE override_id=?", (identity,)).fetchone()[0], 0)

    def test_existing_value_and_links_restored_without_replacing_row(self):
        identity = self.existing(linked=False)
        before = compat.plan(self.db)
        receipt = compat.apply(self.db, before)
        self.assertFalse(receipt["created_override"])
        self.assertTrue(receipt["added_link"])
        self.assertEqual(receipt["after"]["override"]["override_id"], identity)
        compat.rollback(self.db, receipt)
        self.assertEqual(compat.plan(self.db), before)

    def test_unrelated_writes_survive_apply_and_rollback(self):
        baseline = compat.plan(self.db)
        self.db.execute("INSERT INTO unrelated VALUES('new-before-apply')")
        self.db.commit()
        receipt = compat.apply(self.db, baseline)
        self.db.execute("UPDATE flag_overrides SET value=0 WHERE config_package_id=81")
        self.db.execute("INSERT INTO unrelated VALUES('new-before-rollback')")
        self.db.commit()
        compat.rollback(self.db, receipt)
        self.assertEqual(self.db.execute("SELECT value FROM flag_overrides WHERE config_package_id=81").fetchone()[0], 0)
        self.assertEqual(self.db.execute("SELECT data FROM unrelated").fetchall(), [("keep-me",), ("new-before-apply",), ("new-before-rollback",)])

    def test_competing_scoped_insert_is_not_claimed_as_ours(self):
        baseline = compat.plan(self.db)
        self.existing(value=0)
        with self.assertRaisesRegex(compat.CompatibilityError, "changed externally"):
            compat.apply(self.db, baseline)
        self.assertFalse(self.db.in_transaction)

    def test_committed_state_change_rejects_stale_plan(self):
        baseline = compat.plan(self.db)
        self.db.execute("UPDATE config_packages SET committed_experiment_state_id=551 WHERE config_package_id=71")
        self.db.commit()
        with self.assertRaisesRegex(compat.CompatibilityError, "changed externally"):
            compat.apply(self.db, baseline)
        self.assertIsNone(compat.plan(self.db)["override"])

    def test_rollback_refuses_competing_value_change(self):
        receipt = compat.apply(self.db, compat.plan(self.db))
        identity = receipt["after"]["override"]["override_id"]
        self.db.execute("UPDATE flag_overrides SET value=1 WHERE override_id=?", (identity,))
        self.db.commit()
        with self.assertRaisesRegex(compat.CompatibilityError, "changed"):
            compat.rollback(self.db, receipt)
        self.assertEqual(self.db.execute("SELECT value FROM flag_overrides WHERE override_id=?", (identity,)).fetchone()[0], 1)

    def test_rollback_refuses_new_external_reference(self):
        receipt = compat.apply(self.db, compat.plan(self.db))
        identity = receipt["after"]["override"]["override_id"]
        self.db.execute("INSERT INTO experiment_states_to_overrides VALUES(551,?)", (identity,))
        self.db.commit()
        with self.assertRaisesRegex(compat.CompatibilityError, "changed"):
            compat.rollback(self.db, receipt)
        self.assertEqual(self.db.execute("SELECT count(*) FROM experiment_states_to_overrides WHERE override_id=?", (identity,)).fetchone()[0], 2)

    def test_missing_committed_state_is_rejected_without_creation(self):
        self.db.execute("UPDATE config_packages SET committed_experiment_state_id=NULL WHERE config_package_id=71")
        self.db.commit()
        with self.assertRaisesRegex(compat.CompatibilityError, "no committed state"):
            compat.plan(self.db)
        self.assertEqual(self.db.execute("SELECT count(*) FROM experiment_states").fetchone()[0], 3)

    def test_other_account_and_ambiguous_history_are_rejected(self):
        identity = self.existing()
        self.db.execute("UPDATE flag_overrides SET active=NULL WHERE override_id=?", (identity,))
        self.db.execute("INSERT INTO flag_overrides(config_package_id,account_id,name,value,type) VALUES(71,0,'1',1,2)")
        self.db.commit()
        with self.assertRaisesRegex(compat.CompatibilityError, "Ambiguous"):
            compat.plan(self.db)

    def test_account_specific_candidate_is_rejected(self):
        self.db.execute("INSERT INTO flag_overrides(config_package_id,account_id,name,value,type) VALUES(71,4,'1',1,2)")
        self.db.commit()
        with self.assertRaisesRegex(compat.CompatibilityError, "account"):
            compat.plan(self.db)

    def test_journal_callback_runs_before_commit_with_exact_new_id(self):
        observer = sqlite3.connect(self.path)
        saved = []
        def journal(receipt):
            self.assertTrue(self.db.in_transaction)
            self.assertGreater(receipt["after"]["override"]["override_id"], 0)
            self.assertEqual(observer.execute("SELECT count(*) FROM flag_overrides WHERE config_package_id=71 AND name='1'").fetchone()[0], 0)
            saved.append(json.loads(json.dumps(receipt)))
        try:
            receipt = compat.apply(self.db, compat.plan(self.db), before_commit=journal)
            self.assertEqual(saved, [receipt])
            self.assertEqual(observer.execute("SELECT count(*) FROM flag_overrides WHERE config_package_id=71 AND name='1'").fetchone()[0], 1)
        finally:
            observer.close()

    def test_failed_journal_rolls_back_and_precommit_receipt_is_recoverable(self):
        baseline = compat.plan(self.db)
        saved = []
        def fail(receipt):
            saved.append(receipt)
            raise OSError("journal fsync failed")
        with self.assertRaises(OSError):
            compat.apply(self.db, baseline, before_commit=fail)
        self.assertEqual(compat.plan(self.db), baseline)
        self.assertTrue(compat.rollback(self.db, saved[0])["already_restored"])

    def test_standalone_source_exec_and_custom_row_factory(self):
        namespace = {}
        exec(compile(SOURCE.read_text(), str(SOURCE), "exec"), namespace)
        self.db.row_factory = sqlite3.Row
        self.assertEqual(namespace["plan"](self.db)["config"]["config_package_id"], 71)


class DeviceEncryptedTests(unittest.TestCase):
    def test_de_single_row_apply_rollback_preserves_other_namespace(self):
        with sqlite3.connect(":memory:") as db:
            db.executescript(DE_SCHEMA)
            before = compat.plan(db, "de")
            receipt = compat.apply(db, before)
            self.assertEqual(receipt["after"]["override"], {"config_package_id": 29, "name": "1", "value": 0, "type": 2})
            compat.rollback(db, receipt)
            self.assertEqual(compat.plan(db, "de"), before)
            self.assertEqual(db.execute("SELECT * FROM flag_overrides").fetchall(), [(30, "1", 1, 2)])

    def test_de_existing_value_restored(self):
        with sqlite3.connect(":memory:") as db:
            db.executescript(DE_SCHEMA)
            db.execute("INSERT INTO flag_overrides VALUES(29,'1',1,2)")
            db.commit()
            receipt = compat.apply(db, compat.plan(db, "de"))
            compat.rollback(db, receipt)
            self.assertEqual(db.execute("SELECT value FROM flag_overrides WHERE config_package_id=29").fetchone()[0], 1)


if __name__ == "__main__":
    unittest.main()
