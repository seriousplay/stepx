from pathlib import Path
import struct
import sys
import unittest
import zlib


sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
import google_play_compat_snapshot as snapshot


def varint(value):
    out = bytearray()
    while value >= 128:
        out.append((value & 127) | 128)
        value >>= 7
    out.append(value)
    return bytes(out)


def blob(number, content):
    return varint((number << 3) | 2) + varint(len(content)) + content


def flag(name=b"1", value=b"\x18\x01", unknown=b""):
    return blob(5, blob(1, name) + value + unknown)


def override(name=b"1", value=b"\x10\x01", unknown=b"", key=None):
    entry_value = value + blob(10, name) + unknown
    return blob(5, blob(1, name if key is None else key) + blob(2, entry_value))


def primary(metadata=b"", flags=b"\x01\x09", version=1):
    compressor = zlib.compressobj(wbits=-zlib.MAX_WBITS)
    compressed = compressor.compress(flags) + compressor.flush()
    return struct.pack("<II", version, 0x12345678) + varint(len(metadata)) + metadata + varint(len(compressed)) + compressed


class GooglePlayCompatSnapshotTest(unittest.TestCase):
    def test_true_changes_exactly_one_byte_and_preserves_unknown_fields(self):
        # Includes unknown varint, fixed64, length-delimited and fixed32
        # fields, both outside and inside the target flag.
        opaque = blob(1, b"opaque-snapshot") + blob(2, b"opaque-metadata")
        unknown = b"\xa0\x06\x81\x00\xa9\x06abcdefgh\xb2\x06\x03xyz\xbd\x06abcd"
        before = opaque + unknown + flag(unknown=unknown) + flag(b"other") + blob(3, b"opaque-server")
        after = snapshot.patch_fallback(before)
        changed = [i for i, (a, b) in enumerate(zip(before, after)) if a != b]
        self.assertEqual(len(after), len(before))
        self.assertEqual(len(changed), 1)
        self.assertEqual((before[changed[0]], after[changed[0]]), (1, 0))
        self.assertEqual(snapshot.inspect_fallback(after), {
            "present": True, "stored_value": False, "effective_value": False,
        })
        self.assertEqual(snapshot.patch_fallback(after), after)

    def test_existing_false_is_byte_identical(self):
        before = blob(3, b"metadata") + flag(value=b"\x18\x00")
        self.assertIs(snapshot.patch_fallback(before), before)

    def test_missing_target_appends_only_legal_false_flag(self):
        for before in (b"", blob(1, b"metadata"), flag(b"2")):
            with self.subTest(length=len(before)):
                self.assertEqual(snapshot.inspect_fallback(before), {
                    "present": False, "stored_value": None, "effective_value": True,
                })
                after = snapshot.patch_fallback(before)
                self.assertEqual(after, before + b"\x2a\x05\x0a\x01\x31\x18\x00")
                self.assertFalse(snapshot.inspect_fallback(after)["effective_value"])

    def test_duplicate_target_is_rejected(self):
        for data in (flag() + flag(), flag() + flag(value=b"\x18\x00")):
            with self.assertRaises(snapshot.SnapshotError):
                snapshot.patch_fallback(data)

    def test_non_boolean_or_ambiguous_target_is_rejected(self):
        for value in (b"", b"\x10\x01", blob(5, b"false"), b"\x18\x01\x18\x00", b"\x18\x00\x10\x01", b"\x18\x02", b"\x18\x80\x00"):
            with self.subTest(value_type=value[:1]):
                with self.assertRaises(snapshot.SnapshotError):
                    snapshot.patch_fallback(flag(value=value))
        with self.assertRaises(snapshot.SnapshotError):
            snapshot.patch_fallback(blob(5, blob(1, b"1") + blob(1, b"1") + b"\x18\x00"))

    def test_truncation_and_invalid_wire_data_are_rejected(self):
        encoded = flag()
        malformed = [encoded[:i] for i in range(1, len(encoded))]
        malformed += [b"\x80", b"\x00", b"\x0a\xff", b"\x09abc", b"\x0dabc", b"\x0f", b"\xff" * 11, b"\x28\x01"]
        for data in malformed:
            with self.subTest(length=len(data)):
                with self.assertRaises(snapshot.SnapshotError):
                    snapshot.patch_fallback(data)

    def test_report_and_errors_never_include_opaque_metadata(self):
        marker = b"DO-NOT-REPORT-OPAQUE-DATA"
        data = blob(1, marker) + flag()
        self.assertNotIn(marker.decode(), repr(snapshot.inspect_fallback(data)))
        try:
            snapshot.patch_fallback(data + flag())
        except snapshot.SnapshotError as error:
            self.assertNotIn(marker.decode(), str(error))
        else:
            self.fail("Duplicate target unexpectedly accepted")

    def test_primary_adds_false_override_without_touching_compressed_flags(self):
        metadata = blob(1, b"opaque-snapshot") + blob(2, b"opaque-metadata") + blob(3, b"opaque-server") + b"\xa0\x06\x81\x00"
        for flags in (b"\x00", b"\x01\x09"):
            with self.subTest(has_target=flags != b"\x00"):
                before = primary(metadata, flags)
                state_before = snapshot._primary(before)
                after = snapshot.patch_primary(before)
                state_after = snapshot._primary(after)
                self.assertEqual(before[:8], after[:8])
                self.assertEqual(after[state_after.metadata_start:state_after.metadata_end], metadata + snapshot._APPENDED_PRIMARY_OVERRIDE)
                self.assertEqual(before[state_before.metadata_end:], after[state_after.metadata_end:])
                self.assertEqual(snapshot.inspect_primary(after), {
                    "present": True, "stored_value": False, "effective_value": False,
                })
                self.assertEqual(snapshot.patch_primary(after), after)

    def test_primary_true_override_changes_one_byte_and_false_is_identical(self):
        metadata = blob(1, b"opaque") + override(unknown=blob(99, b"keep-this"))
        before = primary(metadata, b"\x01\x08")  # base false, override true
        self.assertTrue(snapshot.inspect_primary(before)["effective_value"])
        after = snapshot.patch_primary(before)
        self.assertEqual(len(before), len(after))
        self.assertEqual(sum(a != b for a, b in zip(before, after)), 1)
        self.assertFalse(snapshot.inspect_primary(after)["effective_value"])
        self.assertIs(snapshot.patch_primary(after), after)
        already_false = primary(flags=b"\x01\x08")
        self.assertIs(snapshot.patch_primary(already_false), already_false)

    def test_primary_duplicate_nonbool_or_mismatched_override_is_rejected(self):
        for metadata in (
            override() + override(),
            override(value=b"\x08\x01"),
            override(value=b"\x10\x00\x10\x01"),
            override(key=b"other"),
            override(name=b"other", key=b"1"),
            blob(5, blob(1, b"1")),
        ):
            with self.assertRaises(snapshot.SnapshotError):
                snapshot.patch_primary(primary(metadata))
        for flags in (b"\x01\x0a\x01", b"\x02\x09\x01\x01\x31"):
            with self.assertRaises(snapshot.SnapshotError):
                snapshot.patch_primary(primary(flags=flags))

    def test_primary_truncated_header_metadata_and_compression_are_rejected(self):
        before = primary(blob(1, b"opaque"))
        for cut in range(len(before)):
            with self.subTest(cut=cut):
                with self.assertRaises(snapshot.SnapshotError):
                    snapshot.patch_primary(before[:cut])
        for bad in (primary(version=2), before + b"x", primary(flags=b"\x02\x09"), primary(flags=b"\x00x"), primary(flags=b"\x01\x0e")):
            with self.assertRaises(snapshot.SnapshotError):
                snapshot.patch_primary(bad)

    def test_primary_reports_only_target_flag_and_checks_inflate_limit(self):
        marker = b"DO-NOT-REPORT-OPAQUE-DATA"
        before = primary(blob(1, marker), b"\x00")
        self.assertEqual(snapshot.inspect_primary(before), {
            "present": False, "stored_value": None, "effective_value": True,
        })
        self.assertNotIn(marker.decode(), repr(snapshot.inspect_primary(before)))
        with self.assertRaises(snapshot.SnapshotError):
            snapshot.patch_primary(primary(flags=b"x" * (snapshot.MAX_SNAPSHOT_BYTES + 1)))

    def test_size_and_mutable_inputs_are_rejected(self):
        with self.assertRaises(snapshot.SnapshotError):
            snapshot.patch_fallback(b"x" * (snapshot.MAX_SNAPSHOT_BYTES + 1))
        with self.assertRaises(TypeError):
            snapshot.patch_fallback(bytearray(flag()))


if __name__ == "__main__":
    unittest.main()
