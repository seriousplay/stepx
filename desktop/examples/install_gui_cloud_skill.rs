use anyhow::Result;
use stepx_companion::{adb::Adb,device::Device,amoo::AgentBus,amoo_gui_chat};
fn main()->Result<()> {
 let a=Adb::discover()?;let d=Device::inspect(&a,"STEPX_DEVICE_SERIAL")?;let mut b=AgentBus::connect(&a,&d)?;
 let key=b.create_session()?;let p=amoo_gui_chat::cloud_install_prompt(&a,&d)?;let r=b.send(&key,&p)?;
 println!("installation_requested=true");
 for _ in 0..70 {std::thread::sleep(std::time::Duration::from_secs(3));let h=b.call("chat.history",serde_json::json!({"sessionId":key,"limit":50}))?;
 for m in h["messages"].as_array().into_iter().flatten(){if m["runId"]==r.run_id&&m["role"]=="assistant"&&m["stopReason"]=="STOP"{println!("{}",m["text"]);return Ok(());}}
 }anyhow::bail!("installation unconfirmed")
}
