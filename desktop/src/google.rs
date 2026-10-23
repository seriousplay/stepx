//! Desktop entry points for the fixed-firmware Google workflow. Inspection is
//! read-only; compatibility changes require an explicit installation or action.
use crate::{
    adb::Adb,
    device::{Device, GoogleSuiteStatus},
};
use anyhow::{bail, Context, Result};
use std::{path::PathBuf, process::Command};

pub fn workflow_root() -> Result<PathBuf> {
    let root = std::env::var_os("STEPX_GOOGLE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            crate::resources::root().join("handoff/google-play-stepx")
        });
    if !root.join("tools/manage.py").is_file() {
        bail!("缺少 Google 安装组件，请配置 STEPX_GOOGLE_HOME 后重试");
    }
    Ok(root)
}

pub fn run_script(device: &Device, adb: &Adb, script: &str, args: &[&str]) -> Result<String> {
    let root = workflow_root()?;
    let output = crate::resources::python_command()
        .arg(root.join("tools").join(script))
        .args(args)
        .current_dir(&root)
        .env("STEPX_SERIAL", &device.transport)
        .env("STEPX_ADB", &adb.path)
        .env_remove("PYTHONOPTIMIZE")
        .output()
        .context("无法启动 Google 安装组件，请检查 Python 3 环境")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let reason = stderr
            .lines()
            .rev()
            .find(|s| !s.trim().is_empty())
            .unwrap_or("组件未完成操作");
        bail!("{} {} 失败：{}", script, args.join(" "), reason);
    }
    Ok(String::from_utf8(output.stdout)?.trim().into())
}

fn run_compatibility_script(device: &Device, adb: &Adb, action: &str) -> Result<serde_json::Value> {
    if !matches!(action, "status" | "apply" | "rollback") {
        bail!("不支持的 Google 兼容补丁操作");
    }
    let script =
        crate::resources::root().join("scripts/google-play-compat.py");
    if !script.is_file() {
        bail!("当前桌面安装缺少 Google Play 兼容组件，请更新 desktop 后重试");
    }
    let output = crate::resources::python_command()
        .arg(&script)
        .arg(action)
        .current_dir(script.parent().context("兼容组件目录无效")?)
        .env("STEPX_SERIAL", &device.transport)
        .env("STEPX_ADB", &adb.path)
        .env_remove("PYTHONOPTIMIZE")
        .output()
        .context("无法启动 Google Play 兼容组件，请检查 Python 3 环境")?;
    // Only the component's bounded JSON summary may reach the UI. Worker
    // stderr and private rollback receipts are never displayed here.
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)
        .context("Google Play 兼容组件未返回有效状态，请重新检测")?;
    if value["ok"].as_bool().is_none()
        || value["supported"].as_bool().is_none()
        || value["active"].as_bool().is_none()
        || value["can_rollback"].as_bool().is_none()
        || value["message"].as_str().is_none()
    {
        bail!("Google Play 兼容组件返回的状态不完整");
    }
    if !output.status.success() && value["ok"] == true {
        bail!("Google Play 兼容组件异常退出，请重新检测");
    }
    Ok(value)
}

fn apply_compatibility_status(status: &mut GoogleSuiteStatus, value: &serde_json::Value) {
    status.compatibility_checked = true;
    status.compatibility_supported =
        status.installed && value["ok"] == true && value["supported"] == true;
    status.compatibility_active = status.compatibility_supported && value["active"] == true;
    // A valid backup can remain available after a partial application failure.
    status.compatibility_can_rollback = status.installed && value["can_rollback"] == true;
    status.compatibility_note = value["message"]
        .as_str()
        .unwrap_or("兼容补丁状态未确认，请重新检测")
        .into();
}

fn should_apply_during_install(plan: GoogleInstallPlan, status: &GoogleSuiteStatus) -> bool {
    plan == GoogleInstallPlan::Existing
        && status.installed
        && status.compatibility_checked
        && status.compatibility_supported
        && !status.compatibility_active
}

/// Called only after the existing utility confirmation panel or an explicitly
/// confirmed combined installation. The CLI rechecks exact device/APK guards.
pub fn change_compatibility(
    device: &Device,
    adb: &Adb,
    rollback: bool,
    progress: impl Fn(&str),
) -> Result<(GoogleSuiteStatus, String)> {
    device.verify(adb)?;
    progress(if rollback {
        "核对备份并撤销 Google Play 兼容补丁…"
    } else {
        "核对版本、保存备份并应用 Google Play 兼容补丁…"
    });
    let value = run_compatibility_script(device, adb, if rollback { "rollback" } else { "apply" })?;
    let message = value["message"]
        .as_str()
        .unwrap_or("兼容操作未完成")
        .to_string();
    if value["ok"] != true {
        bail!("{message}");
    }
    progress("重新读取 Google 服务与兼容补丁状态…");
    let status = inspect(device, adb)
        .with_context(|| format!("{message} 状态回读未完成，请重新检测"))?;
    if !status.compatibility_checked || status.compatibility_active == rollback {
        bail!(
            "兼容补丁状态回读与操作结果不一致，请点击检测状态：{}",
            status.compatibility_note
        );
    }
    Ok((status, message))
}

pub fn apply_adapter_status(status: &mut GoogleSuiteStatus, value: &serde_json::Value) -> bool {
    let count = value["permission_checks"].as_u64().unwrap_or(0) as u32;
    status.total_permissions = count;
    status.system_server_pid = value["target_pid"].as_u64().unwrap_or(0) as u32;
    status.pid_matches = value["current_pid_matches"].as_bool() == Some(true)
        && value["current_boot_matches"].as_bool() == Some(true);
    let ready = value["status"].as_str() == Some("success")
        && value["detached"].as_bool() == Some(true)
        && status.pid_matches
        && status.system_server_pid > 0
        && count >= 120
        && value["all_checked_granted"].as_bool() == Some(true);
    status.granted_count = if ready { count } else { 0 };
    status.boot_module_ready = ready;
    ready
}

fn package_flags_clear(flags: &serde_json::Value) -> Result<bool> {
    // Android writes signed 32-bit flags to packages.xml; negative values are valid.
    let parse = |key: &str| -> Result<i64> {
        flags[key]
            .as_str()
            .context("未读取到持久化标记")?
            .parse()
            .context("持久化标记无效")
    };
    Ok(parse("publicFlags")? & 1 == 0 && parse("privateFlags")? & 8 == 0)
}

pub fn inspect(device: &Device, adb: &Adb) -> Result<GoogleSuiteStatus> {
    device.verify(adb)?;
    let packages = adb.shell(
        &device.transport,
        &["pm", "list", "packages", "--user", "0"],
    )?;
    let mut status = GoogleSuiteStatus {
        checked: true,
        ..Default::default()
    };
    match run_script(device, adb, "certification.py", &[])
        .and_then(|text| Ok(serde_json::from_str::<serde_json::Value>(&text)?)) {
        Ok(value) => {
            status.certification_note = if value["certification_block_visible"] == true {
                "已在手机发现“设备未认证”拦截页；Google 服务当前不可按正常可用验收。"
            } else { "未读取到 Google 认证结果；请在手机 Play → 设置 → 关于核对。" }.into();
            status.certification_catalog = value["catalog_note"].as_str().unwrap_or("机型列表未确认").into();
            status.boot_security_note = format!("Bootloader：{} · Verified Boot：{}",
                match value["bootloader_locked"].as_str() {Some("1")=>"已锁定",Some("0")=>"已解锁",_=>"未知"},
                value["verified_boot"].as_str().unwrap_or("未知"));
            if value["disabled_components"].as_array().is_some_and(|a| !a.is_empty()) {
                status.certification_note.push_str(" 检测到 Google 组件被停用，请在手机应用设置中启用后重试。");
            }
        }
        Err(_) => status.certification_note = "认证诊断暂不可用；权限校验成功不代表 Google 认证通过。".into(),
    }
    for (package, target) in [
        ("com.google.android.gsf", &mut status.gsf_version),
        ("com.google.android.gms", &mut status.gms_version),
        ("com.android.vending", &mut status.play_version),
    ] {
        if packages
            .lines()
            .any(|line| line.trim() == format!("package:{package}"))
        {
            let dump = adb.shell(&device.transport, &["dumpsys", "package", package])?;
            *target = dump
                .lines()
                .find_map(|l| l.trim().strip_prefix("versionName="))
                .unwrap_or("版本未知")
                .to_string();
        }
    }
    status.installed = [
        &status.gsf_version,
        &status.gms_version,
        &status.play_version,
    ]
    .iter()
    .all(|v| !v.is_empty());
    if !status.installed {
        status.compatibility_note = "完整安装 Google 服务后，可检查兼容补丁适配情况。".into();
        status.verification_note = if [
            &status.gsf_version,
            &status.gms_version,
            &status.play_version,
        ]
        .iter()
        .all(|v| v.is_empty())
        {
            "未检测到 Google 服务，可继续安装条件检查。"
        } else {
            "检测到部分 Google 组件。为保留现有数据，已停用重复安装，请先检查已有安装。"
        }
        .into();
        return Ok(status);
    }
    match run_compatibility_script(device, adb, "status") {
        Ok(value) => apply_compatibility_status(&mut status, &value),
        Err(error) => status.compatibility_note = format!("兼容补丁暂未完成检测：{error}"),
    }
    let verification = (|| -> Result<()> {
        run_script(device, adb, "manage.py", &["preflight"])?;
        let value: serde_json::Value =
            serde_json::from_str(&run_script(device, adb, "manage.py", &["status"])?)?;
        if !apply_adapter_status(&mut status, &value) {
            bail!("当前启动的权限校验未通过");
        }
        let package_state: serde_json::Value =
            serde_json::from_str(&run_script(device, adb, "package_state.py", &[])?)?;
        status.persistent_flags_clear = package_flags_clear(&package_state["persistent_package"])?;
        if !status.persistent_flags_clear {
            bail!("检测到异常的持久化标记，请停止安装操作并检查设备");
        }
        Ok(())
    })();
    status.verification_note = match verification {
        Ok(()) => format!(
            "三件套已安装；本次启动权限检测 {}/{} 通过。请在手机打开 Play 验证实际使用。",
            status.granted_count, status.total_permissions
        ),
        Err(e) => format!(
            "三件套已安装，适配状态未确认。请解锁手机并确认 StepClaw 可用，再检测。原因：{e}"
        ),
    };
    Ok(status)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GoogleInstallPlan {
    Missing,
    Existing,
    Partial,
}

fn install_plan(preflight: &serde_json::Value) -> Result<GoogleInstallPlan> {
    let packages = preflight["google_packages"]
        .as_object()
        .context("未读取到 Google 包状态")?;
    let mut installed = 0;
    for package in [
        "com.google.android.gsf",
        "com.google.android.gms",
        "com.android.vending",
    ] {
        installed += usize::from(
            packages
                .get(package)
                .and_then(serde_json::Value::as_bool)
                .context("Google 包检测结果不完整")?,
        );
    }
    Ok(match installed {
        0 => GoogleInstallPlan::Missing,
        3 => GoogleInstallPlan::Existing,
        _ => GoogleInstallPlan::Partial,
    })
}

pub struct GoogleInstallResult {
    pub google: GoogleSuiteStatus,
    pub companion: crate::companion_app::CompanionStatus,
    pub message: String,
}

/// A single user-facing installation action always provisions the companion.
/// Existing/partial Google APKs are preserved. A verified existing bundle can
/// receive the compatibility patch as part of this explicit installation.
pub fn install(device: &Device, adb: &Adb, progress: impl Fn(&str)) -> Result<GoogleInstallResult> {
    device.verify(adb)?;
    progress("检查 Google Play 与 StepX伴侣安装条件…");
    let preflight: serde_json::Value =
        serde_json::from_str(&run_script(device, adb, "manage.py", &["preflight"])?)?;
    if preflight["user_unlocked"].as_bool() != Some(true) {
        bail!("请先解锁手机，再安装 Google Play 与 StepX伴侣");
    }
    let plan = install_plan(&preflight)?;
    let companion = crate::companion_app::install(adb, device, |message| {
        progress(&format!("StepX伴侣 · {message}"));
    })
    .context("StepX伴侣未完成安装或权限校验；尚未改动 Google 安装")?;
    let google_message = match plan {
        GoogleInstallPlan::Missing => {
            install_fresh(device, adb, |label| progress(&format!("Google Play · {label}")))
                .with_context(|| format!("{} Google Play 安装尚未完成", companion.message))?
        }
        GoogleInstallPlan::Existing => "已有 Google 组件已保留，未重复安装。".into(),
        GoogleInstallPlan::Partial => "Google 组件不完整，现有组件已保留；请按检测结果处理 Google 安装。".into(),
    };
    // Preserve a partial companion service failure in this combined entry point.
    let mut message = format!("{} {google_message}", companion.message);
    progress("正在回读 Google Play 与 StepX伴侣安装状态…");
    let mut google = inspect(device, adb)
        .with_context(|| format!("{} Google Play 状态回读未完成", companion.message))?;
    if should_apply_during_install(plan, &google) {
        let (updated, patch_message) = change_compatibility(device, adb, false, &progress)
            .with_context(|| format!("{} Google 组件已保留，兼容补丁未完成", companion.message))?;
        google = updated;
        message.push(' ');
        message.push_str(&patch_message);
    } else if plan == GoogleInstallPlan::Existing && !google.compatibility_active {
        message.push(' ');
        message.push_str(&google.compatibility_note);
    }
    Ok(GoogleInstallResult {
        google,
        companion,
        message,
    })
}

fn install_fresh(device: &Device, adb: &Adb, progress: impl Fn(&str)) -> Result<String> {
    device.verify(adb)?;
    progress("1 / 6 · 检查设备与安装条件");
    // Preflight validates the exact firmware before downloading or changing phone data.
    let preflight: serde_json::Value =
        serde_json::from_str(&run_script(device, adb, "manage.py", &["preflight"])?)?;
    if preflight["user_unlocked"].as_bool() != Some(true) {
        bail!("请先在手机上解锁，再安装 Google 服务");
    }
    let packages = preflight["google_packages"]
        .as_object()
        .context("未读取到 Google 包状态")?;
    if packages.len() != 3 || packages.values().any(|v| v.as_bool() != Some(false)) {
        bail!("手机已有 Google 组件，已停止重复安装；请使用检测状态");
    }
    // These scripts recheck serial, firmware, APK signatures and baseline before each mutation.
    for (stage, label) in [
        ("fetch", "2 / 6 · 获取并核验安装包"),
        ("prepare", "3 / 6 · 保存安装基线"),
        ("install-apks", "4 / 6 · 安装 Google 三件套"),
    ] {
        progress(label);
        run_script(device, adb, "manage.py", &[stage])
            .with_context(|| format!("Google 安装停在 {stage} 阶段；请检查原因，勿重复覆盖安装"))?;
    }
    progress("5 / 6 · 保存首次启动前的包状态");
    run_script(device, adb, "package_state.py", &[])?;
    let state = workflow_root()?.join("state");
    let before = state.join("package-before-first-reboot.json");
    // Never overwrite evidence from a prior attempt.
    let mut target = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&before)?;
    use std::io::Write;
    target.write_all(&std::fs::read(state.join("package-state-current.json"))?)?;
    progress("6 / 6 · 安装开机适配，完成后需手动重启");
    run_script(device, adb, "install_adapter.py", &[])?;
    Ok("安装步骤已完成。请手动重启手机并解锁，再点击「检测状态」；确认适配通过前暂不要登录 Google。".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compatibility_status_keeps_certification_separate_and_requires_guards() {
        let mut status = GoogleSuiteStatus {
            installed: true,
            certification_note: "认证未核验".into(),
            ..Default::default()
        };
        let valid = serde_json::json!({
            "ok": true, "supported": true, "active": true,
            "can_rollback": true, "message": "已应用兼容补丁"
        });
        apply_compatibility_status(&mut status, &valid);
        assert!(status.compatibility_active);
        assert!(status.compatibility_can_rollback);
        assert_eq!(status.certification_note, "认证未核验");
        for key in ["ok", "supported", "active"] {
            let mut missing = valid.clone();
            missing.as_object_mut().unwrap().remove(key);
            apply_compatibility_status(&mut status, &missing);
            assert!(!status.compatibility_active, "{key}");
        }
        status.installed = false;
        apply_compatibility_status(&mut status, &valid);
        assert!(!status.compatibility_supported);
        assert!(!status.compatibility_active);
        assert!(!status.compatibility_can_rollback);
    }

    #[test]
    fn failed_patch_preserves_the_rollback_entry() {
        let mut status = GoogleSuiteStatus { installed: true, ..Default::default() };
        apply_compatibility_status(&mut status, &serde_json::json!({
            "ok": false, "supported": false, "active": false,
            "can_rollback": true, "message": "本次应用未完成，可撤销已修改的部分"
        }));
        assert!(status.compatibility_checked);
        assert!(!status.compatibility_active);
        assert!(status.compatibility_can_rollback);
        assert!(status.compatibility_note.contains("未完成"));
    }

    #[test]
    fn combined_install_only_patches_a_verified_existing_bundle() {
        let valid = GoogleSuiteStatus {
            installed: true,
            compatibility_checked: true,
            compatibility_supported: true,
            ..Default::default()
        };
        assert!(should_apply_during_install(GoogleInstallPlan::Existing, &valid));
        for plan in [GoogleInstallPlan::Missing, GoogleInstallPlan::Partial] {
            assert!(!should_apply_during_install(plan, &valid));
        }
        let mut status = valid.clone();
        status.compatibility_active = true;
        assert!(!should_apply_during_install(GoogleInstallPlan::Existing, &status));
        status = valid.clone();
        status.compatibility_checked = false;
        assert!(!should_apply_during_install(GoogleInstallPlan::Existing, &status));
        status = valid.clone();
        status.compatibility_supported = false;
        assert!(!should_apply_during_install(GoogleInstallPlan::Existing, &status));
        status = valid;
        status.installed = false;
        assert!(!should_apply_during_install(GoogleInstallPlan::Existing, &status));
    }

    #[test]
    fn installation_plan_preserves_existing_and_partial_google_packages() {
        let report = |gsf, gms, play| serde_json::json!({"google_packages":{"com.google.android.gsf":gsf,"com.google.android.gms":gms,"com.android.vending":play}});
        assert_eq!(
            install_plan(&report(false, false, false)).unwrap(),
            GoogleInstallPlan::Missing
        );
        assert_eq!(
            install_plan(&report(true, true, true)).unwrap(),
            GoogleInstallPlan::Existing
        );
        assert_eq!(
            install_plan(&report(false, true, true)).unwrap(),
            GoogleInstallPlan::Partial
        );
        assert_eq!(
            install_plan(&report(true, false, false)).unwrap(),
            GoogleInstallPlan::Partial
        );
        assert!(install_plan(&serde_json::json!({})).is_err());
        assert!(install_plan(&serde_json::json!({"google_packages":{"com.google.android.gsf":true,"com.google.android.gms":true}})).is_err());
    }
    #[test]
    fn signed_android_flags_are_checked_without_assuming_clear() {
        assert!(package_flags_clear(
            &serde_json::json!({"publicFlags":"-1605714364", "privateFlags":"-1476357872"})
        )
        .unwrap());
        assert!(
            !package_flags_clear(&serde_json::json!({"publicFlags":"1", "privateFlags":"0"}))
                .unwrap()
        );
        assert!(
            !package_flags_clear(&serde_json::json!({"publicFlags":"0", "privateFlags":"8"}))
                .unwrap()
        );
        assert!(package_flags_clear(&serde_json::json!({"publicFlags":"0"})).is_err());
    }
    #[test]
    fn missing_or_stale_adapter_evidence_never_passes() {
        let valid = serde_json::json!({"status":"success", "detached":true, "current_pid_matches":true,
            "current_boot_matches":true, "permission_checks":153, "all_checked_granted":true, "target_pid":123});
        assert!(apply_adapter_status(
            &mut GoogleSuiteStatus::default(),
            &valid
        ));
        for key in [
            "status",
            "detached",
            "current_pid_matches",
            "current_boot_matches",
            "permission_checks",
            "all_checked_granted",
            "target_pid",
        ] {
            let mut missing = valid.clone();
            missing.as_object_mut().unwrap().remove(key);
            assert!(
                !apply_adapter_status(&mut GoogleSuiteStatus::default(), &missing),
                "{key}"
            );
        }
        assert!(!apply_adapter_status(
            &mut GoogleSuiteStatus::default(),
            &serde_json::json!({})
        ));
    }
}
