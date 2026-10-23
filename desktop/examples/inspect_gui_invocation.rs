use anyhow::Result;
use stepx_companion::{adb::Adb,device::Device,amoo::AgentBus};
use serde_json::json;
fn main()->Result<()> {
 let adb=Adb::discover()?;let d=Device::inspect(&adb,"STEPX_DEVICE_SERIAL")?;let mut b=AgentBus::connect(&adb,&d)?;
 let prefix=std::env::args().nth(1).unwrap_or("请使用已安装的 stepx-gui-agent 技能".into());
 let list=b.call("sessions.list",json!({"limit":100}))?;
 for row in list["sessions"].as_array().into_iter().flatten(){
  let key=row["key"].as_str().or_else(||row["sessionKey"].as_str()).unwrap_or("");
  if !key.starts_with("agent:main:desktop-"){continue;}
  let history=b.call("chat.history",json!({"sessionId":key,"limit":80}))?;
  let rows=history["messages"].as_array().cloned().unwrap_or_default();
  if !rows.iter().any(|v|v["text"].as_str().unwrap_or("").starts_with(&prefix)){continue;}
  for m in rows {println!("{}",json!({"role":m["role"],"stop":m["stopReason"],"text":m["text"],"tools":m["toolCalls"]}));}
 }Ok(())
}
