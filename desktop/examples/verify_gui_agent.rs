use anyhow::{ensure,Context,Result};
use stepx_companion::{adb::Adb,device::Device,gui_agent::{self,Control},amoo_screen::ScreenEvent,companion_gui};
fn main()->Result<()> {
    let args:Vec<String>=std::env::args().collect();
    let adb=Adb::discover()?;
    let device=Device::inspect(&adb,"STEPX_DEVICE_SERIAL")?;
    ensure!(device.serial=="STEPX_DEVICE_SERIAL","错误设备");
    if args.get(1).map(String::as_str)==Some("status") {println!("{}",companion_gui::call(&adb,&device,serde_json::json!({"op":"status"}))?);return Ok(());}
    let task=args.get(1).context("需要测试目标")?;
    let control=Control::default();
    let result=gui_agent::run(&adb,&device,"com.android.settings",task,&control,|event|match event{
        ScreenEvent::Screenshot{number,png}=>{let _=std::fs::write(format!("../diagnostics/gui-agent-20261023/screen-{number}.png"),png);},
        ScreenEvent::Step{number,description}=>println!("STEP {number}: {description}"),
        ScreenEvent::Status(s)=>println!("{s}")
    })?;
    println!("{}",serde_json::to_string(&result)?);
    ensure!(result.state==stepx_companion::amoo_gui::GuiRunState::Completed,"任务未完成");Ok(())
}
