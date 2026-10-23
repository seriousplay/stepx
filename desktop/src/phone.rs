use crate::{adb::Adb, bridge::call_bridge, device::Device};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;

/// A call is prepared without placing it. The UI must display this exact plan
/// and require a second click before submitting it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DialPlan {
    pub request_id: String,
    pub device_serial: String,
    pub number: String,
    pub subscription: i32,
    pub subscription_label: String,
    pub account_key: String,
}

pub fn normalize_number(number: &str) -> Result<String> {
    let number = number.trim();
    if number.is_empty() || number.len() > 64 {
        bail!("请输入有效的电话号码");
    }
    let mut clean = String::new();
    for ch in number.chars() {
        if ch.is_ascii_digit() || (ch == '+' && clean.is_empty()) {
            clean.push(ch);
        } else if !" -()".contains(ch) {
            bail!("号码只支持数字、开头的 + 和空格、括号、短横线；服务代码请在手机拨号");
        }
    }
    let digits = clean.trim_start_matches('+');
    if digits.len() < 2 || digits.len() > 40 || !digits.chars().all(|c| c.is_ascii_digit()) {
        bail!("请输入有效的电话号码");
    }
    Ok(clean)
}

pub fn prepare_call(
    adb: &Adb,
    device: &Device,
    number: &str,
    subscription: Option<i32>,
) -> Result<DialPlan> {
    let number = normalize_number(number)?;
    let result = call_bridge(
        adb,
        device,
        &json!({"op": "phone_prepare", "number": number, "subscription": subscription.unwrap_or(-1)}),
        Duration::from_secs(15),
    )?;
    Ok(DialPlan {
        request_id: format!(
            "{:016x}{:016x}",
            rand::random::<u64>(),
            rand::random::<u64>()
        ),
        device_serial: device.serial.clone(),
        number,
        subscription: result
            .get("subscription")
            .and_then(|v| v.as_i64())
            .context("手机未返回有效的通话 SIM")? as i32,
        subscription_label: result
            .get("subscription_label")
            .and_then(|v| v.as_str())
            .context("手机未返回通话 SIM 名称")?
            .to_owned(),
        account_key: result
            .get("account_key")
            .and_then(|v| v.as_str())
            .context("手机未返回可用的通话账户")?
            .to_owned(),
    })
}

pub fn submit_call(adb: &Adb, device: &Device, plan: &DialPlan) -> Result<String> {
    if plan.device_serial != device.serial {
        bail!("连接设备已变更，请重新确认呼叫号码与 SIM 卡");
    }
    let result = call_bridge(
        adb,
        device,
        &json!({
            "op": "phone_call", "id": plan.request_id, "number": plan.number,
            "subscription": plan.subscription, "account_key": plan.account_key,
        }),
        Duration::from_secs(15),
    )
    .context("未能确认呼叫请求结果，未自动重拨；请先检查手机通话状态")?;
    match result.get("state").and_then(|v| v.as_str()) {
        Some("submitted") => Ok(format!(
            "已通过 {} 提交呼叫请求，请查看手机通话状态",
            plan.subscription_label
        )),
        Some("unknown") => bail!("呼叫请求状态未确认，未自动重拨；请先检查手机通话状态"),
        _ => bail!("手机未返回有效的呼叫提交状态，未自动重拨"),
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_number;

    #[test]
    fn formats_numbers_without_changing_destination() {
        assert_eq!(
            normalize_number(" +1 (202) 555-0148 ").unwrap(),
            "+12025550148"
        );
        assert_eq!(normalize_number("010-5555 0123").unwrap(), "01055550123");
    }

    #[test]
    fn rejects_mmi_extensions_and_invalid_characters_instead_of_stripping_them() {
        for input in [
            "",
            " ",
            "+",
            "1",
            "+  +123",
            "123+456",
            "123abc456",
            "123,456",
            "123;456",
            "*#06#",
            "１２３４５",
        ] {
            assert!(normalize_number(input).is_err(), "accepted {input:?}");
        }
    }
}
