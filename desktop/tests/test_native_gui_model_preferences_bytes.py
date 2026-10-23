"""Local MMKV-format fixtures; no Android/native/device calls."""
import importlib.util
import json
from pathlib import Path
import struct
import unittest
import zlib

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('model_preferences_bytes', ROOT / 'scripts/gui-native-model-preferences.py')
reader = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(reader)


def varint(value):
    out = bytearray()
    while value >= 128:
        out.append((value & 127) | 128)
        value >>= 7
    out.append(value)
    return bytes(out)


def field(value):
    return varint(len(value)) + value


def item(key, value):
    return field(key.encode()) + field(b'' if value is None else field(value.encode()))


def files(payload, version=4):
    data = struct.pack('<I', len(payload)) + payload + b'\0' * 48
    crc = bytearray(112)
    struct.pack_into('<II', crc, 0, zlib.crc32(payload), version)
    struct.pack_into('<I', crc, 28, len(payload))
    return data, bytes(crc)


class BinaryModelPreferencesTest(unittest.TestCase):
    def test_default_and_noncanonical_size_holder(self):
        # Actual MMKV deliberately ignores the first random/fake size holder.
        data, crc = files(b'\xff\xff\xff\x7f' + item('current_model_id', reader.DEFAULT_ID))
        result = reader.parse_preferences(data, crc)
        self.assertEqual(result['current_model_id'], reader.DEFAULT_ID)
        self.assertTrue(result['matches_static_default'])

    def test_custom_selection_skips_unknown_values_and_redacts(self):
        payload = varint(7) + item('current_model_id', 'provider/current-gui')
        payload += item('model_map', json.dumps({'provider/current-gui': 'GeminiModelCell', 'token': 'PRIVATE'}))
        payload += field(b'token') + field(b'\xff\xfePRIVATE_URL')
        result = reader.parse_preferences(*files(payload))
        self.assertEqual(result['model_cell_type'], 'GeminiModelCell')
        self.assertTrue(result['custom_model'])
        self.assertNotIn('PRIVATE', json.dumps(result))
        self.assertNotIn('token', json.dumps(result))

    def test_append_override_and_tombstone(self):
        payload = varint(0) + item('current_model_id', 'stale') + item('current_model_id', reader.DEFAULT_ID)
        self.assertEqual(reader.parse_preferences(*files(payload))['current_model_id'], reader.DEFAULT_ID)
        result = reader.parse_preferences(*files(payload + item('current_model_id', None)))
        self.assertEqual(result['status'], 'no_saved_selection')

    def test_empty_valid_file(self):
        self.assertEqual(reader.parse_preferences(*files(b''))['status'], 'no_saved_selection')

    def test_crc_mismatch_and_length_mismatch(self):
        data, crc = files(varint(0) + item('current_model_id', reader.DEFAULT_ID))
        corrupt = bytearray(data)
        corrupt[8] ^= 1
        with self.assertRaisesRegex(ValueError, '^crc_mismatch$'):
            reader.parse_preferences(bytes(corrupt), crc)
        bad_meta = bytearray(crc)
        struct.pack_into('<I', bad_meta, 28, 1)
        with self.assertRaisesRegex(ValueError, '^inconsistent_actual_length$'):
            reader.parse_preferences(data, bytes(bad_meta))

    def test_unsupported_metadata_rejected(self):
        data, crc = files(varint(0))
        for offset, value, expected in ((4, 5, 'unsupported_meta_version'), (12, 1, 'unsupported_crypto_metadata'),
                                        (104, 1, 'unsupported_meta_flags')):
            bad = bytearray(crc)
            bad[offset] = value
            with self.assertRaisesRegex(ValueError, '^' + expected + '$'):
                reader.parse_preferences(data, bytes(bad))

    def test_no_recovery_from_malformed_data(self):
        for payload in (b'\x80', b'\xff\xff\xff\xff\x1f', b'\0\xff\x7f', b'\0\0'):
            with self.assertRaises(ValueError):
                reader.parse_preferences(*files(payload))
        malformed_string = varint(0) + field(b'current_model_id') + field(field(b'valid') + b'trailer')
        with self.assertRaisesRegex(ValueError, '^unexpected_string_trailer$'):
            reader.parse_preferences(*files(malformed_string))

    def test_unregistered_and_private_looking_values_not_returned(self):
        for value, status in (('private-opaque', 'unregistered_selection'),
                              ('https://private.invalid/token', 'invalid_selection'), ('../private', 'invalid_selection')):
            result = reader.parse_preferences(*files(varint(0) + item('current_model_id', value)))
            self.assertEqual(result['status'], status)
            self.assertNotIn(value, json.dumps(result))

    def test_generated_worker_compiles_without_execution(self):
        code = reader.build_worker()
        compile(code, 'generated-worker', 'exec')
        self.assertNotIn('ctypes', code)
        self.assertNotIn('mmap(', code)
        self.assertNotIn('O_WRONLY', code)
        self.assertIn('os.O_RDONLY', code)


if __name__ == '__main__':
    unittest.main()
