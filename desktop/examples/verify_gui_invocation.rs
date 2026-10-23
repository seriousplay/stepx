use anyhow::{Result,ensure};
use stepx_companion::{adb::Adb,device::Device,amoo::AgentBus};
use serde_json::json;
fn main()->Result<()> {
 let adb=Adb::discover()?;let device=Device::inspect(&adb,"STEPX_DEVICE_SERIAL")?;
 let mut bus=AgentBus::connect(&adb,&device)?;let key=bus.create_session()?;
 let prompt=std::env::args().nth(1).unwrap_or("请使用已安装的 stepx-gui-agent 技能，通过 StepX伴侣操作手机：打开设置，进入关于手机，读取 Android 版本。只浏览，不修改设置。请等待 GUI 任务真正完成后再告诉我结果，不要使用原厂 gui_agent。".into());
 let pending=bus.send(&key,&prompt)?;
 let result=(||->Result<()> {
  for _ in 0..100 {
   let history=bus.call("chat.history",json!({"sessionId":key,"limit":100}))?;
   for row in history["messages"].as_array().into_iter().flatten().filter(|m|m["runId"]==pending.run_id&&m["role"]=="assistant") {
    if row["stopReason"]=="STOP"||row["stopReason"]=="stop" {
     println!("{}",json!({"text":row["text"],"model":row["responseModel"],"stopReason":row["stopReason"]}));
     ensure!(row["isError"]!=true,"Amoo error");return Ok(());
    }
   }
   std::thread::sleep(std::time::Duration::from_secs(3));
  }
  anyhow::bail!("Amoo invocation timeout")
 })();let deleted=bus.call("sessions.delete",json!({"key":key})).is_ok();println!("session_deleted={deleted}");result
}
