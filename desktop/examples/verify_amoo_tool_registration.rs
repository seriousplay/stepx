//! Read-only, bounded capability discovery. Never prints credentials or chat history.
use anyhow::{ensure, Result};
use serde_json::{json, Value};
use std::time::Duration;
use stepx_companion::{adb::Adb, amoo::AgentBus, device::Device};
fn shape(v: &Value) -> Value {
    match v {
        Value::Object(o) => Value::Object(
            o.iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        if v.is_array() {
                            json!({"count":v.as_array().unwrap().len()})
                        } else if v.is_object() {
                            shape(v)
                        } else {
                            json!(v.is_null())
                        },
                    )
                })
                .collect(),
        ),
        Value::Array(a) => json!({"count":a.len()}),
        _ => json!({"scalar":true}),
    }
}
fn main() -> Result<()> {
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, "STEPX_DEVICE_SERIAL")?;
    ensure!(device.serial == "STEPX_DEVICE_SERIAL", "wrong device");
    for method in ["skills.list", "skills.status", "tools.catalog"] {
        let mut bus = AgentBus::connect(&adb, &device)?;
        let result = bus.call_with_timeout(method, json!({}), Duration::from_secs(6));
        match result {
            Ok(v) => println!("{}", json!({"method":method,"ok":true,"shape":shape(&v)})),
            Err(e) => {
                let s = format!("{e:#}");
                println!(
                    "{}",
                    json!({"method":method,"ok":false,"category":
                    if s.to_lowercase().contains("timed out") || s.contains("超时") {"timeout"}
                    else if s.contains("NO_ROUTE") {"no_route"} else {"unavailable"}})
                );
            }
        }
    }
    Ok(())
}
