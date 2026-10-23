import base64, contextlib, hashlib, http.server, importlib.util, io, json, pathlib, tempfile, threading, unittest, zipfile
P=pathlib.Path(__file__).resolve().parents[1]/'src/ota_transfer.py'
spec=importlib.util.spec_from_file_location('transfer',P); ota=importlib.util.module_from_spec(spec);spec.loader.exec_module(ota)
def package(path,extra=None):
    payload=b'CrAU'+bytes(range(256))*256
    with zipfile.ZipFile(path,'w') as z:
        z.writestr('META-INF/com/android/metadata','ota-type=AB\npost-aios-version=Test AOS\npre-device=pandora\npost-build=vendor/pandora/test\npost-build-incremental=173\npost-timestamp=200\n')
        z.writestr('payload.bin',payload)
        z.writestr('payload_properties.txt','FILE_SIZE=%s\nFILE_HASH=%s\n'%(len(payload),base64.b64encode(hashlib.sha256(payload).digest()).decode()))
        z.writestr('META-INF/com/android/otacert','fixture certificate, phone signature verification remains mandatory')
        if extra:z.writestr(*extra)
    data=path.read_bytes()
    return {'version':'Test AOS','size':len(data),'sha256':hashlib.sha256(data).hexdigest(),'md5':hashlib.md5(data).hexdigest()}
class Tests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory();self.root=pathlib.Path(self.tmp.name);self.path=self.root/'source.zip';self.req=package(self.path);self.output=io.StringIO();self.quiet=contextlib.redirect_stdout(self.output);self.quiet.__enter__()
    def tearDown(self): self.quiet.__exit__(None,None,None);self.tmp.cleanup()
    def test_hash_and_metadata(self):
        result=ota.verify(self.req,self.path);self.assertEqual(result['metadata']['pre-device'],'pandora');self.assertEqual(result['sha256'],self.req['sha256'])
        with self.assertRaisesRegex(ValueError,'目标版本'):ota.verify(dict(self.req,version='wrong'),self.path)
    def test_tampered_truncated_and_oversize(self):
        data=self.path.read_bytes();self.path.write_bytes(data[:-1]);
        with self.assertRaisesRegex(ValueError,'大小'):ota.verify(self.req,self.path)
        self.path.write_bytes(data[:-1]+bytes([data[-1]^1]))
        with self.assertRaisesRegex(ValueError,'校验失败'):ota.verify(self.req,self.path)
        self.path.write_bytes(data+b'x')
        with self.assertRaisesRegex(ValueError,'大小'):ota.verify(self.req,self.path)
    def test_zip_path_traversal(self):
        req=package(self.path,('../escape','bad'))
        with self.assertRaisesRegex(ValueError,'不安全路径'):ota.verify(req,self.path)
    def test_link_and_cancellation(self):
        link=self.root/'link';link.symlink_to(self.path)
        with self.assertRaisesRegex(ValueError,'普通文件'):ota.verify(self.req,link)
        cancel=self.root/'cancel';cancel.touch()
        with self.assertRaises(ota.Cancelled):ota.verify(dict(self.req,cancel=str(cancel)),self.path)
    def test_reject_unsafe_url_digest_and_size(self):
        for url in ['http://example.org/a','https://u:p@example.org/a','https://x/a#f','https://x/\\file','https://x/\n']:
            with self.assertRaises(ValueError):ota.validate_url(url)
        for req in [{},dict(self.req,size=ota.MAX_SIZE+1),dict(self.req,sha256='evil')]:
            with self.assertRaises(ValueError):ota.contract(req)
    def test_stage_progress_and_cancelled_phone_hash_cleanup(self):
        fake=self.root/'fake adb'
        fake.write_text("""#!/usr/bin/env python3
import sys,pathlib,shlex,shutil,hashlib,time
root=pathlib.Path(ROOT)
a=sys.argv[3:]
def path(v):return root/v.lstrip('/')
if a[0]=='push':
 p=path(a[2]);p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(pathlib.Path(a[1]).read_bytes());time.sleep(1.2)
else:
 a=shlex.split(a[1])
 if a[0]=='getprop':print('test-device')
 elif a[0]=='df':print('Filesystem 1K-blocks Used Available Use% Mounted on\\n/fake 999999999 0 999999999 0% /sdcard')
 elif a[0]=='mkdir':path(a[-1]).mkdir(parents=True,exist_ok=True)
 elif a[0]=='sh':
  p=path(shlex.split(a[2])[3]);print(p.stat().st_size if p.exists() else 0)
 elif a[0]=='sha256sum':print(hashlib.sha256(path(a[1]).read_bytes()).hexdigest(),a[1])
 elif a[0]=='mv':path(a[1]).rename(path(a[2]))
 elif a[0]=='rm':path(a[-1]).unlink(missing_ok=True)
 elif a[0]=='rmdir':
  try:path(a[1]).rmdir()
  except OSError:sys.exit(1)
""".replace('ROOT',repr(str(self.root/'phone'))))
        fake.chmod(0o700)
        remote='/sdcard/Download/StepX-OTA/'+'a'*32+'/update.zip'
        cancel=self.root/'cancel-stage'
        req=dict(self.req,path=str(self.path),adb=str(fake),transport='test',serial='test-device',remote=remote,cancel=str(cancel))
        result=ota.stage(req);self.assertEqual(result['remote'],remote)
        events=[json.loads(v) for v in self.output.getvalue().splitlines()]
        self.assertTrue(any(e.get('phase')=='transfer' and e['done']>0 for e in events))
        (self.root/'phone'/remote.lstrip('/')).unlink()
        original=ota.emit
        def cancelling(event,**kw):
            original(event,**kw)
            if kw.get('phase')=='phone_verify':cancel.touch()
        ota.emit=cancelling
        try:
            with self.assertRaises(ota.Cancelled):ota.stage(req)
        finally:ota.emit=original
        self.assertFalse((self.root/'phone'/(remote+'.part').lstrip('/')).exists())
    def test_real_http_range_resume_and_validation(self):
        data=self.path.read_bytes();requests=[]
        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self,*a):pass
            def do_GET(self):
                start=int(self.headers.get('Range','bytes=0-').split('=')[1].split('-')[0]);requests.append(start)
                if self.path=='/redirect':
                    self.send_response(302);self.send_header('Location','http://example.org/forbidden');self.end_headers();return
                self.send_response(206 if start else 200)
                if start:self.send_header('Content-Range','bytes %s-%s/%s'%(start,len(data)-1,len(data)))
                self.send_header('Content-Length',str(len(data)-start));self.end_headers();self.wfile.write(data[start:])
        server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Handler);thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
        try:
            origin='127.0.0.1:%s'%server.server_port;cache=self.root/'cache';cache.mkdir();(cache/(self.req['sha256']+'.part')).write_bytes(data[:100])
            req=dict(self.req,url='http://'+origin+'/firmware',cache=str(cache));result=ota.download(req,origin)
            self.assertEqual(requests,[100]);self.assertEqual(pathlib.Path(result['path']).read_bytes(),data)
            self.assertFalse((cache/(self.req['sha256']+'.part')).exists())
            # A corrupted application cache must recover, without touching external files.
            pathlib.Path(result['path']).write_bytes(b'broken')
            result=ota.download(req,origin)
            self.assertEqual(requests,[100,0]);self.assertEqual(self.path.read_bytes(),data)
            pathlib.Path(result['path']).unlink();req['url']='http://'+origin+'/redirect'
            with self.assertRaisesRegex(ValueError,'HTTPS'):ota.download(req,origin)
        finally:server.shutdown();server.server_close();thread.join()
if __name__=='__main__':unittest.main()
