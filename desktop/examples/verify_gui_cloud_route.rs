use anyhow::Result;
use stepx_companion::{adb::Adb,device::Device,amoo::AgentBus};
use serde_json::json;
fn main()->Result<()> {
 let a=Adb::discover()?;let d=Device::inspect(&a,"STEPX_DEVICE_SERIAL")?;let mut b=AgentBus::connect(&a,&d)?;
 let v=b.call("node.list",json!({}))?;
 for n in v["nodes"].as_array().into_iter().flatten(){
  println!("{}",json!({"name":n["displayName"],"connected":n["connected"],"commands":n["commands"]}));
  if n["connected"]==true {
   let id=n["nodeId"].as_str().unwrap_or("");
   let r=b.call_with_timeout("node.invoke",json!({"nodeId":id,"command":"system.which","params":{"command":"python3"},"idempotencyKey":format!("stepx-gui-probe-{}",rand::random::<u64>()),"timeoutMs":10000}),std::time::Duration::from_secs(20));
   println!("probe={r:?}");
  }
 }Ok(())
}
