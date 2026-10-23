use anyhow::{Context,Result,ensure};
use stepx_companion::{adb::Adb,device::Device,openclaw::GatewayWs};
use serde_json::json;
fn main()->Result<()> {
 let adb=Adb::discover()?;let device=Device::inspect(&adb,"STEPX_DEVICE_SERIAL")?;let mut ws=GatewayWs::connect(&adb,&device)?;
 let status=ws.call("skills.status",json!({}))?;let root=status["workspaceDir"].as_str().context("workspace missing")?;
 if std::env::args().nth(1).as_deref()==Some("--where") {println!("workspace={root}");return Ok(());}
 let request=std::env::args().nth(1).unwrap_or("{\"op\":\"status\"}".into());
 let result=ws.call("stepclaw.system.run",json!({"command":["python3",format!("{root}/skills/stepx-gui-agent/client.py"),request],"timeoutMs":15000}))?;
 ensure!(result["exitCode"]==0,"phone client error");println!("{}",result["stdout"].as_str().unwrap_or(""));Ok(())
}
