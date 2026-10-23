"""Scoped, journaled cache replacement. Stop the owning app before mutation.

Standalone standard-library source for a bounded worker. This changes no APK,
account or certification data. Receipt contents are private and must not be logged.
"""

import base64
import contextlib
import errno
import hashlib
import json
import os
import re
import secrets
import stat
import zlib


ROOT_BASE = "/data/user/0"
PACKAGES = {"gms": "com.google.android.gms", "play": "com.android.vending"}
MAX_BYTES = 64 * 1024 * 1024
MAX_FILES = 32
FORMAT = 1


class CacheCompatibilityError(RuntimeError):
    pass


def _require(condition, message):
    if not condition:
        raise CacheCompatibilityError(message)


def _path(scope, path, root_base, journal=False):
    _require(scope in PACKAGES, "Unknown cache scope")
    root_base = os.fspath(root_base)
    path = os.fspath(path)
    _require(os.path.isabs(root_base) and os.path.normpath(root_base) == root_base,
             "Root must be an absolute normalized path")
    _require(os.path.isabs(path) and os.path.normpath(path) == path and "\x00" not in path,
             "Path must be absolute and normalized")
    package_root = root_base + "/" + PACKAGES[scope]
    _require(path.startswith(package_root + "/"), "Path is outside the scoped package")
    relative = path[len(package_root) + 1:]
    parts = relative.split("/")
    if journal:
        allowed = (len(parts) == 3 and parts[:2] == ["files", "stepx-google-compat"]
                   and re.fullmatch(r"[A-Za-z0-9_-]+\.json", parts[2]))
    elif scope == "gms":
        allowed = (relative == "files/phenotype/shared/com.google.android.gms.gmscompliance.pb"
                   or parts[-1] == "0dPD7nD8E2yOLxoYWJqw_g==.pb")
    else:
        allowed = (len(parts) == 2 and parts[0] == "files"
                   and re.fullmatch(r"experiment-flags-regular-[A-Za-z0-9._@+=%-]+", parts[1]))
    _require(bool(allowed), "Path is not in the fixed cache allowlist")
    return path


@contextlib.contextmanager
def _directory(path):
    """Anchor every component; never follow a symlink, even in the base path."""
    parts = [part for part in path.split("/") if part]
    descriptor = os.open("/", (os.O_PATH if parts else os.O_RDONLY) | os.O_DIRECTORY)
    try:
        for index, part in enumerate(parts):
            # App domains can search /data ancestors without permission to list
            # them. The final directory needs a normal fd for fsync and xattrs.
            access = os.O_RDONLY if index == len(parts) - 1 else os.O_PATH
            following = os.open(part, access | os.O_DIRECTORY | os.O_NOFOLLOW,
                                dir_fd=descriptor)
            os.close(descriptor)
            descriptor = following
        yield descriptor
    finally:
        os.close(descriptor)


def _label(descriptor, root_base):
    try:
        value = os.getxattr(descriptor, "security.selinux")
    except OSError as error:
        if root_base != ROOT_BASE and error.errno in (errno.ENODATA, errno.ENOTSUP):
            return None
        raise CacheCompatibilityError("Cannot read required SELinux label") from error
    _require(bool(value), "Empty SELinux label")
    return base64.b64encode(value).decode("ascii")


def _metadata(descriptor, root_base):
    info = os.fstat(descriptor)
    return {"uid": info.st_uid, "gid": info.st_gid, "mode": stat.S_IMODE(info.st_mode),
            "selinux_b64": _label(descriptor, root_base)}


def _read_at(parent, name, root_base, limit=MAX_BYTES):
    descriptor = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=parent)
    try:
        info = os.fstat(descriptor)
        _require(stat.S_ISREG(info.st_mode) and info.st_nlink == 1,
                 "Cache must be a regular file with one link")
        _require(info.st_size <= limit, "Cache exceeds size limit")
        metadata = _metadata(descriptor, root_base)
        chunks, length = [], 0
        while True:
            chunk = os.read(descriptor, min(65536, limit + 1 - length))
            if not chunk:
                break
            chunks.append(chunk)
            length += len(chunk)
            _require(length <= limit, "Cache exceeds size limit")
        after = os.fstat(descriptor)
        _require((info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, info.st_ctime_ns)
                 == (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns),
                 "Cache changed during read")
        _require(metadata == _metadata(descriptor, root_base), "Cache metadata changed during read")
        data = b"".join(chunks)
        metadata["sha256"] = hashlib.sha256(data).hexdigest()
        return data, metadata
    finally:
        os.close(descriptor)


def _read(path, root_base, limit=MAX_BYTES):
    with _directory(os.path.dirname(path)) as parent:
        return _read_at(parent, os.path.basename(path), root_base, limit)


def _compressed(data):
    return base64.b64encode(zlib.compress(data)).decode("ascii")


def _original(entry):
    try:
        compressed = base64.b64decode(entry["data_zlib_b64"], validate=True)
        decoder = zlib.decompressobj()
        data = decoder.decompress(compressed, MAX_BYTES + 1)
        _require(len(data) <= MAX_BYTES and decoder.eof and not decoder.unused_data,
                 "Invalid or oversized original data")
    except (ValueError, KeyError, zlib.error) as error:
        raise CacheCompatibilityError("Invalid original data") from error
    _require(hashlib.sha256(data).hexdigest() == entry["sha256"], "Original checksum mismatch")
    return data


def _expected(entry, digest=None):
    return {key: (digest if key == "sha256" and digest is not None else entry[key])
            for key in ("uid", "gid", "mode", "selinux_b64", "sha256")}


def prepare(scope, paths, root_base=ROOT_BASE):
    root_base = os.fspath(root_base)
    paths = list(paths)
    _require(0 < len(paths) <= MAX_FILES and len(set(paths)) == len(paths),
             "Require unique cache paths within the file limit")
    entries = []
    for path in paths:
        path = _path(scope, path, root_base)
        data, metadata = _read(path, root_base)
        entries.append(dict(metadata, path=path, data_zlib_b64=_compressed(data)))
    return {"format": FORMAT, "scope": scope, "entries": entries}


def _validate(scope, baseline, root_base):
    _require(baseline.get("format") == FORMAT and baseline.get("scope") == scope,
             "Baseline scope or format mismatch")
    entries = baseline.get("entries", [])
    _require(0 < len(entries) <= MAX_FILES, "Invalid baseline file count")
    paths = [_path(scope, entry["path"], root_base) for entry in entries]
    _require(len(paths) == len(set(paths)), "Duplicate baseline paths")
    originals = [_original(entry) for entry in entries]
    return entries, originals


def _set_metadata(descriptor, metadata, root_base):
    current = os.fstat(descriptor)
    if (current.st_uid, current.st_gid) != (metadata["uid"], metadata["gid"]):
        os.fchown(descriptor, metadata["uid"], metadata["gid"])
    os.fchmod(descriptor, metadata["mode"])
    label = metadata["selinux_b64"]
    if label is not None:
        encoded = base64.b64decode(label, validate=True)
        try:
            actual = os.getxattr(descriptor, "security.selinux")
        except OSError:
            actual = None
        if actual != encoded:
            os.setxattr(descriptor, "security.selinux", encoded)
    _require(_metadata(descriptor, root_base) == {key: metadata[key] for key in
             ("uid", "gid", "mode", "selinux_b64")}, "Cannot preserve file metadata")


def _atomic_replace(path, data, expected, root_base):
    with _directory(os.path.dirname(path)) as parent:
        name = os.path.basename(path)
        temporary = ".stepx-compat-" + secrets.token_hex(12)
        descriptor = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                             0o600, dir_fd=parent)
        try:
            with os.fdopen(descriptor, "wb", closefd=True) as stream:
                stream.write(data)
                stream.flush()
                _set_metadata(stream.fileno(), expected, root_base)
                os.fsync(stream.fileno())
            _, current = _read_at(parent, name, root_base)
            _require(current == expected, "Cache compare-and-swap conflict")
            os.replace(temporary, name, src_dir_fd=parent, dst_dir_fd=parent)
            os.fsync(parent)
        finally:
            try:
                os.unlink(temporary, dir_fd=parent)
            except FileNotFoundError:
                pass


def _write_journal(scope, path, receipt, root_base):
    _path(scope, path, root_base, journal=True)
    files = root_base + "/" + PACKAGES[scope] + "/files"
    with _directory(files) as parent:
        metadata = _metadata(parent, root_base)
        metadata["mode"] = 0o700
        created = False
        try:
            os.mkdir("stepx-google-compat", 0o700, dir_fd=parent)
            created = True
        except FileExistsError:
            pass
        directory = os.open("stepx-google-compat", os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
                            dir_fd=parent)
        try:
            # fsuid may differ from euid in the bounded worker. Only our newly
            # created directory is normalized; an existing directory is checked.
            if created:
                _set_metadata(directory, metadata, root_base)
            _require(_metadata(directory, root_base) == metadata, "Journal directory is not private")
            os.fsync(directory)
        finally:
            os.close(directory)
        os.fsync(parent)
    raw = json.dumps(receipt, separators=(",", ":"), sort_keys=True).encode()
    _require(len(raw) <= MAX_BYTES * MAX_FILES * 2, "Journal exceeds size limit")
    metadata["mode"] = 0o600
    with _directory(os.path.dirname(path)) as parent:
        name = os.path.basename(path)
        # Exclusive create intentionally refuses to overwrite a recovery journal.
        descriptor = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                             0o600, dir_fd=parent)
        try:
            with os.fdopen(descriptor, "wb", closefd=True) as stream:
                stream.write(raw)
                stream.flush()
                _set_metadata(stream.fileno(), metadata, root_base)
                os.fsync(stream.fileno())
            os.fsync(parent)
        except BaseException:
            # No cache write has happened, so an incomplete journal is dispensable.
            os.unlink(name, dir_fd=parent)
            os.fsync(parent)
            raise


def apply(scope, baseline, new_bytes_list, journal_path, root_base=ROOT_BASE):
    root_base = os.fspath(root_base)
    entries, originals = _validate(scope, baseline, root_base)
    replacements = list(new_bytes_list)
    _require(len(replacements) == len(entries) and all(isinstance(data, bytes) and
             len(data) <= MAX_BYTES for data in replacements), "Invalid replacement data")
    _path(scope, journal_path, root_base, journal=True)
    for entry in entries:
        _require(_read(entry["path"], root_base)[1] == _expected(entry),
                 "Cache compare-and-swap conflict")
    receipt = {"format": FORMAT, "scope": scope, "baseline": baseline,
               "after_sha256": [hashlib.sha256(data).hexdigest() for data in replacements]}
    _write_journal(scope, journal_path, receipt, root_base)
    attempted = []
    try:
        for index, (entry, data) in enumerate(zip(entries, replacements)):
            if receipt["after_sha256"][index] == entry["sha256"]:
                continue
            attempted.append(index)  # Includes a rename followed by a failed fsync.
            _atomic_replace(entry["path"], data, _expected(entry), root_base)
    except BaseException as error:
        restored, conflicts = [], []
        for index in reversed(attempted):
            entry = entries[index]
            try:
                current = _read(entry["path"], root_base)[1]
                if current == _expected(entry):
                    continue
                _require(current == _expected(entry, receipt["after_sha256"][index]),
                         "External modification prevents failure rollback")
                _atomic_replace(entry["path"], originals[index], current, root_base)
                restored.append(index)
            except BaseException:
                conflicts.append(index)
        failure = CacheCompatibilityError("Cache apply failed; recovery journal retained")
        failure.restored_indices, failure.conflict_indices = restored, conflicts
        raise failure from error
    return {"applied": True, "journal_path": journal_path, "file_count": len(entries)}


def rollback(scope, journal_path, root_base=ROOT_BASE):
    root_base = os.fspath(root_base)
    _path(scope, journal_path, root_base, journal=True)
    raw, journal_metadata = _read(journal_path, root_base, MAX_BYTES * MAX_FILES * 2)
    _require(journal_metadata["mode"] == 0o600, "Journal is not private")
    with _directory(root_base + "/" + PACKAGES[scope] + "/files") as parent:
        owner = _metadata(parent, root_base)
    _require(all(journal_metadata[key] == owner[key] for key in ("uid", "gid", "selinux_b64")),
             "Journal ownership or label mismatch")
    receipt = json.loads(raw)
    _require(receipt.get("format") == FORMAT and receipt.get("scope") == scope,
             "Journal scope or format mismatch")
    entries, originals = _validate(scope, receipt["baseline"], root_base)
    digests = receipt.get("after_sha256", [])
    _require(len(digests) == len(entries) and all(isinstance(value, str) and
             re.fullmatch(r"[0-9a-f]{64}", value) for value in digests), "Invalid after checksums")
    restored, unchanged, conflicts = [], [], []
    for index, entry in enumerate(entries):
        try:
            current = _read(entry["path"], root_base)[1]
            if current == _expected(entry):
                unchanged.append(index)
            else:
                _require(current == _expected(entry, digests[index]),
                         "External modification prevents rollback")
                _atomic_replace(entry["path"], originals[index], current, root_base)
                restored.append(index)
        except (OSError, CacheCompatibilityError):
            conflicts.append(index)
    result = {"rolled_back": not conflicts, "restored_indices": restored,
              "unchanged_indices": unchanged, "conflict_indices": conflicts}
    if conflicts:
        error = CacheCompatibilityError("Rollback retained external modifications; journal retained")
        error.result = result
        raise error
    return result
