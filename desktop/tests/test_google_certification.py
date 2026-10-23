from pathlib import Path
import sys
import unittest

TOOLS = Path(__file__).resolve().parents[2] / 'handoff/google-play-stepx/tools'
sys.path.insert(0, str(TOOLS))
import certification


class CertificationTest(unittest.TestCase):
    def table(self, rows):
        headers = ['Retail Branding', 'Marketing Name', 'Device', 'Model']
        return '<table>' + ''.join('<tr>' + ''.join('<td>' + x + '</td>' for x in r) + '</tr>' for r in [headers] + rows) + '</table>'

    def test_same_codename_other_brand_is_not_certification(self):
        identity = {'brand': 'StepX', 'manufacturer': 'StepX', 'model': 'PZ261A'}
        html = self.table([['Xiaomi', 'Xiaomi 17 Pro', 'pandora', '25098PN5AC']])
        self.assertFalse(certification.listed(html, identity))

    def test_exact_brand_and_model_required(self):
        identity = {'brand': 'StepX', 'manufacturer': 'StepX', 'model': 'PZ261A'}
        self.assertFalse(certification.listed(self.table([['Other', 'Phone', 'pandora', 'PZ261A']]), identity))
        self.assertTrue(certification.listed(self.table([['StepX', 'Phone', 'pandora', 'PZ261A']]), identity))

    def test_truncated_or_error_response_is_unknown(self):
        with self.assertRaises(ValueError):
            certification.listed('<html>temporarily unavailable</html>', {'brand': 'StepX', 'manufacturer': 'StepX', 'model': 'PZ261A'})
        with self.assertRaises(ValueError):
            certification.listed(self.table([['Other','Phone','pandora','123']]).replace('</table>',''), {'brand':'StepX','manufacturer':'StepX','model':'PZ261A'})


if __name__ == '__main__':
    unittest.main()
