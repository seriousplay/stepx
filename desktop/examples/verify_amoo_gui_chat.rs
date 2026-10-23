use anyhow::Result;
use stepx_companion::{adb::Adb,device::Device,amoo::AgentBus,amoo_gui_chat,gui_agent::Control};
fn main()->Result<()> {
 let a=Adb::discover()?;let d=Device::inspect(&a,"STEPX_DEVICE_SERIAL")?;let mut b=AgentBus::connect(&a,&d)?;
 let key=b.create_session()?;
 let (pending,nonce)=amoo_gui_chat::send(&mut b,&key,"打开京东，我的订单")?;
 println!("conversation={key}");
 let result=amoo_gui_chat::finish(&a,&d,&pending,&nonce,&Control::default(),|e|match e{stepx_companion::amoo_screen::ScreenEvent::Screenshot{number,png}=>{let _=std::fs::write(format!("diagnostics/gui-agent-20261023/chat-jd-{number}.png"),png);},other=>println!("{other:?}")})?;
 println!("{}",serde_json::to_string(&result)?);Ok(())
}
