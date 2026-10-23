//! Explicit test of an installed phone skill, desktop harness, and a real DOCX.
use anyhow::{bail, ensure, Context, Result};
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use stepx_companion::{
    adb::Adb,
    amoo::AgentBus,
    amoo_catalog,
    device::Device,
    work::{self, WorkEvent, WorkRequest},
};
fn main() -> Result<()> {
    let serial = std::env::args()
        .nth(1)
        .context("usage: verify_work_skill SERIAL --send")?;
    ensure!(
        std::env::args().any(|a| a == "--send"),
        "explicit --send required"
    );
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, &serial)?;
    let focus = || -> Result<String> {
        Ok(adb
            .shell(&device.transport, &["dumpsys", "window"])?
            .lines()
            .filter(|l| l.contains("mCurrentFocus=") || l.contains("mFocusedApp="))
            .collect::<Vec<_>>()
            .join("\n"))
    };
    let before = focus()?;
    let folder = tempfile::Builder::new()
        .prefix("stepx-work-skill-live-")
        .tempdir()?
        .keep();
    let bundle = amoo_catalog::export_skill(&adb, &device, "Word / DOCX")?;
    let docs = bundle
        .files
        .into_iter()
        .map(|f| (f.path, f.content))
        .collect::<Vec<_>>();
    let installed = work::import_skill_documents(&folder, &docs)?;
    let skill_path = installed
        .path
        .strip_prefix(&folder)?
        .to_string_lossy()
        .into_owned();
    ensure!(
        std::fs::read_to_string(&installed.path)?.contains("Treat DOCX as OOXML"),
        "phone skill content mismatch"
    );
    std::fs::write(folder.join("brief.txt"),"合成项目：星桥办公验证\n任务编号：AMOO-SKILL-DOCX-20261022\n完成数量：17\n结论：资料已归档\n")?;
    println!(
        "phone_skill_imported=true name={} documents={}",
        installed.name,
        docs.len()
    );
    let mut bus = AgentBus::connect(&adb, &device)?;
    let key = bus.create_session()?;
    let prompt=format!("这是隔离的桌面技能验收。必须先用桌面 read_file 阅读已安装手机技能导入后的 {skill_path}，再 read_file 阅读 brief.txt。根据该技能有关Word/OOXML文档和A4页面的要求，使用桌面 write_document 生成 skill-report.docx，标题‘星桥办公验证’，正文必须原样包含任务编号 AMOO-SKILL-DOCX-20261022、完成数量17以及结论资料已归档。只生成这一份新的DOCX。不要调用手机exec/外部网站，不要声称已经打开办公软件；保存后使用read_file检查实际生成文档再完成。所有动作必须通过amoo_work JSON桌面工具协议。");
    let handle = work::start(
        adb.clone(),
        device.clone(),
        WorkRequest {
            prompt,
            workspace: folder.clone(),
            session_key: Some(key.clone()),
            model: None,
            attachments: vec![folder.join("brief.txt")],
        },
    );
    let mut terminal = false;
    let mut skill_read = false;
    let result = (|| -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(360);
        while Instant::now() < deadline {
            match handle.events.recv_timeout(Duration::from_secs(5)) {
                Ok(WorkEvent::Tool(log)) => {
                    if log.tool == "read_file" && log.success && log.detail.contains(&skill_path) {
                        skill_read = true;
                    }
                    println!(
                        "tool={} success={} skill_read={}",
                        log.tool, log.success, skill_read
                    );
                }
                Ok(WorkEvent::Artifact(a)) => println!("artifact={} bytes={}", a.kind, a.bytes),
                Ok(WorkEvent::Complete(_)) => {
                    terminal = true;
                    break;
                }
                Ok(WorkEvent::Failed(error)) => {
                    terminal = true;
                    bail!("{error}");
                }
                Ok(WorkEvent::Cancelled) => {
                    terminal = true;
                    bail!("cancelled");
                }
                Ok(_) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => bail!("worker disconnected"),
            }
        }
        ensure!(terminal, "test timed out");
        ensure!(skill_read, "model did not actually read imported skill");
        let artifacts = work::load_artifacts(&folder)?;
        let artifact = artifacts
            .iter()
            .find(|a| a.path.extension().is_some_and(|e| e == "docx"))
            .context("no real DOCX artifact")?;
        let python="import sys,zipfile,xml.etree.ElementTree as E;z=zipfile.ZipFile(sys.argv[1]);assert z.testzip() is None;assert '[Content_Types].xml' in z.namelist() and '_rels/.rels' in z.namelist();r=E.fromstring(z.read('word/document.xml'));t=''.join(r.itertext());assert 'AMOO-SKILL-DOCX-20261022' in t and '17' in t and '资料已归档' in t;assert r.find('.//{http://schemas.openxmlformats.org/wordprocessingml/2006/main}pgSz') is not None;print('docx_ooxml_verified=true synthetic_content_verified=true')";
        let output = std::process::Command::new("python3")
            .args(["-c", python])
            .arg(&artifact.path)
            .output()?;
        ensure!(
            output.status.success(),
            "DOCX validation failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        print!("{}", String::from_utf8_lossy(&output.stdout));
        println!("skill_read_verified=true docx={}", artifact.path.display());
        Ok(())
    })();
    if !terminal {
        handle.cancel.store(true, Ordering::SeqCst);
        let deadline = Instant::now() + Duration::from_secs(45);
        while Instant::now() < deadline {
            if matches!(
                handle.events.recv_timeout(Duration::from_secs(2)),
                Ok(WorkEvent::Cancelled | WorkEvent::Complete(_) | WorkEvent::Failed(_))
            ) {
                break;
            }
        }
    }
    let cleanup = bus.call("sessions.delete", serde_json::json!({"key":key}));
    println!("temporary_session_cleanup={}", cleanup.is_ok());
    cleanup?;
    ensure!(before == focus()?, "phone foreground changed");
    println!(
        "phone_foreground_unchanged=true workspace={}",
        folder.display()
    );
    result
}
