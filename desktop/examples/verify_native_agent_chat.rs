//! Uses the ordinary Amoo conversation API, with no injected prompt/protocol.
use anyhow::{ensure,Result};
use serde_json::json;
use stepx_companion::{adb::Adb,device::Device,amoo::AgentBus};
fn main()->Result<()>{
    let adb=Adb::discover()?;let device=Device::inspect(&adb,"STEPX_DEVICE_SERIAL")?;
    ensure!(device.serial=="STEPX_DEVICE_SERIAL","错误设备");
    let mut bus=AgentBus::connect(&adb,&device)?;
    let key=bus.create_session()?;
    let prompt=std::env::args().nth(1).unwrap_or("打开京东，我的订单".into());
    let pending=bus.send(&key,&prompt)?;
    println!("conversation={key} normal_message_sent=true");
    let path=format!("../diagnostics/native-mcp-20261023/independent-chat-{}.json",pending.run_id.replace(|c:char|!c.is_ascii_alphanumeric(),""));
    for _ in 0..120 {
        let history=bus.call("chat.history",json!({"sessionId":key,"limit":100}))?;
        std::fs::write(&path,serde_json::to_vec_pretty(&json!({"session":key,"run":pending.run_id,"history":history}))?)?;
        for row in history["messages"].as_array().into_iter().flatten().filter(|m|m["runId"]==pending.run_id&&m["role"]=="assistant") {
            if row["stopReason"]=="STOP"||row["stopReason"]=="stop" {
                println!("{}",json!({"text":row["text"],"model":row["responseModel"],"isError":row["isError"],"evidence_file":path}));
                ensure!(row["isError"]!=true,"Amoo error");return Ok(());
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(3));
    }
    anyhow::bail!("Amoo 尚未结束，测试记录保留在 {path}")
}
