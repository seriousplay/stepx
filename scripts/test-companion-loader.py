import ast
import unittest
from companion_loader import IMAGE_BASE,adapt
from pathlib import Path

class LoaderTests(unittest.TestCase):
    def bias(self,maps):
        scope={'maps':maps};exec(IMAGE_BASE,scope);return scope['image_base']('libc.so')
    def test_ignores_earlier_readonly_file_copy(self):
        self.assertEqual(self.bias(['100000-300000 r--p 00000000 00:01 1 /libc.so',
            '458000-4c3000 r-xp 00058000 00:01 1 /libc.so']),0x400000)
    def test_rejects_missing_or_ambiguous_executable_copy(self):
        for maps in [['100000-300000 r--p 00000000 00:01 1 /libc.so'],
            ['458000-4c3000 r-xp 00058000 00:01 1 /libc.so','858000-8c3000 r-xp 00058000 00:01 1 /libc.so']]:
            with self.assertRaises(RuntimeError):self.bias(maps)
    def test_fixed_loader_still_parses_and_validates_before_attach(self):
        source=adapt((Path(__file__).resolve().parents[1]/'handoff/google-play-stepx/module/inject.py').read_text())
        ast.parse(source)
        self.assertLess(source.index('Native entry point is not executable'),source.index('pt(0x4206,t)'))
    def test_template_drift_fails_closed(self):
        with self.assertRaises(RuntimeError):adapt('different loader')

if __name__=='__main__':unittest.main()
