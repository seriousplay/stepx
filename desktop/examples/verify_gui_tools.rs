use anyhow::Result;
use stepx_companion::{adb::Adb,device::Device,gui_agent_api,openclaw::GatewayWs};
fn main()->Result<()> {
 let adb=Adb::discover()?;let device=Device::inspect(&adb,"STEPX_DEVICE_SERIAL")?;
 gui_agent_api::start(&adb,&device)?;
 println!("GUI tool installed and callable from phone");
 let mut ws=GatewayWs::connect(&adb,&device)?;
 let status=ws.call("skills.status",serde_json::json!({}))?;
 let found=status["skills"].as_array().into_iter().flatten().any(|v|v["name"]=="stepx-gui-agent"||v["skillKey"]=="stepx-gui-agent");
 println!("skill_catalog_visible={found}");
 anyhow::ensure!(found,"skill not visible");
 std::thread::sleep(std::time::Duration::from_secs(1200));Ok(())
}
