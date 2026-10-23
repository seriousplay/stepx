"""One reversible GmsCompliance flag override; no account or token access.

This file is deliberately standalone so its source can run in a bounded phone
worker. The caller owns transport, private receipt storage, backups, process
restart and snapshot invalidation. A successful database transaction does not
establish Google certification or prove that a running process read the flag.
"""

import contextlib
import hashlib
import json
import sqlite3


NAMESPACE = "com.google.android.gms.gmscompliance"
FLAG_NAME = "1"
FORMAT = 1
CE_COLUMNS = (
    "override_id", "config_package_id", "config_package_name", "account_id",
    "active", "name", "value", "type", "source",
)
DE_COLUMNS = ("config_package_id", "name", "value", "type")


class CompatibilityError(RuntimeError):
    pass


def _rows(connection, sql, parameters=()):
    cursor = connection.execute(sql, parameters)
    names = [item[0] for item in cursor.description]
    return [dict(zip(names, row)) for row in cursor.fetchall()]


def _require(condition, message):
    if not condition:
        raise CompatibilityError(message)


@contextlib.contextmanager
def _transaction(connection, write=False):
    _require(not connection.in_transaction, "A caller transaction is already active")
    if write:
        connection.execute("PRAGMA foreign_keys = ON")
        _require(connection.execute("PRAGMA foreign_keys").fetchone()[0] == 1,
                 "Foreign keys must remain enabled")
    connection.execute("BEGIN IMMEDIATE" if write else "BEGIN")
    try:
        yield
        connection.commit()
    except BaseException:
        connection.rollback()
        raise


def _schema(connection, kind):
    required = {
        "config_packages": {"config_package_id", "name"},
        "flag_overrides": set(CE_COLUMNS if kind == "ce" else DE_COLUMNS),
    }
    if kind == "ce":
        required["config_packages"].add("committed_experiment_state_id")
        required["accounts"] = {"account_id", "name"}
        required["experiment_states"] = {
            "experiment_state_id", "account_id", "config_package_id",
        }
        required["experiment_states_to_overrides"] = {
            "experiment_state_id", "override_id",
        }
        required["flag_overrides_to_commit"] = {
            "override_id", "config_package_id", "account_id",
        }
    definitions = []
    for table, columns in required.items():
        actual = {row[1] for row in connection.execute('PRAGMA table_info("' + table + '")')}
        _require(columns <= actual, "Unsupported " + kind + " schema: " + table)
        definitions.extend(tuple(row) for row in connection.execute(
            "SELECT type, name, sql FROM sqlite_master WHERE tbl_name = ? ORDER BY type, name",
            (table,),
        ).fetchall())
    version = connection.execute("PRAGMA user_version").fetchone()[0]
    digest = hashlib.sha256(json.dumps(definitions, separators=(",", ":")).encode()).hexdigest()
    return {"user_version": version, "sha256": digest}


def _check_flag(row, kind):
    if row is None:
        return
    _require(row["name"] == FLAG_NAME and row["type"] == 2
             and type(row["value"]) is int and row["value"] in (0, 1),
             "The scoped flag is not the expected Boolean")
    if kind == "ce":
        _require(row["account_id"] == 0 and row["active"] == 1
                 and row["config_package_name"] is None,
                 "Unsupported account or override state")


def _read_plan(connection, kind):
    _require(kind in ("ce", "de"), "Database kind must be ce or de")
    schema = _schema(connection, kind)
    fields = "config_package_id, committed_experiment_state_id" if kind == "ce" else "config_package_id"
    configs = _rows(connection, "SELECT " + fields + " FROM config_packages WHERE name = ?", (NAMESPACE,))
    _require(len(configs) == 1, "The exact configuration package must exist uniquely")
    config = configs[0]
    package_id = config["config_package_id"]
    result = {"format": FORMAT, "namespace": NAMESPACE, "flag": FLAG_NAME,
              "kind": kind, "schema": schema, "config": config,
              "override": None, "links": [], "pending_links": []}
    if kind == "ce":
        # Query only the reserved anonymous identity; never return account names.
        identities = [tuple(row) for row in connection.execute("SELECT account_id FROM accounts WHERE name = ''")]
        _require(identities == [(0,)], "The anonymous account identity is unavailable")
        committed = config["committed_experiment_state_id"]
        _require(type(committed) is int, "The configuration has no committed state")
        states = _rows(connection,
                      "SELECT experiment_state_id, account_id, config_package_id "
                      "FROM experiment_states WHERE experiment_state_id = ?", (committed,))
        _require(len(states) == 1 and states[0]["account_id"] == 0
                 and states[0]["config_package_id"] == package_id,
                 "The committed state must belong to this anonymous configuration")
        result["state"] = states[0]
        candidates = _rows(connection,
            "SELECT " + ", ".join("o." + name for name in CE_COLUMNS) + " FROM flag_overrides o "
            "WHERE (o.config_package_id = ? OR o.config_package_name = ?) AND o.name = ? "
            "AND (o.active IS 1 OR EXISTS (SELECT 1 FROM experiment_states_to_overrides l "
            "WHERE l.override_id = o.override_id AND l.experiment_state_id = ?)) ORDER BY o.override_id",
            (package_id, NAMESPACE, FLAG_NAME, committed))
        # A wildcard/account override or multiple historical rows linked into the
        # committed state would make precedence ambiguous. Never guess or merge.
        _require(len(candidates) <= 1, "Ambiguous overrides for the scoped flag")
        if candidates:
            row = candidates[0]
            _check_flag(row, kind)
            _require(row["config_package_id"] == package_id, "An unresolved override already exists")
            result["override"] = row
            result["links"] = _rows(connection,
                "SELECT experiment_state_id, override_id FROM experiment_states_to_overrides "
                "WHERE override_id = ? ORDER BY experiment_state_id", (row["override_id"],))
            result["pending_links"] = _rows(connection,
                "SELECT override_id, config_package_id, account_id FROM flag_overrides_to_commit "
                "WHERE override_id = ? ORDER BY config_package_id, account_id", (row["override_id"],))
    else:
        candidates = _rows(connection,
            "SELECT " + ", ".join(DE_COLUMNS) + " FROM flag_overrides WHERE config_package_id = ? AND name = ?",
            (package_id, FLAG_NAME))
        _require(len(candidates) <= 1, "Ambiguous overrides for the scoped flag")
        if candidates:
            _check_flag(candidates[0], kind)
            result["override"] = candidates[0]
    return result


def plan(connection, kind="ce"):
    """Return a consistent, read-only baseline containing no tokens or identities."""
    with _transaction(connection):
        return _read_plan(connection, kind)


def _validate_plan(baseline):
    _require(isinstance(baseline, dict) and baseline.get("format") == FORMAT
             and baseline.get("namespace") == NAMESPACE and baseline.get("flag") == FLAG_NAME
             and baseline.get("kind") in ("ce", "de"), "Invalid scoped baseline")
    _check_flag(baseline.get("override"), baseline["kind"])


def _validate_receipt(receipt, baseline=None):
    _require(isinstance(receipt, dict) and receipt.get("format") == FORMAT
             and receipt.get("operation") == "gmscompliance_boolean_override", "Invalid receipt")
    _validate_plan(receipt["before"])
    _validate_plan(receipt["after"])
    before, after = receipt["before"], receipt["after"]
    _require(before["kind"] == after["kind"] and before["config"] == after["config"]
             and before["schema"] == after["schema"], "Receipt scope changed")
    _require(after["override"] is not None and after["override"]["value"] == 0,
             "Receipt does not contain the fixed Boolean override")
    if baseline is not None:
        _require(before == baseline, "Receipt belongs to a different baseline")


def apply(connection, baseline, receipt=None, before_commit=None):
    """Apply one flag using CAS. Pass its receipt for a safe idempotent retry.

    Retrying an old baseline without its receipt intentionally fails closed: an
    equal value written by another actor does not confer ownership on this tool.
    before_commit receives the exact receipt while the transaction is still
    active, permitting a caller to fsync its recovery journal before SQL commit.
    A callback failure rolls back the SQL transaction.
    """
    _validate_plan(baseline)
    if receipt is not None:
        _validate_receipt(receipt, baseline)
    kind = baseline["kind"]
    with _transaction(connection, write=True):
        current = _read_plan(connection, kind)
        if receipt is not None:
            _require(current == receipt["after"], "The applied state changed externally")
            return receipt
        _require(current == baseline, "The planned state changed externally")
        row = current["override"]
        package_id = current["config"]["config_package_id"]
        created = row is None
        changed_value = row is not None and row["value"] != 0
        added_link = False
        if kind == "ce":
            if created:
                cursor = connection.execute(
                    "INSERT INTO flag_overrides (config_package_id, config_package_name, account_id, "
                    "active, name, value, type, source) VALUES (?, NULL, 0, 1, ?, 0, 2, 0)",
                    (package_id, FLAG_NAME))
                override_id = cursor.lastrowid
            else:
                override_id = row["override_id"]
                if changed_value:
                    cursor = connection.execute(
                        "UPDATE flag_overrides SET value = 0 WHERE override_id = ? AND value = ? AND type = 2",
                        (override_id, row["value"]))
                    _require(cursor.rowcount == 1, "Scoped update did not affect exactly one row")
            committed = current["config"]["committed_experiment_state_id"]
            if not any(link["experiment_state_id"] == committed for link in current["links"]):
                connection.execute("INSERT INTO experiment_states_to_overrides "
                                   "(experiment_state_id, override_id) VALUES (?, ?)", (committed, override_id))
                added_link = True
        elif created:
            connection.execute("INSERT INTO flag_overrides (config_package_id, name, value, type) VALUES (?, ?, 0, 2)",
                               (package_id, FLAG_NAME))
        elif changed_value:
            cursor = connection.execute("UPDATE flag_overrides SET value = 0 "
                "WHERE config_package_id = ? AND name = ? AND value = ? AND type = 2",
                (package_id, FLAG_NAME, row["value"]))
            _require(cursor.rowcount == 1, "Scoped update did not affect exactly one row")
        after = _read_plan(connection, kind)
        _require(after["override"] is not None and after["override"]["value"] == 0,
                 "Override readback failed")
        result = {"format": FORMAT, "operation": "gmscompliance_boolean_override",
                  "before": baseline, "after": after, "created_override": created,
                  "changed_value": changed_value, "added_link": added_link,
                  "changed": created or changed_value or added_link}
        if before_commit is not None:
            before_commit(result)
        return result


def rollback(connection, receipt):
    """Undo only changes owned by a receipt, refusing any competing scoped edit."""
    _validate_receipt(receipt)
    before, after = receipt["before"], receipt["after"]
    kind = before["kind"]
    with _transaction(connection, write=True):
        current = _read_plan(connection, kind)
        if current == before:
            return {"rolled_back": False, "already_restored": True}
        _require(current == after, "The scoped state changed; rollback would overwrite another edit")
        row = after["override"]
        if kind == "ce":
            override_id = row["override_id"]
            if receipt["added_link"]:
                cursor = connection.execute("DELETE FROM experiment_states_to_overrides "
                    "WHERE experiment_state_id = ? AND override_id = ?",
                    (after["config"]["committed_experiment_state_id"], override_id))
                _require(cursor.rowcount == 1, "Owned link was not removed exactly once")
            if receipt["created_override"]:
                cursor = connection.execute("DELETE FROM flag_overrides WHERE override_id = ?", (override_id,))
                _require(cursor.rowcount == 1, "Owned override was not removed exactly once")
            elif receipt["changed_value"]:
                connection.execute("UPDATE flag_overrides SET value = ? WHERE override_id = ?",
                                   (before["override"]["value"], override_id))
        elif receipt["created_override"]:
            cursor = connection.execute("DELETE FROM flag_overrides WHERE config_package_id = ? AND name = ?",
                                        (row["config_package_id"], FLAG_NAME))
            _require(cursor.rowcount == 1, "Owned override was not removed exactly once")
        elif receipt["changed_value"]:
            connection.execute("UPDATE flag_overrides SET value = ? WHERE config_package_id = ? AND name = ?",
                               (before["override"]["value"], row["config_package_id"], FLAG_NAME))
        _require(_read_plan(connection, kind) == before, "Rollback readback did not match the scoped baseline")
        return {"rolled_back": True, "already_restored": False}
