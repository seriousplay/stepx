"""One reversible Play activity-list override in the committed account scope.

Standalone standard-library source for the bounded worker. No account names,
tokens, device-certification records or unrelated flags are read or changed.
The caller owns private journaling, process stopping and cache refresh.
"""

import base64
import contextlib
import hashlib
import json
import sqlite3


NAMESPACE = "com.google.android.finsky.regular"
FLAG_NAME = "GmscoreCompliance__activities_to_block_on_uncertified_devices"
FORMAT = 1
OPERATION = "play_activity_list_blob_override"
COLUMNS = ("override_id", "config_package_id", "config_package_name", "account_id",
           "active", "name", "value", "type", "source")


class CompatibilityError(RuntimeError):
    pass


def _require(condition, message):
    if not condition:
        raise CompatibilityError(message)


def _rows(connection, sql, parameters=()):
    cursor = connection.execute(sql, parameters)
    columns = [description[0] for description in cursor.description]
    return [dict(zip(columns, row)) for row in cursor.fetchall()]


@contextlib.contextmanager
def _transaction(connection, write=False):
    _require(not connection.in_transaction, "A caller transaction is already active")
    if write:
        connection.execute("PRAGMA foreign_keys=ON")
        _require(connection.execute("PRAGMA foreign_keys").fetchone()[0] == 1,
                 "Foreign keys must remain enabled")
    connection.execute("BEGIN IMMEDIATE" if write else "BEGIN")
    try:
        yield
        connection.commit()
    except BaseException:
        connection.rollback()
        raise


def _schema(connection):
    required = {
        "accounts": {"account_id"},
        "config_packages": {"config_package_id", "name", "committed_experiment_state_id"},
        "experiment_states": {"experiment_state_id", "account_id", "config_package_id"},
        "flag_overrides": set(COLUMNS),
        "experiment_states_to_overrides": {"experiment_state_id", "override_id"},
        "flag_overrides_to_commit": {"override_id", "config_package_id", "account_id"},
    }
    definitions = []
    for table, expected in required.items():
        columns = {row[1] for row in connection.execute('PRAGMA table_info("' + table + '")')}
        _require(expected <= columns, "Unsupported committed-account schema")
        definitions.extend(tuple(row) for row in connection.execute(
            "SELECT type,name,sql FROM sqlite_master WHERE tbl_name=? ORDER BY type,name", (table,)))
    return {"user_version": connection.execute("PRAGMA user_version").fetchone()[0],
            "sha256": hashlib.sha256(json.dumps(definitions, separators=(",", ":")).encode()).hexdigest()}


def _blob(row):
    _require(row["type"] == 5 and isinstance(row.get("value_b64"), str),
             "Scoped override must be a BLOB flag")
    try:
        value = base64.b64decode(row["value_b64"], validate=True)
    except (ValueError, TypeError) as error:
        raise CompatibilityError("Invalid original BLOB") from error
    _require(base64.b64encode(value).decode("ascii") == row["value_b64"], "Noncanonical BLOB encoding")
    return value


def _read_plan(connection):
    schema = _schema(connection)
    configs = _rows(connection, "SELECT config_package_id,committed_experiment_state_id "
                    "FROM config_packages WHERE name=?", (NAMESPACE,))
    _require(len(configs) == 1, "The exact configuration must exist uniquely")
    config = configs[0]
    committed = config["committed_experiment_state_id"]
    package_id = config["config_package_id"]
    _require(type(committed) is int, "Configuration has no committed state")
    states = _rows(connection, "SELECT experiment_state_id,account_id,config_package_id "
                   "FROM experiment_states WHERE experiment_state_id=?", (committed,))
    _require(len(states) == 1 and states[0]["config_package_id"] == package_id,
             "Committed state belongs to a different configuration")
    state = states[0]
    account = state["account_id"]
    _require(type(account) is int and account >= 0, "Invalid committed account identifier")
    _require(connection.execute("SELECT count(*) FROM accounts WHERE account_id=?", (account,)).fetchone()[0] == 1,
             "Committed account identity is missing")
    result = {"format": FORMAT, "namespace": NAMESPACE, "flag": FLAG_NAME, "kind": "ce",
              "schema": schema, "config": config, "state": state,
              "override": None, "links": [], "pending_links": []}
    # Inspect account 0 as a possible competing fallback, but never create or
    # alter its row when a different account owns the committed state. Also
    # detect any mismatched/historical row already linked to this state.
    candidates = _rows(connection,
        "SELECT " + ",".join("o." + column for column in COLUMNS) + ",typeof(o.value) AS value_storage "
        "FROM flag_overrides o WHERE o.name=? AND ("
        "((o.config_package_id=? OR o.config_package_name=?) AND o.account_id IN (?,0) AND o.active IS 1) "
        "OR EXISTS (SELECT 1 FROM experiment_states_to_overrides l "
        "WHERE l.override_id=o.override_id AND l.experiment_state_id=?)) ORDER BY o.override_id",
        (FLAG_NAME, package_id, NAMESPACE, account, committed))
    _require(len(candidates) <= 1, "Ambiguous committed-account override precedence")
    if candidates:
        row = candidates[0]
        _require(row["config_package_id"] == package_id and row["config_package_name"] is None
                 and row["account_id"] == account and row["active"] == 1 and row["type"] == 5
                 and row.pop("value_storage") == "blob" and isinstance(row["value"], bytes),
                 "Existing override is not an active BLOB in the exact committed account")
        row["value_b64"] = base64.b64encode(row.pop("value")).decode("ascii")
        result["override"] = row
        identity = row["override_id"]
        result["links"] = _rows(connection,
            "SELECT l.experiment_state_id,l.override_id,s.account_id,s.config_package_id "
            "FROM experiment_states_to_overrides l LEFT JOIN experiment_states s "
            "ON s.experiment_state_id=l.experiment_state_id WHERE l.override_id=? ORDER BY l.experiment_state_id",
            (identity,))
        result["pending_links"] = _rows(connection,
            "SELECT override_id,config_package_id,account_id FROM flag_overrides_to_commit "
            "WHERE override_id=? ORDER BY config_package_id,account_id", (identity,))
        _require(all(link["account_id"] == account and link["config_package_id"] == package_id
                     for link in result["links"] + result["pending_links"]),
                 "Scoped override is referenced by another configuration or account")
    return result


def plan(connection):
    """Read a consistent baseline without account names, server tokens or writes."""
    with _transaction(connection):
        return _read_plan(connection)


def _validate_plan(baseline):
    _require(isinstance(baseline, dict) and baseline.get("format") == FORMAT
             and baseline.get("namespace") == NAMESPACE and baseline.get("flag") == FLAG_NAME
             and baseline.get("kind") == "ce", "Invalid scoped baseline")
    row = baseline.get("override")
    if row is not None:
        _require(row["name"] == FLAG_NAME and row["account_id"] == baseline["state"]["account_id"],
                 "Baseline override scope changed")
        _blob(row)


def is_active(baseline):
    """Whether the fixed empty BLOB is linked to the existing committed state."""
    _validate_plan(baseline)
    row = baseline["override"]
    return (row is not None and row["active"] == 1 and row["value_b64"] == ""
            and any(link["experiment_state_id"] == baseline["config"]["committed_experiment_state_id"]
                    and link["override_id"] == row["override_id"] for link in baseline["links"]))


def _validate_receipt(receipt, baseline=None):
    _require(isinstance(receipt, dict) and receipt.get("format") == FORMAT
             and receipt.get("operation") == OPERATION, "Invalid scoped receipt")
    before, after = receipt["before"], receipt["after"]
    _validate_plan(before)
    _validate_plan(after)
    _require(all(before[key] == after[key] for key in ("schema", "config", "state"))
             and is_active(after), "Receipt changed account/state or does not contain the fixed override")
    if baseline is not None:
        _require(before == baseline, "Receipt belongs to another baseline")


def apply(connection, baseline, receipt=None, before_commit=None):
    """CAS one BLOB and its current-state link; journal callback runs precommit."""
    _validate_plan(baseline)
    if receipt is not None:
        _validate_receipt(receipt, baseline)
    with _transaction(connection, write=True):
        current = _read_plan(connection)
        if receipt is not None:
            _require(current == receipt["after"], "Applied state changed externally")
            return receipt
        _require(current == baseline, "Planned state changed externally")
        row = current["override"]
        created = row is None
        changed_value = row is not None and row["value_b64"] != ""
        if created:
            cursor = connection.execute("INSERT INTO flag_overrides "
                "(config_package_id,config_package_name,account_id,active,name,value,type,source) "
                "VALUES (?,NULL,?,1,?,?,5,0)",
                (current["config"]["config_package_id"], current["state"]["account_id"], FLAG_NAME, b""))
            identity = cursor.lastrowid
        else:
            identity = row["override_id"]
            if changed_value:
                cursor = connection.execute("UPDATE flag_overrides SET value=? WHERE override_id=? "
                    "AND type=5 AND typeof(value)='blob' AND value=?", (b"", identity, _blob(row)))
                _require(cursor.rowcount == 1, "BLOB update did not affect exactly one scoped row")
        committed = current["config"]["committed_experiment_state_id"]
        added_link = not any(link["experiment_state_id"] == committed for link in current["links"])
        if added_link:
            connection.execute("INSERT INTO experiment_states_to_overrides (experiment_state_id,override_id) "
                               "VALUES (?,?)", (committed, identity))
        after = _read_plan(connection)
        _require(is_active(after), "Committed BLOB override readback failed")
        result = {"format": FORMAT, "operation": OPERATION, "before": baseline, "after": after,
                  "created_override": created, "changed_value": changed_value, "added_link": added_link,
                  "changed": created or changed_value or added_link}
        if before_commit is not None:
            before_commit(result)
        return result


def rollback(connection, receipt):
    """Restore only our exact row/link, refusing changed state or references."""
    _validate_receipt(receipt)
    before, after = receipt["before"], receipt["after"]
    with _transaction(connection, write=True):
        current = _read_plan(connection)
        if current == before:
            return {"rolled_back": False, "already_restored": True}
        _require(current == after, "Scoped state changed; rollback would overwrite another edit")
        identity = after["override"]["override_id"]
        if receipt["added_link"]:
            cursor = connection.execute("DELETE FROM experiment_states_to_overrides "
                "WHERE experiment_state_id=? AND override_id=?",
                (after["config"]["committed_experiment_state_id"], identity))
            _require(cursor.rowcount == 1, "Owned link was not removed exactly once")
        if receipt["created_override"]:
            cursor = connection.execute("DELETE FROM flag_overrides WHERE override_id=?", (identity,))
            _require(cursor.rowcount == 1, "Owned override was not removed exactly once")
        elif receipt["changed_value"]:
            cursor = connection.execute("UPDATE flag_overrides SET value=? WHERE override_id=? "
                "AND type=5 AND typeof(value)='blob' AND value=?", (_blob(before["override"]), identity, b""))
            _require(cursor.rowcount == 1, "Original BLOB was not restored exactly once")
        _require(_read_plan(connection) == before, "Rollback readback differs from scoped baseline")
        return {"rolled_back": True, "already_restored": False}
