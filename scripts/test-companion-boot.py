"""Exercise real boot-adapter filesystem transactions in an isolated temporary root."""
import ast,base64,contextlib,hashlib,io,json,tempfile,unittest
from pathlib import Path
SOURCE=Path(__file__).with_name('companion-boot.py').read_text()
TREE=ast.parse(SOURCE)
def hashof(data):return hashlib.sha256(data).hexdigest()
def install_code():
    function=next(x for x in TREE.body if isinstance(x,ast.FunctionDef) and x.name=='install')
    return next(x.value.value for x in function.body if isinstance(x,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='code' for t in x.targets))
def remove_code():
    function=next(x for x in TREE.body if isinstance(x,ast.FunctionDef) and x.name=='remove')
    return next(x.value.args[0].value for x in function.body if isinstance(x,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='proof' for t in x.targets))
class BootFilesystemTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.root=Path(self.temp.name)
        self.base=self.root/'usr/local/lib/stepx-companion';self.hook=self.root/'usr/local/lib/stepx-gms/boot.py'
        self.hook.parent.mkdir(parents=True);self.original=b'# existing factory adapter\n';self.hook.write_bytes(self.original)
        self.chain=b'# companion chain\n'
        self.config={'signer':'a'*64,'uid':10215,'installed_boot':'old','chain_sha256':hashof(self.chain),'original_sha256':hashof(self.original)}
        self.data={name:base64.b64encode(data).decode() for name,data in {'chain.py':self.chain,'grant.py':b'# grant','boot.py':b'# boot','config.json':json.dumps(self.config).encode()}.items()}
    def tearDown(self):self.temp.cleanup()
    def run_code(self,code,**extra):
        code=code.replace("'/usr/local/lib/",repr(str(self.root/'usr/local/lib')+'/')[:-1]).replace("'/var/lib/",repr(str(self.root/'var/lib')+'/')[:-1])
        with contextlib.redirect_stdout(io.StringIO()):exec(code,{'ORIGINAL':hashof(self.original),'CONFIG':dict(self.config),'DATA':dict(self.data),**extra})
    def install(self):self.run_code(install_code())
    def test_install_keeps_exact_backup_and_can_restore(self):
        self.install();self.assertEqual((self.base/'previous-boot.py').read_bytes(),self.original)
        self.assertEqual(self.hook.read_bytes(),self.chain)
        self.run_code(remove_code());self.assertEqual(self.hook.read_bytes(),self.original)
    def test_foreign_hook_is_never_overwritten(self):
        self.install();self.hook.write_bytes(b'# another administrator')
        with self.assertRaisesRegex(RuntimeError,'Unrecognized boot hook'):self.install()
        self.assertEqual(self.hook.read_bytes(),b'# another administrator')
    def test_changed_backup_blocks_restore(self):
        self.install();(self.base/'previous-boot.py').write_bytes(b'# modified')
        with self.assertRaisesRegex(RuntimeError,'refusing to overwrite'):self.run_code(remove_code())
        self.assertEqual(self.hook.read_bytes(),self.chain)
    def test_companion_identity_change_blocks_upgrade(self):
        self.install();self.config['uid']=10216
        before=(self.base/'config.json').read_bytes()
        with self.assertRaisesRegex(RuntimeError,'Boot identity changed'):self.install()
        self.assertEqual((self.base/'config.json').read_bytes(),before)
    def test_reinstall_preserves_first_install_boot_and_original(self):
        self.install();self.config['installed_boot']='new';self.install()
        self.assertEqual(json.loads((self.base/'config.json').read_text())['installed_boot'],'old')
        self.assertEqual((self.base/'previous-boot.py').read_bytes(),self.original)
    def test_symlink_hook_target_is_not_written(self):
        self.hook.unlink();other=self.root/'other';other.write_bytes(self.original);self.hook.symlink_to(other)
        with self.assertRaisesRegex(RuntimeError,'symlink'):self.install()
        self.assertEqual(other.read_bytes(),self.original)

    def test_current_permissions_must_survive_package_update(self):
        function=next(x for x in TREE.body if isinstance(x,ast.FunctionDef) and x.name=='current_permissions_ready')
        scope={};exec(compile(ast.Module(body=[function],type_ignores=[]),'current-permissions','exec'),scope)
        ready=scope['current_permissions_ready']
        original={name:True for name in ('installed','signature_verified','granted','connection_permissions_granted','wake_permissions_granted')}
        self.assertTrue(ready(original))
        for key in original:
            for value in (False,None,'true'):
                self.assertFalse(ready({**original,key:value}),key)
            self.assertFalse(ready({k:v for k,v in original.items() if k!=key}),key)
if __name__=='__main__':unittest.main()
