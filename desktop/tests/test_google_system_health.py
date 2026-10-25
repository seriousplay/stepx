"""Regression coverage for stale success, delayed restarts and boot exclusion."""
import ast
import importlib.util
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'handoff/google-play-stepx/tools'))
spec = importlib.util.spec_from_file_location('google_health', ROOT / 'handoff/google-play-stepx/tools/health.py')
health = importlib.util.module_from_spec(spec); spec.loader.exec_module(health)


class HealthTests(unittest.TestCase):
    def test_receipt_is_bound_to_process_boot_and_both_packages(self):
        versions = {'com.google.android.gms': 100, 'com.android.vending': 200}
        good = dict(status='ready', registered=True, update_monitor=True, boot_id='boot',
                    system_server_pid=30, gms_version=100, play_version=200)
        self.assertTrue(health.current_login(good, 'boot', 30, versions))
        for key, value in [('boot_id','old'),('system_server_pid',29),('gms_version',99),
                           ('play_version',199),('registered',False),('update_monitor',False),('status','needs_repair')]:
            self.assertFalse(health.current_login({**good,key:value},'boot',30,versions),key)
        for key in good:
            self.assertFalse(health.current_login({k:v for k,v in good.items() if k!=key},'boot',30,versions),key)

    def test_delayed_system_restart_is_not_success(self):
        tree=ast.parse((ROOT/'scripts/companion-permissions.py').read_text())
        fn=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='verify_system_server_stable')
        from unittest.mock import Mock
        clock=Mock();clock.monotonic.side_effect=[0,0,1];clock.sleep=Mock()
        shell=Mock(side_effect=['30','30','31'])
        scope={'time':clock,'shell':shell}
        exec(compile(ast.Module(body=[fn],type_ignores=[]),'stability','exec'),scope)
        self.assertFalse(scope[fn.name](30,20))
        self.assertEqual(shell.call_count,3)

    def test_both_worker_templates_parse_and_lock_before_factory_exec(self):
        import importlib.util
        sys.path.insert(0,str(ROOT/'scripts'))
        spec=importlib.util.spec_from_file_location('companion_permissions',ROOT/'scripts/companion-permissions.py')
        cp=importlib.util.module_from_spec(spec);spec.loader.exec_module(cp)
        raw=(ROOT/'handoff/google-play-stepx/tools/root_worker.py').read_text()
        for source in [raw, cp.agentbus_worker_source(raw)]:
            tree=ast.parse(source)
            assign=next(n for n in tree.body if isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='src' for t in n.targets))
            value=assign.value
            while isinstance(value,ast.Call):value=value.func.value
            template=value.value
            ast.parse(template.replace('CODE','"pass"').replace('RESULT','"/tmp/result"'))
            self.assertLess(template.index("fcntl.flock(fd"),template.index('child=os.fork()'))
            self.assertIn("'/var/lib/stepx-gms/lock'",template)
            self.assertIn("'/var/lib/stepx-companion/lock'",template)
            self.assertIn('if lock_error:raise RuntimeError(lock_error)',template)


if __name__=='__main__':unittest.main()
