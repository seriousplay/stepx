from pathlib import Path
import struct
import sys
import unittest


SCRIPTS = Path(__file__).resolve().parents[2] / "scripts"
sys.path.insert(0, str(SCRIPTS))
import google_play_compat_store_cache as cache


def utf(payload):
    return struct.pack(">H", len(payload)) + payload


def record(kind, name, payload):
    return bytes([kind]) + utf(name) + payload


KEY = cache.TARGET_FLAG.encode("ascii")
HEADER = b"\x01" + utf(b"private-header-one") + utf(b"private-header-two") + utf(b"private-header-three")


def target(payload=b"\x0a\x0cMainActivity", name=KEY):
    return record(6, name, struct.pack(">i", len(payload)) + payload)


class GooglePlayCompatStoreCacheTest(unittest.TestCase):
    def assert_invalid(self, data):
        for operation in (cache.inspect_cache, cache.patch_cache):
            with self.subTest(operation=operation.__name__):
                with self.assertRaises(cache.StoreCacheError):
                    operation(data)

    def test_all_record_types_and_headers_preserved_exactly(self):
        # Includes signed numeric extrema, raw NaN payload, noncanonical true
        # and unrelated duplicate names. None may be normalized or dropped.
        records = [
            record(1, b"integer", b"\x80"),
            record(2, b"integer", b"\x80\x00"),
            record(3, b"i32", b"\x80\x00\x00\x00"),
            record(4, b"i64", b"\x80" + b"\x00" * 7),
            record(5, b"string", utf(b"raw\xc0\x80text")),
            record(6, b"bytes", struct.pack(">i", 5) + b"\x00\xffabc"),
            record(7, b"double", bytes.fromhex("7ff8000000000042")),
            record(8, b"boolean", b"\xff"),
        ]
        for position in range(len(records) + 1):
            with self.subTest(position=position):
                prefix = HEADER + b"".join(records[:position])
                suffix = b"".join(records[position:]) + b"\x00"
                original = prefix + target() + suffix
                self.assertEqual(cache.inspect_cache(original), {
                    "present": True, "empty": False, "size": 14,
                })
                patched = cache.patch_cache(original)
                self.assertEqual(patched, prefix + target(b"") + suffix)
                self.assertEqual(cache.inspect_cache(patched), {
                    "present": True, "empty": True, "size": 0,
                })
                self.assertIs(cache.patch_cache(patched), patched)

    def test_absent_target_inserted_immediately_before_terminator(self):
        for records in (b"", record(5, b"unrelated", utf(b"value"))):
            original = HEADER + records + b"\x00"
            self.assertEqual(cache.inspect_cache(original), {
                "present": False, "empty": False, "size": None,
            })
            self.assertEqual(cache.patch_cache(original), original[:-1] + target(b"") + b"\x00")

    def test_already_empty_is_identical_object(self):
        original = HEADER + target(b"") + b"\x00"
        self.assertIs(cache.patch_cache(original), original)

    def test_duplicate_target_rejected_including_empty(self):
        for first in (b"", b"first"):
            for second in (b"", b"second"):
                self.assert_invalid(HEADER + target(first) + target(second) + b"\x00")

    def test_target_requires_byte_array_type(self):
        for kind, payload in ((1, b"\x00"), (2, b"\x00" * 2), (3, b"\x00" * 4),
                              (4, b"\x00" * 8), (5, utf(b"")), (7, b"\x00" * 8), (8, b"\x00")):
            with self.subTest(kind=kind):
                self.assert_invalid(HEADER + record(kind, KEY, payload) + b"\x00")

    def test_every_truncation_rejected(self):
        original = (HEADER + record(5, b"unicode-\xe4\xb8\xad", utf(b"\xed\xa0\xbd\xed\xb8\x80"))
                    + target() + record(4, b"last", b"\x00" * 8) + b"\x00")
        for position in range(len(original)):
            with self.subTest(position=position):
                self.assert_invalid(original[:position])

    def test_invalid_version_record_type_and_trailing_bytes_rejected(self):
        for version in (0, 2, 255):
            self.assert_invalid(bytes([version]) + HEADER[1:] + b"\x00")
        for kind in (9, 127, 128, 255):
            self.assert_invalid(HEADER + record(kind, b"other", b"") + b"\x00")
        for suffix in (b"\x00", b"garbage", target()):
            self.assert_invalid(HEADER + b"\x00" + suffix)

    def test_negative_and_out_of_bounds_byte_array_lengths_rejected(self):
        for name in (KEY, b"other"):
            for length in (-1, -2147483648, 2147483647, 100):
                with self.subTest(length=length, target=name == KEY):
                    self.assert_invalid(HEADER + record(6, name, struct.pack(">i", length)) + b"\x00")

    def test_java_modified_utf_is_validated_in_headers_names_and_values(self):
        invalid = (b"\x80", b"\xc0", b"\xc2A", b"\xe0\x80", b"\xe0A\x80", b"\xe0\x80A",
                   b"\xf0\x9f\x98\x80", b"\xff")
        for bad in invalid:
            with self.subTest(bad=bad):
                self.assert_invalid(b"\x01" + utf(bad) + utf(b"") * 2 + b"\x00")
                self.assert_invalid(HEADER + record(8, bad, b"\x01") + b"\x00")
                self.assert_invalid(HEADER + record(5, b"other", utf(bad)) + b"\x00")
        self.assert_invalid(b"\x01\xff\xffshort")
        self.assert_invalid(HEADER + b"\x05\xff\xffshort")
        self.assert_invalid(HEADER + record(5, b"other", b"\xff\xffshort"))

    def test_java_utf_length_is_bytes_and_accepts_full_unsigned_short(self):
        payload = b"\xe4\xb8\xad" * 21845
        self.assertEqual(len(payload), 65535)
        header = b"\x01" + utf(payload) + utf(b"\xc0\x80") + utf(b"\xed\xa0\xbd\xed\xb8\x80")
        original = header + target() + b"\x00"
        self.assertEqual(cache.patch_cache(original), header + target(b"") + b"\x00")

    def test_java_accepted_noncanonical_name_cannot_hide_duplicate_target(self):
        # DataInputStream accepts overlong two-/three-byte encodings. Match
        # decoded names, but retain their exact raw representation on write.
        for encoded_g in (b"\xc1\x87", b"\xe0\x81\x87"):
            name = encoded_g + KEY[1:]
            self.assertEqual(cache.patch_cache(HEADER + target(name=name) + b"\x00"),
                             HEADER + target(b"", name=name) + b"\x00")
            self.assert_invalid(HEADER + target(name=name) + target() + b"\x00")

    def test_valid_java_code_units_remain_unchanged(self):
        # NUL, unpaired surrogate and accepted overlong forms are Java UTF
        # code units; Python's ordinary UTF-8 decoder would reject some.
        value = b"\x00\xc0\x80\xc1\x81\xe0\x81\x81\xed\xa0\x80"
        original = HEADER + record(5, value, utf(value)) + b"\x00"
        self.assertEqual(cache.patch_cache(original), original[:-1] + target(b"") + b"\x00")

    def test_report_and_errors_never_expose_metadata(self):
        marker = b"PRIVATE-METADATA-DO-NOT-REPORT"
        original = b"\x01" + utf(marker) * 3 + target(marker) + b"\x00"
        report = cache.inspect_cache(original)
        self.assertEqual(set(report), {"present", "empty", "size"})
        self.assertNotIn(marker.decode(), repr(report))
        try:
            cache.patch_cache(original[:-1] + target() + b"\x00")
        except cache.StoreCacheError as error:
            self.assertNotIn(marker.decode(), str(error))
        else:
            self.fail("Duplicate target unexpectedly accepted")

    def test_mutable_and_non_bytes_inputs_rejected(self):
        for value in (bytearray(HEADER), memoryview(HEADER), "", None, 1):
            for operation in (cache.inspect_cache, cache.patch_cache):
                with self.assertRaises(TypeError):
                    operation(value)

    def test_source_executes_without_module_registration_or_cli_io(self):
        namespace = {}
        source = (SCRIPTS / "google_play_compat_store_cache.py").read_text()
        exec(compile(source, "store-cache-worker", "exec"), namespace)
        original = HEADER + target() + b"\x00"
        self.assertEqual(namespace["patch_cache"](original), HEADER + target(b"") + b"\x00")


if __name__ == "__main__":
    unittest.main()
