"""Fixed phone-side skill installer/exporter. Never executes skill code.

Uses ClawHub's current ownerHandle API, since old phone installers only accept
globally unique bare slugs. Receives a JSON argument; stdout is structured proof.
"""
import hashlib
import io
import json
import os
import pathlib
import re
import shutil
import stat
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request
import zipfile

MAX_BYTES = 8 * 1024 * 1024
MAX_TEXT = 1024 * 1024
MAX_FILES = 200
BASE = "https://clawhub.ai"
TEXT_SUFFIXES = {".md", ".txt", ".json", ".yaml", ".yml", ".csv"}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def component(value):
    require(isinstance(value, str) and re.fullmatch(r"[a-zA-Z0-9][a-zA-Z0-9_.-]{0,159}", value), "技能标识无效")
    require(value not in (".", ".."), "技能标识无效")
    return value


def request(path, query, limit=MAX_BYTES):
    url = BASE + path + "?" + urllib.parse.urlencode(query)
    req = urllib.request.Request(url, headers={"User-Agent": "StepX-Companion/0.1"})
    try:
        with urllib.request.urlopen(req, timeout=12) as response:
            require(urllib.parse.urlparse(response.url).scheme == "https", "技能下载必须使用 HTTPS")
            data = response.read(limit + 1)
            require(len(data) <= limit, "技能包超过大小上限")
            return data
    except urllib.error.HTTPError as error:
        if error.code == 429:
            retry = error.headers.get("Retry-After", "稍后")
            raise ValueError("技能市场限流，请等待 " + retry + " 后重试") from None
        raise ValueError("技能市场请求失败 HTTP " + str(error.code)) from None


def safe_path(value):
    require(isinstance(value, str) and value and "\\" not in value and "\x00" not in value, "技能包路径无效")
    path = pathlib.PurePosixPath(value)
    require(not path.is_absolute() and all(x not in (".", "..") for x in value.split("/")), "技能包包含越界路径")
    return path


def install(args):
    owner, slug = component(args["owner"]), component(args["slug"])
    query = {"ownerHandle": owner}
    detail = json.loads(request("/api/v1/skills/" + slug, query, MAX_TEXT))
    require(detail.get("owner", {}).get("handle", "").lower() == owner.lower(), "技能发布者与所选条目不一致")
    require(detail.get("skill", {}).get("slug") == slug, "技能标识与所选条目不一致")
    moderation = detail.get("moderation") or {}
    require(not moderation.get("isMalwareBlocked") and not moderation.get("isSuspicious"), "此技能被市场标记，不自动安装")
    version = detail.get("latestVersion", {}).get("version")
    require(isinstance(version, str) and re.fullmatch(r"[a-zA-Z0-9_.+-]{1,100}", version), "市场未确认可安装版本")
    metadata = json.loads(request("/api/v1/skills/" + slug + "/versions/" + urllib.parse.quote(version, safe=""), query, MAX_TEXT))
    release = metadata.get("version", {})
    require(release.get("version") == version and metadata.get("skill", {}).get("slug") == slug, "技能版本不一致")
    security = release.get("security") or {}
    require(security.get("status") not in ("malicious", "suspicious", "blocked"), "此版本未通过市场安全检查")
    manifest = release.get("files", [])
    require(isinstance(manifest, list) and 0 < len(manifest) <= MAX_FILES, "技能文件清单无效或过大")
    expected = {}
    for entry in manifest:
        name = str(safe_path(entry.get("path")))
        require(name not in expected and name != "_meta.json", "技能清单路径重复")
        digest = entry.get("sha256", "")
        require(re.fullmatch(r"[a-fA-F0-9]{64}", digest), "技能清单缺少文件校验")
        size = entry.get("size")
        require(isinstance(size, int) and 0 <= size <= MAX_BYTES, "技能文件大小无效")
        expected[name] = (digest.lower(), size)
    require(sum(size for _, size in expected.values()) <= MAX_BYTES, "技能解压后超过大小上限")
    require("SKILL.md" in expected, "技能缺少 SKILL.md")
    data = request("/api/v1/download", {"ownerHandle": owner, "slug": slug, "version": version})
    archive = zipfile.ZipFile(io.BytesIO(data))
    require(len(archive.infolist()) <= MAX_FILES + 1, "技能压缩包条目过多")
    contents = {}
    meta = None
    for item in archive.infolist():
        name = str(safe_path(item.filename))
        require(name not in contents and not item.is_dir(), "技能压缩包路径重复或无效")
        require(not stat.S_ISLNK(item.external_attr >> 16), "技能压缩包包含符号链接")
        require(item.file_size <= MAX_BYTES and not (item.flag_bits & 1), "技能压缩包不受支持")
        raw = archive.read(item)
        if name == "_meta.json":
            require(meta is None, "技能来源记录重复")
            meta = json.loads(raw)
            continue
        require(name in expected, "技能包包含清单外文件")
        digest, size = expected[name]
        require(len(raw) == size and hashlib.sha256(raw).hexdigest() == digest, "技能文件完整性校验失败")
        contents[name] = raw
    require(set(contents) == set(expected), "技能压缩包缺少清单文件")
    require(meta and meta.get("slug") == slug and meta.get("version") == version, "技能来源记录不一致")
    # Plain instruction content is validated before installing, but never executed.
    skill_text = contents["SKILL.md"].decode("utf-8")
    require(len(contents["SKILL.md"]) <= MAX_TEXT and skill_text.strip(), "技能说明为空或过大")
    match = re.search(r"(?m)^name:\s*(.+)$", skill_text)
    skill_name = match.group(1).strip().strip("\"'") if match else slug
    require(skill_name not in args.get("existingNames", []), "手机已存在同名技能，不会覆盖")
    workspace = pathlib.Path(args["workspace"])
    require(workspace.is_absolute() and str(workspace.resolve()).startswith("/root/.openclaw/"), "手机技能工作目录异常")
    require(not workspace.is_symlink(), "手机技能工作目录不能为符号链接")
    root = workspace / "skills"
    root.mkdir(exist_ok=True)
    require(not root.is_symlink() and root.resolve().parent == workspace.resolve(), "手机技能目录越界")
    target = root / (owner.lower() + "--" + slug)
    require(not target.exists() and not target.is_symlink(), "此发布者的技能已安装，不会覆盖")
    stage = pathlib.Path(tempfile.mkdtemp(prefix=".stepx-install-", dir=root))
    try:
        for name, raw in contents.items():
            file = stage.joinpath(*safe_path(name).parts)
            file.parent.mkdir(parents=True, exist_ok=True)
            with file.open("xb") as out:
                out.write(raw)
            file.chmod(0o644)
        provenance = {"version": 1, "registry": BASE, "ownerHandle": owner, "slug": slug,
                      "installedVersion": version, "installedAt": int(time.time() * 1000),
                      "archiveSha256": hashlib.sha256(data).hexdigest(), "installer": "stepx-owner-qualified-v1",
                      "files": {name: digest for name, (digest, _) in expected.items()}}
        (stage / ".clawhub").mkdir()
        (stage / ".clawhub" / "origin.json").write_text(json.dumps(provenance), encoding="utf-8")
        # No shared lockfile rewrite: other phone/client installs may run concurrently.
        # rename() cannot overwrite a populated existing skill directory.
        require(not target.exists(), "技能安装目录已存在")
        stage.rename(target)
    finally:
        if stage.exists():
            shutil.rmtree(stage)
    installed = (target / "SKILL.md").read_bytes()
    require(hashlib.sha256(installed).hexdigest() == expected["SKILL.md"][0], "手机安装后校验失败")
    return {"ok": True, "owner": owner, "slug": slug, "version": version, "name": skill_name,
            "targetDir": str(target), "filePath": str(target / "SKILL.md"), "files": len(contents),
            "sha256": expected["SKILL.md"][0], "bytes": sum(len(raw) for raw in contents.values())}


def export(args):
    main = pathlib.Path(args["filePath"])
    require(main.is_absolute() and main.name == "SKILL.md" and main.is_file() and not main.is_symlink(), "手机技能说明路径无效")
    roots = [pathlib.Path(p).resolve() for p in args["allowedRoots"] if p and pathlib.Path(p).is_absolute()]
    base = main.parent.resolve()
    require(any(base.is_relative_to(root) for root in roots), "手机技能不在声明目录内")
    files, total, omitted = [], 0, 0
    for index, path in enumerate(sorted(base.rglob("*"))):
        require(index < 1000, "手机技能目录条目过多")
        if len(files) >= MAX_FILES:
            raise ValueError("手机技能文本文件过多")
        if path.is_symlink():
            omitted += 1
            continue
        if not path.is_file() or ".clawhub" in path.parts:
            continue
        if path.suffix.lower() not in TEXT_SUFFIXES:
            omitted += 1
            continue
        require(path.resolve().is_relative_to(base), "手机技能引用文件越界")
        require(path.stat().st_size <= MAX_TEXT, "技能引用文本过大")
        raw = path.read_bytes()
        total += len(raw)
        require(total <= MAX_TEXT, "技能引用文本总量过大")
        try:
            content = raw.decode("utf-8")
        except UnicodeDecodeError:
            omitted += 1
            continue
        files.append({"path": str(path.relative_to(base)), "content": content})
    require(any(file["path"] == "SKILL.md" for file in files), "未读取到手机 SKILL.md")
    return {"name": args["name"], "files": files, "omitted": omitted}


def origins(args):
    result = {}
    root = pathlib.Path(args["workspace"]) / "skills"
    require(root.is_absolute() and str(root.resolve()).startswith("/root/.openclaw/"), "手机技能目录异常")
    for main_path in args["files"][:MAX_FILES]:
        main = pathlib.Path(main_path)
        if not main.is_absolute() or main.is_symlink() or not main.resolve().is_relative_to(root.resolve()):
            continue
        origin = main.parent / ".clawhub" / "origin.json"
        if origin.is_file() and not origin.is_symlink() and origin.stat().st_size <= MAX_TEXT:
            data = json.loads(origin.read_text(encoding="utf-8"))
            if data.get("installer") == "stepx-owner-qualified-v1" and data.get("registry") == BASE:
                owner, slug = component(data["ownerHandle"]), component(data["slug"])
                result[main_path] = {"reference": "@" + owner + "/" + slug, "version": data["installedVersion"]}
    return result


if __name__ == "__main__":
    try:
        args = json.loads(sys.argv[1])
        action = args.get("action")
        require(action in ("install", "export", "origins"), "未知技能操作")
        result = install(args) if action == "install" else origins(args) if action == "origins" else export(args)
        print(json.dumps(result, ensure_ascii=False))
    except Exception as error:
        print(json.dumps({"ok": False, "error": str(error)[:400]}, ensure_ascii=False))
        sys.exit(1)
