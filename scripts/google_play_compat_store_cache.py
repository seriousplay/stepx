"""Lossless editor for one Play Store 53.6.15 experiment-cache flag.

The format is the version-1 DataInputStream format read by Play's aohp:
three Java modified-UTF headers, typed flag records, then a zero terminator.
The sole permitted change is an empty byte array for the activity block list.
This changes a local activity gate; it does not confer Google certification.

All functions operate on immutable bytes. File discovery, backups, atomic
writes, process lifecycle and rollback belong to the caller. The module has
no file I/O or CLI and can be exec'd into a worker namespace. Reports and
errors never contain header tokens, account identifiers or unrelated flags.
"""

import struct


TARGET_FLAG = "GmscoreCompliance__activities_to_block_on_uncertified_devices"
_TARGET_BYTES = TARGET_FLAG.encode("ascii")
_EMPTY_RECORD = b"\x06" + struct.pack(">H", len(_TARGET_BYTES)) + _TARGET_BYTES + b"\x00\x00\x00\x00"
_FIXED_SIZES = {1: 1, 2: 2, 3: 4, 4: 8, 7: 8, 8: 1}


class StoreCacheError(ValueError):
    """Invalid or unsupported cache, without including its contents."""


class _Reader:
    def __init__(self, data):
        self.data = data
        self.pos = 0

    def skip(self, length):
        if length < 0 or length > len(self.data) - self.pos:
            raise StoreCacheError("Truncated cache field or invalid length")
        start = self.pos
        self.pos += length
        return start

    def byte(self):
        return self.data[self.skip(1)]

    def utf(self, decode=False):
        """Validate Java readUTF framing/encoding, optionally decode a name.

        Match DataInputStream's UTF-16 code-unit decoding, including encoded
        NUL, surrogate pairs and its accepted noncanonical 1/2/3-byte forms.
        Four-byte standard UTF-8 is invalid here. Decoding names also detects
        duplicate target names with different accepted byte encodings.
        """
        length_pos = self.skip(2)
        length = struct.unpack_from(">H", self.data, length_pos)[0]
        pos = self.skip(length)
        end = self.pos
        chars = [] if decode else None
        while pos < end:
            first = self.data[pos]
            pos += 1
            if first <= 0x7f:
                value = first
            elif 0xc0 <= first <= 0xdf:
                if pos >= end or self.data[pos] & 0xc0 != 0x80:
                    raise StoreCacheError("Invalid Java UTF encoding")
                value = ((first & 0x1f) << 6) | (self.data[pos] & 0x3f)
                pos += 1
            elif 0xe0 <= first <= 0xef:
                if (end - pos < 2 or self.data[pos] & 0xc0 != 0x80
                        or self.data[pos + 1] & 0xc0 != 0x80):
                    raise StoreCacheError("Invalid Java UTF encoding")
                value = ((first & 0x0f) << 12) | ((self.data[pos] & 0x3f) << 6) | (self.data[pos + 1] & 0x3f)
                pos += 2
            else:
                raise StoreCacheError("Invalid Java UTF encoding")
            if chars is not None:
                chars.append(chr(value))
        return "".join(chars) if chars is not None else None


def _parse(data):
    if not isinstance(data, bytes):
        raise TypeError("Cache must be immutable bytes")
    reader = _Reader(data)
    if reader.byte() != 1:
        raise StoreCacheError("Unsupported cache version")
    for _ in range(3):
        reader.utf()
    target = None
    while True:
        record_start = reader.pos
        kind = reader.byte()
        if kind == 0:
            if reader.pos != len(data):
                raise StoreCacheError("Trailing bytes after cache terminator")
            return target, record_start
        if kind not in _FIXED_SIZES and kind not in (5, 6):
            raise StoreCacheError("Unsupported cache record type")
        is_target = reader.utf(decode=True) == TARGET_FLAG
        if is_target:
            if target is not None:
                raise StoreCacheError("Duplicate target flag")
            if kind != 6:
                raise StoreCacheError("Target flag must be a byte array")
        if kind == 5:
            reader.utf()
        elif kind == 6:
            length_pos = reader.skip(4)
            length = struct.unpack_from(">i", data, length_pos)[0]
            reader.skip(length)
            if is_target:
                target = (length_pos, reader.pos, length)
        else:
            # Java readBoolean accepts every nonzero byte as true. Preserve
            # all raw numeric/boolean bits, including NaN payloads, unchanged.
            reader.skip(_FIXED_SIZES[kind])


def inspect_cache(data: bytes) -> dict:
    """Report only the target's presence, emptiness and payload byte length.

    Absent is {present: False, empty: False, size: None}; it is distinct from
    a stored empty list. Even unrelated records are validated before return.
    """
    target, _ = _parse(data)
    return {
        "present": target is not None,
        "empty": target is not None and target[2] == 0,
        "size": None if target is None else target[2],
    }


def patch_cache(data: bytes) -> bytes:
    """Empty only the fixed target byte array, or insert it before the end.

    Every other original byte, including all headers and the original target
    name encoding, is retained. Already-empty inputs are returned unchanged.
    """
    target, terminator = _parse(data)
    if target is None:
        result = data[:terminator] + _EMPTY_RECORD + data[terminator:]
    elif target[2] == 0:
        return data
    else:
        length_pos, end, _ = target
        result = data[:length_pos] + b"\x00\x00\x00\x00" + data[end:]
    if inspect_cache(result) != {"present": True, "empty": True, "size": 0}:
        raise StoreCacheError("Target flag verification failed")
    return result
