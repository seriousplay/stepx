"""Narrow, lossless editor for the GMS compliance flag 1 snapshot.

The fallback schema is pinned to GMS 26.41.31 (264131035): hgkv field 5
contains hgkw messages, whose name is field 1 and boolean oneof is field 3.
Only this flag can be set, and only to false.  File ownership, compare-and-swap,
backups, process restarts and rollback belong to the caller.  This module never
prints snapshot data or returns account/snapshot/server tokens.
"""

from dataclasses import dataclass
import struct
from typing import Optional
import zlib


MAX_SNAPSHOT_BYTES = 1024 * 1024
_TARGET_NAME = b"1"
_APPENDED_FLAG = b"\x2a\x05\x0a\x01\x31\x18\x00"
# hgek.field5 map entry: key "1", hgdj { bool(field2): false,
# name(field10): "1" }. hgfd.b() overlays these values onto compressed flags.
_APPENDED_PRIMARY_OVERRIDE = b"\x2a\x0a\x0a\x01\x31\x12\x05\x10\x00\x52\x01\x31"


class SnapshotError(ValueError):
    """Invalid or unsupported input; messages never include input contents."""


@dataclass(frozen=True)
class _Field:
    number: int
    wire: int
    start: int
    payload_start: int
    end: int
    integer: Optional[int] = None


def _varint(data: bytes, pos: int, limit: int):
    value = 0
    for index in range(10):
        if pos >= limit:
            raise SnapshotError("Truncated protobuf varint")
        byte = data[pos]
        pos += 1
        if index == 9 and byte > 1:
            raise SnapshotError("Protobuf varint exceeds 64 bits")
        value |= (byte & 0x7f) << (7 * index)
        if byte < 0x80:
            return value, pos
    raise SnapshotError("Unterminated protobuf varint")


def _encode_varint(value: int) -> bytes:
    out = bytearray()
    while value >= 0x80:
        out.append((value & 0x7f) | 0x80)
        value >>= 7
    out.append(value)
    return bytes(out)


def _fields(data: bytes, start: int = 0, end: Optional[int] = None):
    limit = len(data) if end is None else end
    pos = start
    result = []
    while pos < limit:
        field_start = pos
        tag, pos = _varint(data, pos, limit)
        number, wire = tag >> 3, tag & 7
        if not 1 <= number <= 0x1fffffff:
            raise SnapshotError("Invalid protobuf field number")
        integer = None
        if wire == 0:
            payload_start = pos
            integer, pos = _varint(data, pos, limit)
        elif wire in (1, 5):
            payload_start = pos
            pos += 8 if wire == 1 else 4
        elif wire == 2:
            length, pos = _varint(data, pos, limit)
            payload_start = pos
            pos += length
        else:
            # Known snapshots do not contain groups. Reject unfamiliar wire
            # formats instead of risking a partial or lossy rewrite.
            raise SnapshotError("Unsupported protobuf wire type")
        if pos > limit:
            raise SnapshotError("Truncated protobuf field")
        result.append(_Field(number, wire, field_start, payload_start, pos, integer))
    return result


def _checked_data(data: bytes):
    if not isinstance(data, bytes):
        raise TypeError("Snapshot must be immutable bytes")
    if len(data) > MAX_SNAPSHOT_BYTES:
        raise SnapshotError("Snapshot exceeds supported size")


def _fallback_target(data: bytes):
    _checked_data(data)
    matches = []
    for outer in _fields(data):
        if outer.number in (1, 2, 3, 5) and outer.wire != 2:
            raise SnapshotError("Fallback snapshot field has an unexpected type")
        if outer.number == 4 and outer.wire != 0:
            raise SnapshotError("Fallback snapshot field has an unexpected type")
        if outer.number != 5:
            continue
        nested = _fields(data, outer.payload_start, outer.end)
        names = [field for field in nested if field.number == 1]
        if len(names) > 1:
            raise SnapshotError("Ambiguous flag name")
        if not names:
            continue
        name = names[0]
        if name.wire != 2:
            raise SnapshotError("Flag name has an unexpected type")
        if data[name.payload_start:name.end] != _TARGET_NAME:
            continue
        values = [field for field in nested if 2 <= field.number <= 6]
        if len(values) != 1 or values[0].number != 3 or values[0].wire != 0:
            raise SnapshotError("Target flag is not a unique boolean")
        value = values[0]
        if value.integer not in (0, 1) or value.end - value.payload_start != 1:
            raise SnapshotError("Target flag boolean is not canonically encoded")
        matches.append(value)
    if len(matches) > 1:
        raise SnapshotError("Duplicate target flag")
    return matches[0] if matches else None


def inspect_fallback(data: bytes) -> dict:
    """Report only flag 1; its confirmed absent-value default is true."""
    value = _fallback_target(data)
    return {
        "present": value is not None,
        "stored_value": None if value is None else bool(value.integer),
        "effective_value": True if value is None else bool(value.integer),
    }


def patch_fallback(data: bytes) -> bytes:
    """Set only flag 1 to false, preserving every other original byte."""
    value = _fallback_target(data)
    if value is None:
        if len(data) + len(_APPENDED_FLAG) > MAX_SNAPSHOT_BYTES:
            raise SnapshotError("Patched snapshot exceeds supported size")
        patched = data + _APPENDED_FLAG
    elif value.integer == 0:
        return data
    else:
        patched = data[:value.payload_start] + b"\x00" + data[value.end:]
    if inspect_fallback(patched) != {
        "present": True, "stored_value": False, "effective_value": False
    }:
        raise SnapshotError("Target flag verification failed")
    return patched


def _metadata_target(data: bytes, start: int, end: int):
    matches = []
    for outer in _fields(data, start, end):
        if outer.number in (1, 2, 3, 5) and outer.wire != 2:
            raise SnapshotError("Primary metadata field has an unexpected type")
        if outer.number == 4 and outer.wire != 0:
            raise SnapshotError("Primary metadata field has an unexpected type")
        if outer.number != 5:
            continue
        entry = _fields(data, outer.payload_start, outer.end)
        keys = [field for field in entry if field.number == 1]
        values = [field for field in entry if field.number == 2]
        if len(keys) > 1 or len(values) > 1:
            raise SnapshotError("Ambiguous primary override entry")
        if any(field.wire != 2 for field in keys + values):
            raise SnapshotError("Primary override entry has an unexpected type")
        key = data[keys[0].payload_start:keys[0].end] if keys else b""
        if not values:
            if key == _TARGET_NAME:
                raise SnapshotError("Target override value is absent")
            continue
        nested = _fields(data, values[0].payload_start, values[0].end)
        names = [field for field in nested if field.number == 10]
        if len(names) > 1 or any(field.wire != 2 for field in names):
            raise SnapshotError("Ambiguous primary override name")
        name = data[names[0].payload_start:names[0].end] if names else b""
        if key != _TARGET_NAME and name != _TARGET_NAME:
            continue
        if key != _TARGET_NAME or name != _TARGET_NAME:
            raise SnapshotError("Target override key and name disagree")
        oneof = [field for field in nested if 1 <= field.number <= 5]
        if len(oneof) != 1 or oneof[0].number != 2 or oneof[0].wire != 0:
            raise SnapshotError("Target override is not a unique boolean")
        value = oneof[0]
        if value.integer not in (0, 1) or value.end - value.payload_start != 1:
            raise SnapshotError("Target override boolean is not canonically encoded")
        matches.append(value)
    if len(matches) > 1:
        raise SnapshotError("Duplicate target override")
    return matches[0] if matches else None


def _compressed_target(compressed: bytes):
    # hgeo uses new Inflater(true): this is raw DEFLATE, not a zlib envelope.
    try:
        inflater = zlib.decompressobj(-zlib.MAX_WBITS)
        flags = inflater.decompress(compressed, MAX_SNAPSHOT_BYTES + 1)
    except zlib.error:
        raise SnapshotError("Invalid primary flag compression") from None
    if len(flags) > MAX_SNAPSHOT_BYTES or inflater.unconsumed_tail:
        raise SnapshotError("Inflated primary flags exceed supported size")
    if not inflater.eof or inflater.unused_data:
        raise SnapshotError("Truncated or trailing primary flag compression")
    count, pos = _varint(flags, 0, len(flags))
    if count > 65536:
        raise SnapshotError("Primary flag count exceeds supported size")
    previous_name = 0
    matches = []
    for _ in range(count):
        header, pos = _varint(flags, pos, len(flags))
        delta, value_type = header >> 3, header & 7
        if delta:
            numeric_name = previous_name + delta
            if numeric_name > (1 << 61) - 1:
                raise SnapshotError("Primary numeric flag name overflow")
            previous_name = numeric_name
            is_target = numeric_name == 1
        else:
            length, pos = _varint(flags, pos, len(flags))
            end = pos + length
            if end > len(flags):
                raise SnapshotError("Truncated primary flag name")
            is_target = flags[pos:end] == _TARGET_NAME
            pos = end
        if is_target and value_type not in (0, 1):
            raise SnapshotError("Target primary flag is not boolean")
        if value_type in (0, 1):
            if is_target:
                matches.append(bool(value_type))
        elif value_type == 2:
            _, pos = _varint(flags, pos, len(flags))
        elif value_type == 3:
            pos += 8
        elif value_type in (4, 5):
            length, pos = _varint(flags, pos, len(flags))
            pos += length
        else:
            raise SnapshotError("Unsupported primary flag type")
        if pos > len(flags):
            raise SnapshotError("Truncated primary flag value")
    if pos != len(flags):
        raise SnapshotError("Trailing primary flag data")
    if len(matches) > 1:
        raise SnapshotError("Duplicate target primary flag")
    return matches[0] if matches else None


@dataclass(frozen=True)
class _Primary:
    metadata_start: int
    metadata_end: int
    override: Optional[_Field]
    base_value: Optional[bool]


def _primary(data: bytes):
    _checked_data(data)
    if len(data) < 10:
        raise SnapshotError("Truncated primary header")
    # hgfd.c(): jinr.l() twice (fixed32 LE), jinr.o() for metadata length,
    # metadata protobuf, jinr.o() for compressed length, compressed bytes.
    version = struct.unpack_from("<I", data)[0]
    if version not in (0, 1):
        raise SnapshotError("Unsupported primary snapshot version")
    length, start = _varint(data, 8, len(data))
    end = start + length
    if end >= len(data):
        raise SnapshotError("Truncated primary metadata")
    override = _metadata_target(data, start, end)
    compressed_length, compressed_start = _varint(data, end, len(data))
    if compressed_start + compressed_length != len(data):
        raise SnapshotError("Truncated or trailing primary compressed frame")
    base_value = _compressed_target(data[compressed_start:])
    return _Primary(start, end, override, base_value)


def inspect_primary(data: bytes) -> dict:
    """Report effective flag 1, resolving the exact hgek override map first."""
    state = _primary(data)
    value = bool(state.override.integer) if state.override else state.base_value
    return {
        "present": value is not None,
        "stored_value": value,
        "effective_value": True if value is None else value,
    }


def patch_primary(data: bytes) -> bytes:
    """Add/update only the flag 1 metadata override; retain raw flags bytes."""
    state = _primary(data)
    if state.override:
        if state.override.integer == 0:
            return data
        value = state.override
        patched = data[:value.payload_start] + b"\x00" + data[value.end:]
    elif state.base_value is False:
        return data
    else:
        metadata = data[state.metadata_start:state.metadata_end]
        metadata += _APPENDED_PRIMARY_OVERRIDE
        patched = data[:8] + _encode_varint(len(metadata)) + metadata + data[state.metadata_end:]
        if len(patched) > MAX_SNAPSHOT_BYTES:
            raise SnapshotError("Patched snapshot exceeds supported size")
    if inspect_primary(patched) != {
        "present": True, "stored_value": False, "effective_value": False
    }:
        raise SnapshotError("Target primary flag verification failed")
    return patched
