use anyhow::{ensure,Context,Result};
use stepx_companion::{adb::Adb,device::Device,independent_gui,gui_agent::Control,amoo_screen::ScreenEvent};
fn main()->Result<()>{
    let args:Vec<String>=std::env::args().collect();
    let adb=Adb::discover()?;let device=Device::inspect(&adb,"STEPX_DEVICE_SERIAL")?;
    ensure!(device.serial=="STEPX_DEVICE_SERIAL","错误设备");
    if args.get(1).is_some_and(|s|s=="enable") {stepx_companion::companion_gui::enable(&adb,&device)?;return Ok(());}
    let goal=args.get(1).context("需要任务目标或 worker")?;
    let result=independent_gui::run(&adb,&device,goal,None,&Control::default(),|event| match event {
        ScreenEvent::Screenshot{number,png}=>{let _=std::fs::write(format!("../diagnostics/native-mcp-20261023/independent-screen-{number}.png"),png);},
        ScreenEvent::Status(s)=>println!("{s}"),ScreenEvent::Step{number,description}=>println!("STEP {number}: {description}")
    })?;
    println!("{}",serde_json::to_string(&result)?);
    ensure!(result.state==stepx_companion::amoo_gui::GuiRunState::Completed,"任务未确认完成");Ok(())
}
