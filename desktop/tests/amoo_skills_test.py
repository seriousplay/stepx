"""Regression tests for phone-side archive validation and text-only export."""
import hashlib
import importlib.util
import io
import json
import pathlib
import tempfile
import unittest
from unittest.mock import patch
import zipfile

spec = importlib.util.spec_from_file_location("amoo_skills", pathlib.Path(__file__).parents[1] / "src" / "amoo_skills.py")
skills = importlib.util.module_from_spec(spec)
spec.loader.exec_module(skills)


class SkillArchiveTests(unittest.TestCase):
    def responses(self, owner="author", file_name="SKILL.md", corrupt=False):
        content = b"---\nname: Office Example\n---\n# Process office documents\n"
        detail = {"owner": {"handle": owner}, "skill": {"slug": "office"}, "latestVersion": {"version": "1.0.0"}}
        version = {"skill": {"slug": "office"}, "version": {"version": "1.0.0", "files": [{"path": "SKILL.md", "size": len(content), "sha256": hashlib.sha256(content).hexdigest()}]}}
        out = io.BytesIO()
        with zipfile.ZipFile(out, "w") as archive:
            archive.writestr(file_name, content.replace(b"Office", b"Malice") if corrupt else content)
            archive.writestr("_meta.json", json.dumps({"slug": "office", "version": "1.0.0"}))
        return [json.dumps(detail).encode(), json.dumps(version).encode(), out.getvalue()]

    def install_fails(self, expected, responses, existing=None):
        with patch.object(skills, "request", side_effect=responses):
            with self.assertRaisesRegex(ValueError, expected):
                skills.install({"owner": "author", "slug": "office", "workspace": "/root/.openclaw/workspace", "existingNames": existing or []})

    def test_never_drops_requested_publisher(self):
        self.install_fails("发布者", self.responses(owner="someone-else"))

    def test_modified_file_rejected_before_install(self):
        self.install_fails("完整性", self.responses(corrupt=True))

    def test_zip_traversal_rejected_before_install(self):
        self.install_fails("越界", self.responses(file_name="../SKILL.md"))

    def test_existing_skill_is_not_overwritten(self):
        self.install_fails("同名", self.responses(), ["Office Example"])

    def test_export_includes_references_but_not_scripts_or_symlinks(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            skill = root / "office"
            skill.mkdir()
            (skill / "SKILL.md").write_text("# Office\nRead reference.md", encoding="utf-8")
            (skill / "reference.md").write_text("Reference", encoding="utf-8")
            (skill / "run.py").write_text("raise RuntimeError('must never run')", encoding="utf-8")
            (root / "outside.md").write_text("private", encoding="utf-8")
            (skill / "link.md").symlink_to(root / "outside.md")
            bundle = skills.export({"name": "Office", "filePath": str(skill / "SKILL.md"), "allowedRoots": [str(root)]})
            self.assertEqual({f["path"] for f in bundle["files"]}, {"SKILL.md", "reference.md"})
            self.assertEqual(bundle["omitted"], 2)


if __name__ == "__main__":
    unittest.main()
