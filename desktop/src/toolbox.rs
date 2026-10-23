//! Storage tools available through the authorized shell, without system modifications.
use crate::{adb::Adb, bridge, device::Device, storage};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    time::Duration,
};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct FileInfo {
    pub path: String,
    pub bytes: u64,
    pub modified_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FolderInfo {
    pub name: String,
    pub files: usize,
    pub bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorageReport {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub free_bytes: u64,
    pub filesystem: String,
    pub scanned_files: usize,
    pub scanned_bytes: u64,
    pub limited: bool,
    pub unreadable: usize,
    pub hashed_files: usize,
    pub hash_skipped: usize,
    /// Logical bytes in duplicate copies, never a claim of freed disk space.
    pub duplicate_bytes: u64,
    pub folders: Vec<FolderInfo>,
    pub largest: Vec<FileInfo>,
    pub documents: Vec<FileInfo>,
    pub duplicates: Vec<Vec<FileInfo>>,
    pub scanned_at_ms: i64,
}

pub fn scan(adb: &Adb, device: &Device) -> Result<StorageReport> {
    let value = bridge::call_bridge(
        adb,
        device,
        &serde_json::json!({"op": "storage_scan"}),
        Duration::from_secs(45),
    )?;
    parse_report(value)
}

fn parse_report(value: serde_json::Value) -> Result<StorageReport> {
    let report: StorageReport =
        serde_json::from_value(value).context("存储扫描结果不完整，请重试")?;
    if report.total_bytes == 0
        || report.free_bytes > report.total_bytes
        || report.available_bytes > report.free_bytes
    {
        bail!("手机返回的容量数据无效，请重试");
    }
    for item in report
        .largest
        .iter()
        .chain(report.documents.iter())
        .chain(report.duplicates.iter().flatten())
    {
        storage::validate_path(&item.path)?;
    }
    Ok(report)
}

pub fn open_storage_settings(adb: &Adb, device: &Device) -> Result<()> {
    device.verify(adb)?;
    adb.shell(
        &device.transport,
        &[
            "am",
            "start",
            "-a",
            "android.settings.INTERNAL_STORAGE_SETTINGS",
        ],
    )?;
    Ok(())
}

#[derive(Clone, Debug)]
pub enum ToolOperation {
    ClearCache,
    Compress(FileInfo),
    Restore(FileInfo),
    DeleteFile(FileInfo),
    DeleteDuplicate { file: FileInfo, keep: FileInfo },
}

impl ToolOperation {
    pub fn file(&self) -> Option<&FileInfo> {
        match self {
            Self::ClearCache => None,
            Self::Compress(f)
            | Self::Restore(f)
            | Self::DeleteFile(f)
            | Self::DeleteDuplicate { file: f, .. } => Some(f),
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Self::ClearCache => "清理应用缓存",
            Self::Compress(_) => "压缩文件",
            Self::Restore(_) => "还原文件",
            Self::DeleteFile(_) => "删除文件",
            Self::DeleteDuplicate { .. } => "删除副本",
        }
    }
    pub fn request(&self) -> serde_json::Value {
        let Some(f) = self.file() else {
            return serde_json::json!({"op": "storage_clear_cache"});
        };
        let (action, keep) = match self {
            Self::Compress(_) => ("compress", ""),
            Self::ClearCache => unreachable!(),
            Self::Restore(_) => ("restore", ""),
            Self::DeleteFile(_) => ("delete_file", ""),
            Self::DeleteDuplicate { keep, .. } => ("delete_duplicate", keep.path.as_str()),
        };
        serde_json::json!({"op": "storage_file_operation", "action": action,
            "path": f.path, "bytes": f.bytes, "modified_ms": f.modified_ms, "keep": keep})
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct OperationResult {
    pub completed: bool,
    pub available_before: u64,
    pub available_after: u64,
    pub logical_saved: i64,
    pub result_path: String,
}

pub fn operate(adb: &Adb, device: &Device, operation: &ToolOperation) -> Result<OperationResult> {
    if let Some(file) = operation.file() {
        storage::validate_path(&file.path)?;
    }
    let value = bridge::call_bridge(adb, device, &operation.request(), Duration::from_secs(60))?;
    serde_json::from_value(value).context("操作结果未能确认，请重新扫描")
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SystemCompression {
    pub kernel_support: bool,
    pub volume_support: bool,
    pub filesystem: String,
    pub features: String,
}

/// Reuse the GMS project's existing, firmware-guarded root bridge for sysfs reads only.
pub fn probe_system(adb: &Adb, device: &Device) -> Result<SystemCompression> {
    device.verify(adb)?;
    let tools =
        crate::resources::root().join("handoff/google-play-stepx/tools");
    if !tools.join("root_bridge.py").is_file() {
        bail!("此安装未附带系统能力检测组件");
    }
    let probe = r#"import json,os
parents=[]
for pid in os.listdir('/proc'):
 if not pid.isdigit(): continue
 try:
  status=open('/proc/'+pid+'/status').read()
  if 'Name:\topenclaw_env\n' in status and 'PPid:\t1\n' in status: parents.append(pid)
 except OSError: pass
if len(parents)!=1: raise RuntimeError('无法定位原厂主机环境')
root='/proc/'+parents[0]+'/root'
mount=next(x.split() for x in open('/proc/'+parents[0]+'/mounts') if x.split()[1]=='/data')
device=mount[0].rsplit('/',1)[-1]
if mount[2]!='f2fs': raise RuntimeError('数据分区不是 F2FS')
features=open(root+'/sys/fs/f2fs/'+device+'/features').read().strip()
kernel=open(root+'/sys/fs/f2fs/features/compression').read().strip()=='supported'
print(json.dumps({'kernel_support':kernel,'volume_support':'compression' in features.split(', '),'filesystem':mount[2],'features':features}))
"#;
    let launcher = "import sys,subprocess,json; from common import guard; code=sys.stdin.read(); guard(native=True); p=subprocess.run([sys.executable,'root_bridge.py'],input=code,text=True,capture_output=True,timeout=40); d=json.JSONDecoder().raw_decode(p.stdout.lstrip())[0]; print(json.dumps(d))";
    let mut command = Command::new(crate::resources::python());
    command
        .current_dir(tools)
        .arg("-c")
        .arg(launcher)
        .env("STEPX_SERIAL", &device.transport)
        .env("STEPX_ADB", &adb.path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = crate::adb::run_process(
        command,
        Some(probe.as_bytes().to_vec()),
        Duration::from_secs(50),
    )?;
    if !output.status.success() {
        bail!("系统能力检测未完成，请检查 StepClaw 服务及当前固件是否受支持");
    }
    serde_json::from_slice(&output.stdout).context("没有收到有效的系统能力检测结果")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mutations_are_bound_to_scanned_file_and_explicit_keep_copy() {
        let f = FileInfo {
            path: "/sdcard/Documents/a.txt".into(),
            bytes: 500,
            modified_ms: 12,
        };
        let k = FileInfo {
            path: "/sdcard/Documents/b.txt".into(),
            ..f.clone()
        };
        let request = ToolOperation::DeleteDuplicate { file: f, keep: k }.request();
        assert_eq!(request["bytes"], 500);
        assert_eq!(request["modified_ms"], 12);
        assert_eq!(request["keep"], "/sdcard/Documents/b.txt");
        assert_eq!(request["action"], "delete_duplicate");
    }
    #[test]
    fn incomplete_or_impossible_storage_results_are_rejected() {
        assert!(parse_report(serde_json::json!({"ok": true})).is_err());
        let mut report = serde_json::json!({
            "total_bytes": 1000, "available_bytes": 300, "free_bytes": 400,
            "filesystem": "f2fs", "scanned_files": 0, "scanned_bytes": 0,
            "limited": false, "unreadable": 0, "hashed_files": 0, "hash_skipped": 0,
            "duplicate_bytes": 0, "folders": [], "largest": [], "documents": [],
            "duplicates": [], "scanned_at_ms": 123,
        });
        assert!(parse_report(report.clone()).is_ok());
        report["available_bytes"] = serde_json::json!(1001);
        assert!(parse_report(report.clone()).is_err());
        report["available_bytes"] = serde_json::json!(300);
        report["documents"] = serde_json::json!([{"path": "/data/user/0/private.txt", "bytes": 12, "modified_ms": 1}]);
        assert!(parse_report(report).is_err());
    }
}
