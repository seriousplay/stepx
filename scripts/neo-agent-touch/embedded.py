"""Original build-time assembly; never vendors upstream files into the repository."""
import copy
from pathlib import Path
import re
import shutil
import xml.etree.ElementTree as ET

ANDROID = '{http://schemas.android.com/apk/res/android}'
HOST = 'com.stepx.companion.app'


def merge_manifest(host, neo, neo_package):
    root = copy.deepcopy(host.getroot())
    if root.get('package') != HOST:
        raise RuntimeError('Unexpected companion package')
    app = root.find('application')
    neo_app = neo.getroot().find('application')
    if app is None or neo_app is None:
        raise RuntimeError('Missing application manifest')
    queries = root.find('queries')
    if queries is None:
        queries = ET.SubElement(root, 'queries')
    existing_queries = {ET.tostring(x) for x in queries}
    for query in neo.getroot().find('queries') or []:
        if ET.tostring(query) not in existing_queries:
            queries.append(copy.deepcopy(query))
    permissions = {x.get(ANDROID+'name') for x in root.findall('uses-permission')}
    for permission in neo.getroot().findall('uses-permission'):
        name = permission.get(ANDROID+'name')
        if name not in permissions:
            if name != 'android.permission.INTERNET':
                raise RuntimeError('Unexpected new embedded permission: '+str(name))
            root.insert(0, copy.deepcopy(permission))
    for node in neo_app:
        child = copy.deepcopy(node)
        name = child.get(ANDROID+'name', '')
        if name.startswith('.'):
            name = neo_package + name
        elif '.' not in name:
            name = neo_package + '.' + name
        child.set(ANDROID+'name', name)
        if child.tag == 'activity':
            child.set(ANDROID+'theme', neo_app.get(ANDROID+'theme'))
            child.set(ANDROID+'label', 'StepX伴侣 · Neo Agent Touch')
            child.set(ANDROID+'exported', 'false')
            for intent in list(child.findall('intent-filter')):
                if any(c.get(ANDROID+'name') == 'android.intent.category.LAUNCHER' for c in intent.findall('category')):
                    child.remove(intent)
        if child.tag == 'provider':
            suffix = '.control' if name.endswith('.CompanionBridgeProvider') else '.updates'
            child.set(ANDROID+'authorities', HOST+'.neo'+suffix)
        if name.endswith('.ControlService'):
            child.set(ANDROID+'label', 'StepX伴侣 · Neo 看屏操作')
        if name.endswith('.AgentInputMethodService'):
            child.set(ANDROID+'label', 'StepX伴侣 · Neo 输入')
        app.append(child)
    # Companion's package/version/signature and its sole launch icon remain intact.
    launcher = [x for x in app.findall('activity') for intent in x.findall('intent-filter')
                if any(c.get(ANDROID+'name') == 'android.intent.category.LAUNCHER' for c in intent.findall('category'))]
    if len(launcher) != 1:
        raise RuntimeError('Embedded APK must have exactly one launcher')
    names = [x.get(ANDROID+'name') for x in app if x.tag in ('activity','service','provider','receiver')]
    if len(set(names)) != len(names):
        raise RuntimeError('Duplicate Android component')
    authorities = [x.get(ANDROID+'authorities') for x in app.findall('provider')]
    if len(set(authorities)) != len(authorities) or neo_package+'.control' in authorities:
        raise RuntimeError('Provider authority collision')
    return ET.ElementTree(root)


def assemble(stage, companion, neo_package):
    """Create a private, self-contained snapshot that can be compiled as one APK."""
    resources = stage/'res'
    symbols = {}
    for path in resources.rglob('*'):
        if not path.is_file():
            continue
        kind = path.parent.name.split('-')[0]
        if kind == 'values':
            document = ET.parse(path)
            for node in document.getroot():
                if node.get('name'):
                    symbols[(node.tag, node.get('name'))] = 'neo_'+node.get('name')
        else:
            symbols[(kind, path.stem)] = 'neo_'+path.stem
    def refs(text):
        for (kind, name), renamed in symbols.items():
            text = re.sub(r'@'+re.escape(kind+'/'+name)+r'(?![\w.])', '@'+kind+'/'+renamed, text)
            text = re.sub(r'(?<![\w.])R\.'+re.escape(kind+'.'+name)+r'(?!\w)', 'R.'+kind+'.'+renamed, text)
        return text
    for path in list(resources.rglob('*')):
        if not path.is_file():
            continue
        kind = path.parent.name.split('-')[0]
        if path.suffix == '.xml':
            document = ET.parse(path)
            if kind == 'values':
                for node in document.getroot():
                    if node.get('name'):
                        node.set('name', symbols[(node.tag, node.get('name'))])
            document.write(path, encoding='utf-8', xml_declaration=True)
            text = refs(path.read_text('utf-8'))
            text = text.replace(neo_package+'.MainActivity', HOST+'.NeoSettingsActivity')
            path.write_text(text, 'utf-8')
        if kind != 'values':
            path.rename(path.with_name('neo_'+path.name))
    manifest_path = stage/'AndroidManifest.xml'
    manifest_path.write_text(refs(manifest_path.read_text('utf-8')), 'utf-8')
    # Assets and mutable state belong to Neo without sharing generic names with
    # Studio, file transfer, pairing, or existing Companion preferences.
    for base in ('src','release-src'):
        for path in (stage/base).rglob('*.java'):
            text = refs(path.read_text('utf-8'))
            text = text.replace('file:///android_asset/', 'file:///android_asset/neo/')
            text = text.replace('getAssets().open("agent-skills/', 'getAssets().open("neo/agent-skills/')
            text = re.sub(r'getSharedPreferences\("([^"\n]+)"', lambda m:'getSharedPreferences("neo-'+m.group(1)+'"', text)
            for name in ('last-run.json','action-failure-audit.json','qa-last-search-proposal.json',
                         'qa-decision-original.jpg','qa-decision-annotated.jpg','qa-last-decision.json',
                         'qa-last-ask.json','qa-ask-original.jpg','verified-update','verified-app-update','agent-ime-lease-v1'):
                text = text.replace('"'+name+'"', '"neo-'+name+'"')
            if path.name == 'AppUpdateStore.java':
                text = text.replace('c.getPackageName()+".updates/verified/"', '"'+HOST+'.neo.updates/verified/"')
            if path.name == 'UpdateRules.java':
                text = text.replace('PACKAGE="'+neo_package+'"', 'PACKAGE="'+HOST+'"')
            path.write_text(text, 'utf-8')
    old_assets = stage/'assets'
    old_assets.rename(stage/'neo-assets')
    old_assets.mkdir()
    shutil.move(stage/'neo-assets', old_assets/'neo')
    if (companion/'assets').is_dir():
        for path in (companion/'assets').iterdir():
            if (old_assets/path.name).exists():
                raise RuntimeError('Companion asset collides with Neo namespace')
            if path.is_dir():
                shutil.copytree(path, old_assets/path.name)
            else:
                shutil.copy2(path, old_assets/path.name)
    for path in (companion/'res').rglob('*'):
        if not path.is_file():
            continue
        target = resources/path.relative_to(companion/'res')
        if target.exists():
            raise RuntimeError('Resource filename collision: '+str(target.relative_to(resources)))
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, target)
    shutil.copytree(companion/'src', stage/'companion-src')
    shutil.copytree(companion/'tests', stage/'companion-tests')
    shutil.copytree(companion/'libs', stage/'companion-libs')
    merged = merge_manifest(ET.parse(companion/'AndroidManifest.xml'), ET.parse(manifest_path), neo_package)
    settings = [x for x in merged.getroot().find('application').findall('activity')
                if x.get(ANDROID+'name') in ('.NeoSettingsActivity',HOST+'.NeoSettingsActivity')]
    if len(settings) != 1 or settings[0].get(ANDROID+'exported') != 'true' or not (stage/'companion-src/com/stepx/companion/app/NeoSettingsActivity.java').is_file():
        raise RuntimeError('Embedded build requires the companion-owned Neo settings page')
    merged.write(manifest_path, encoding='utf-8', xml_declaration=True)
    return HOST
