#!/usr/bin/env python3
"""Opt-in live regression check; mutates only its uniquely named test directory."""
import argparse
import gzip
import json
import os
from pathlib import Path
import shutil
import shlex
import subprocess
import tempfile
import uuid

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("serial")
args = parser.parse_args()
adb = [os.environ.get("STEPX_ADB", shutil.which("adb") or "adb"), "-s", args.serial]


def run(*arguments, data=None):
    if arguments[0] == "shell" and len(arguments) > 2:
        arguments = ("shell", shlex.join(str(a) for a in arguments[1:]))
    return subprocess.run(adb + list(arguments), input=data, capture_output=True, check=True, timeout=70).stdout


assert run("shell", "getprop", "ro.serialno").decode().strip() == args.serial
assert run("shell", "getprop", "ro.product.device").decode().strip() == "pandora"
root = Path(__file__).resolve().parents[1]
run("shell", "mkdir", "-p", "/data/local/tmp/stepx-companion")
run("push", str(root / "desktop/assets/stepx-bridge.jar"), "/data/local/tmp/stepx-companion/stepx-bridge.jar")


def bridge(request):
    text = run("shell", "CLASSPATH=/data/local/tmp/stepx-companion/stepx-bridge.jar app_process /data/local/tmp/stepx-companion com.stepx.companion.Bridge",
               data=(json.dumps(request) + "\n").encode()).decode()
    return json.loads(next(line.removeprefix("STEPX_RESULT:") for line in text.splitlines() if line.startswith("STEPX_RESULT:")))


remote = "/sdcard/Documents/StepX-tools-check-" + uuid.uuid4().hex
run("shell", "mkdir", "-p", remote)
local = tempfile.TemporaryDirectory()
checks = []


def push(name, content):
    path = Path(local.name) / name
    path.write_bytes(content)
    run("push", str(path), remote + "/" + name)


def info(name):
    report = bridge({"op": "storage_scan"})
    assert report["ok"], report.get("error")
    return next(f for f in report["documents"] if f["path"] == remote + "/" + name)


def operation(action, item, **extra):
    return bridge({"op": "storage_file_operation", "action": action, **item, **extra})


def exists(name):
    return subprocess.run(adb + ["shell", shlex.join(["test", "-f", remote + "/" + name])], capture_output=True).returncode == 0


try:
    content = ("手机空间释放回归测试\n" * 60000).encode()
    push("source.txt", content)
    push("copy.txt", content)
    push("different.txt", b"x" * len(content))
    a, b, c = info("source.txt"), info("copy.txt"), info("different.txt")
    report = bridge({"op": "storage_scan"})
    assert any({a["path"], b["path"]}.issubset({f["path"] for f in group}) for group in report["duplicates"])
    checks.append("content-based duplicate detection")
    denied = operation("delete_duplicate", b, keep=c["path"])
    assert not denied["ok"] and exists("copy.txt") and exists("different.txt")
    checks.append("same-size different-content deletion blocked")
    stale = {**a, "bytes": a["bytes"] + 1}
    assert not operation("compress", stale)["ok"] and exists("source.txt")
    checks.append("stale scan rejected")
    result = operation("delete_duplicate", b, keep=a["path"])
    assert result["ok"] and not exists("copy.txt") and exists("source.txt")
    checks.append("duplicate deletion keeps confirmed original")
    result = operation("compress", a)
    assert result["ok"], result.get("error")
    assert result["logical_saved"] > 0 and exists("source.txt.gz") and not exists("source.txt")
    delta = result["available_after"] - result["available_before"]
    assert delta > 0, "Phone free space did not increase; inspect concurrent storage activity"
    compressed = Path(local.name) / "compressed.gz"
    run("pull", remote + "/source.txt.gz", str(compressed))
    assert gzip.decompress(compressed.read_bytes()) == content
    checks.append("on-phone compression replaces source after validation")
    result = operation("restore", info("source.txt.gz"))
    assert result["ok"] and exists("source.txt") and not exists("source.txt.gz")
    restored = Path(local.name) / "restored.txt"
    run("pull", remote + "/source.txt", str(restored))
    assert restored.read_bytes() == content
    checks.append("on-phone restoration is byte-exact")
    push("source.txt.gz", b"existing file")
    assert not operation("compress", info("source.txt"))["ok"] and exists("source.txt")
    checks.append("existing destination never overwritten")
    push("broken.txt.gz", b"broken gzip")
    assert not operation("restore", info("broken.txt.gz"))["ok"] and exists("broken.txt.gz")
    checks.append("invalid archive retained")
    push("empty.txt", b"")
    assert not operation("compress", info("empty.txt"))["ok"] and exists("empty.txt")
    checks.append("no-benefit compression leaves source intact")
    result = operation("delete_file", info("different.txt"))
    assert result["ok"] and not exists("different.txt")
    checks.append("explicit file deletion frees phone storage")
    print(json.dumps({"checks": checks, "compression_available_delta_bytes": delta,
                      "compressed_bytes": compressed.stat().st_size, "original_bytes": len(content)}, ensure_ascii=False, indent=2))
finally:
    # Only remove the test directory created by this invocation; no user file is selected.
    run("shell", "rm", "-rf", remote)
    local.cleanup()
