"""Exercise real OOXML outputs and constrained document edits without a model."""
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import xml.etree.ElementTree as ET
import zipfile

ROOT = Path(__file__).resolve().parents[1]
GENERATOR = ROOT / 'src/work_generate.py'
EXTRACTOR = ROOT / 'src/work_extract.py'
spec = importlib.util.spec_from_file_location('work_generate', GENERATOR)
tools = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tools)


def generate(payload, source=None, success=True):
    cmd = [sys.executable, '-I', str(GENERATOR)] + ([str(source)] if source else [])
    result = subprocess.run(cmd, input=json.dumps(payload).encode(), capture_output=True, timeout=20)
    if success:
        if result.returncode: raise AssertionError(result.stderr.decode())
        with zipfile.ZipFile(io.BytesIO(result.stdout)) as archive:
            for name in archive.namelist():
                if name.endswith('.xml') or name.endswith('.rels'): ET.fromstring(archive.read(name))
        return result.stdout
    if result.returncode == 0: raise AssertionError('invalid tool input succeeded')
    if result.stdout: raise AssertionError('failed tool wrote a partial artifact')
    return result.stderr.decode()


class OfficeToolsTest(unittest.TestCase):
    def test_formula_values_are_computed_not_supplied_by_model(self):
        payload = {'tool':'write_spreadsheet','sheet_name':'预算','column_widths':[24,18,18,22], 'rows':[
            ['项目','数量','单价','金额'], ['A',3,12.5,{'formula':'ROUND(B2*C2,2)','format':'currency'}],
            ['B',2,7.25,{'formula':'B3*C3','format':'currency'}],
            ['合计',None,None,{'formula':'SUM(D2:D3)','format':'currency'}],
            ['占比',None,None,{'formula':'IF(D4=0,0,D2/D4)','format':'percent'}]]}
        data = generate(payload)
        with zipfile.ZipFile(io.BytesIO(data)) as z:
            root = ET.fromstring(z.read('xl/worksheets/sheet1.xml'))
            cells = {n.get('r'): n for n in root.findall('.//{'+tools.S+'}c')}
            self.assertEqual(float(cells['D4'].findtext('{'+tools.S+'}v')),52.0)
            self.assertEqual(cells['D4'].findtext('{'+tools.S+'}f'),'SUM(D2:D3)')
            self.assertAlmostEqual(float(cells['D5'].findtext('{'+tools.S+'}v')),37.5/52)
            self.assertIn(b'numFmtId="164"',z.read('xl/styles.xml'))

    def test_formula_guards_and_literal_formula_text(self):
        self.assertEqual(tools.Formula('2*3^2',lambda _:None).value(),18)
        self.assertEqual(tools.Formula('2^3^2',lambda _:None).value(),64)
        self.assertEqual(tools.Formula('IF(A1=0,1,2)',lambda _:None).value(),1)
        self.assertEqual(tools.Formula('ROUND(2.675,2)',lambda _:None).value(),2.68)
        self.assertEqual(tools.Formula('IF(0=0,0,1/0)',lambda _:None).value(),0)
        for expression in ['WEBSERVICE("https://invalid")','IF(TRUE,1,WEBSERVICE("x"))',"'Sheet'!A1",'A1+[other.xlsx]B2','SUM(A1:ZZZ99999)','(-1)^0.5']:
            with self.assertRaises((ValueError,TypeError)):
                tools.Formula(expression,lambda _:1).value()
        bad = generate({'tool':'write_spreadsheet','rows':[[{'formula':'B1'},{'formula':'A1'}]]},success=False)
        self.assertIn('循环',bad)
        data = generate({'tool':'write_spreadsheet','rows':[['=WEBSERVICE("x")',{'value':0.2,'format':'percent'}]]})
        with zipfile.ZipFile(io.BytesIO(data)) as z:
            root = ET.fromstring(z.read('xl/worksheets/sheet1.xml'))
            self.assertEqual(len(root.findall('.//{'+tools.S+'}f')),0)

    def test_edit_word_cross_run_preserves_styles_and_other_parts(self):
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp)/'original.docx'
            original = generate({'tool':'write_document','title':'方案','paragraphs':['旧预算 100 元','下一段保持不变']})
            with zipfile.ZipFile(io.BytesIO(original)) as z, zipfile.ZipFile(source,'w') as out:
                for name in z.namelist():
                    data = z.read(name)
                    if name == 'word/document.xml':
                        data = data.replace('旧预算 100 元'.encode(), '旧预算 '.encode())
                        data = data.replace('旧预算 </w:t></w:r>'.encode(), '旧预算 </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>100</w:t></w:r><w:r><w:t> 元</w:t></w:r>'.encode())
                    out.writestr(name,data)
                out.writestr('word/media/preserved.bin',b'keep-exact-bytes')
            baseline = source.read_bytes()
            data = generate({'tool':'edit_document','replacements':[{'find':'预算 100 元','replace':'预算 120 元','expected_count':1}]},source)
            self.assertEqual(source.read_bytes(),baseline)
            with zipfile.ZipFile(io.BytesIO(data)) as z:
                root = ET.fromstring(z.read('word/document.xml'))
                text = ''.join(n.text or '' for n in root.iter('{'+tools.W+'}t'))
                self.assertIn('旧预算 120 元',text)
                self.assertIn('下一段保持不变',text)
                self.assertEqual(z.read('word/media/preserved.bin'),b'keep-exact-bytes')
                self.assertTrue(list(root.iter('{'+tools.W+'}b')))
            err = generate({'tool':'edit_document','replacements':[{'find':'不存在','replace':'x'}]},source,False)
            self.assertIn('匹配 0',err)

    def test_presentation_has_real_slides_master_theme_and_ordered_extraction(self):
        slides = [{'title':'季度成果','subtitle':'从真实指标到执行计划','layout':'cover'}]
        slides += [{'title':f'第 {i} 页','bullets':['核心指标与验证依据','下一步行动与负责人']} for i in range(2,12)]
        data = generate({'tool':'write_presentation','slides':slides})
        with tempfile.TemporaryDirectory() as tmp:
            p = Path(tmp)/'report.pptx';p.write_bytes(data)
            text = subprocess.check_output([sys.executable,'-I',str(EXTRACTOR),str(p)],text=True)
            self.assertLess(text.index('第 2 页'),text.index('第 10 页'))
        with zipfile.ZipFile(io.BytesIO(data)) as z:
            root = ET.fromstring(z.read('ppt/presentation.xml'))
            self.assertEqual(len(root.findall('.//{'+tools.P+'}sldId')),11)
            self.assertIn('ppt/theme/theme1.xml',z.namelist())
            for info in z.infolist():
                if info.filename.endswith('.rels'):
                    self.assertNotIn(b'TargetMode="External"',z.read(info))
        self.assertIn('文字过多',generate({'tool':'write_presentation','slides':[{'title':'内容','bullets':['长'*160]*6}]},success=False))


if __name__ == '__main__':
    unittest.main()
