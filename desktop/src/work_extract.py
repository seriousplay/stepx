# Fixed Office text reader; never runs document macros or embedded scripts.
import sys, zipfile, posixpath, xml.etree.ElementTree as ET
p = sys.argv[1]
MAX_XML = 8 * 1024 * 1024
MAX_TEXT = 65536
parts = []
size = 0

def append(value):
    global size
    if not value or size >= MAX_TEXT:
        return
    value = str(value)
    b = value.encode('utf-8')
    available = MAX_TEXT - size
    if len(b) > available:
        value = b[:available].decode('utf-8', errors='ignore')
    parts.append(value)
    size += len(value.encode('utf-8')) + 1

def tree(z, name):
    info = z.getinfo(name)
    if info.file_size > MAX_XML:
        raise ValueError('文档 XML 过大')
    data = z.read(name)
    if b'<!DOCTYPE' in data or b'<!ENTITY' in data:
        raise ValueError('不支持带外部实体的文档')
    return ET.fromstring(data)

with zipfile.ZipFile(p) as z:
    names = z.namelist()
    if len(names) > 10000 or len(set(names)) != len(names) or sum(i.file_size for i in z.infolist()) > 64 * 1024 * 1024:
        raise ValueError('文档内部文件过多或解压大小超过 64 MB')
    if 'word/document.xml' in names:
        ns = {'w':'http://schemas.openxmlformats.org/wordprocessingml/2006/main'}
        root = tree(z, 'word/document.xml')
        for paragraph in root.findall('.//w:p', ns):
            append(''.join(t.text or '' for t in paragraph.findall('.//w:t', ns)))
    elif 'xl/workbook.xml' in names:
        ns = {'s':'http://schemas.openxmlformats.org/spreadsheetml/2006/main'}
        shared = []
        if 'xl/sharedStrings.xml' in names:
            for item in tree(z, 'xl/sharedStrings.xml').findall('s:si', ns):
                shared.append(''.join(t.text or '' for t in item.findall('.//s:t', ns)))
        rel = {'r': 'http://schemas.openxmlformats.org/package/2006/relationships'}
        relationships = {n.get('Id'):n.get('Target') for n in tree(z, 'xl/_rels/workbook.xml.rels').findall('r:Relationship', rel) if n.get('TargetMode') != 'External'}
        sheets = []
        for sheet in tree(z, 'xl/workbook.xml').findall('s:sheets/s:sheet', ns):
            target = relationships.get(sheet.get('{http://schemas.openxmlformats.org/officeDocument/2006/relationships}id'))
            if target:
                name = posixpath.normpath('xl/' + target) if not target.startswith('/') else target.lstrip('/')
                if name.startswith('xl/worksheets/') and name in names: sheets.append((sheet.get('name',name),name))
        for label, name in sheets[:30]:
            append('工作表: ' + label)
            for row in tree(z, name).findall('.//s:row', ns):
                values = []
                for cell in row.findall('s:c', ns):
                    value = cell.findtext('s:v', default='', namespaces=ns)
                    typ = cell.get('t')
                    if typ == 's':
                        try: value = shared[int(value)]
                        except (ValueError, IndexError): value = '[无效共享字符串]'
                    elif typ == 'inlineStr':
                        value = ''.join(t.text or '' for t in cell.findall('.//s:t', ns))
                    formula = cell.findtext('s:f', default='', namespaces=ns)
                    if formula: value = f'={formula} → {value if value != "" else "[尚无计算缓存]"}'
                    values.append(f'{cell.get("r", "")}: {value}')
                append(' | '.join(values))
                if size >= MAX_TEXT: break
            if size >= MAX_TEXT: break
    elif 'ppt/presentation.xml' in names:
        ns = {'a':'http://schemas.openxmlformats.org/drawingml/2006/main'}
        rel = {'r': 'http://schemas.openxmlformats.org/package/2006/relationships'}
        relationships = {n.get('Id'):n.get('Target') for n in tree(z, 'ppt/_rels/presentation.xml.rels').findall('r:Relationship', rel) if n.get('TargetMode') != 'External'}
        slides = []
        pns = {'p':'http://schemas.openxmlformats.org/presentationml/2006/main'}
        for slide in tree(z, 'ppt/presentation.xml').findall('p:sldIdLst/p:sldId', pns):
            target = relationships.get(slide.get('{http://schemas.openxmlformats.org/officeDocument/2006/relationships}id'))
            if target:
                name = posixpath.normpath('ppt/' + target) if not target.startswith('/') else target.lstrip('/')
                if name.startswith('ppt/slides/') and name in names: slides.append(name)
        for index, name in enumerate(slides[:100],1):
            append('幻灯片: ' + str(index))
            for node in tree(z, name).findall('.//a:t', ns): append(node.text or '')
            if size >= MAX_TEXT: break
    else:
        raise ValueError('未识别的 Office 文档')
if size >= MAX_TEXT:
    parts.append('[达到 64 KB 阅读上限，后续内容未读取]')
result = '\n'.join(parts)
if not result.strip():
    raise ValueError('文档没有可提取的文字')
sys.stdout.write(result)
