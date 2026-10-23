# StepX伴侣背屏权限

该适配仅支持 Google 适配流程已经核验的 StepX PZ261A 固件和当前授权设备。
APK 仍是普通 `/data/app` 应用，不会添加系统或特权应用标记，也不会修改
SELinux、Google 应用权限或开机脚本。

`scripts/build-companion-permissions.sh` 编译独立 DEX。安装前运行：

```sh
python3 scripts/companion-permissions.py preflight --apk desktop/assets/stepx-companion.apk
```

完成正常 APK 安装后运行 `grant --apk ...`，只为 `com.stepx.companion.app`
授予 `com.stepos.permission.MANAGE_BACK_SCREEN`。固件/native 哈希、APK 包名、
APK 签名、手机已安装包的签名和 UID 均被检查。授权通过现有 RootBridge
执行有时限的 system_server Java 调用，修改受支持固件的 PermissionManager
状态并回读确认；不执行原 Google Java payload。普通 sudo RootBridge 处于受
seccomp 限制的 zygote 上下文，不能直接进入 Android 宿主，因此授权使用现有
`root_worker.py` 的有时限 init 上下文：临时指定一次性启动器，启动后先立即
恢复两个启动属性，再执行独立授权。该过程会短暂重启 StepClaw；原有启动器
字节和 Google 适配器均不修改。执行前后检查启动属性、启动器哈希、
system_server PID、RootBridge 可用性和 Google 权限基线。

此固件有按包名设置的 USB 安装门控。正常安装步骤应使用 `prepare-install --apk ...`
临时允许 `usb_install_item_com.stepx.companion.app`，随后无论安装成功或失败都在
`finally` 中执行 `finish-install`。准备动作会先保存原值，完成动作回读确认恢复
原值；从不改变 USB 安装总开关或其他应用的设置。若发现尚未恢复的安装记录，
必须先执行恢复，不能覆盖基线。

该固件的 StepOS 冻结器会在约 6 秒后冻结普通应用，即使应用已运行前台服务、
持有正常唤醒锁。`runtime --apk ...` 会先核验固件、包签名及 UID，读取
`dumpsys activity freeze whitelist`，仅在伴侣不在白名单时执行一次
`dumpsys activity freeze whiteAdd com.stepx.companion.app`。若当前 UID 已冻结，
再执行一次 `doUnfreeze <已验证UID>` 并回读。没有定时 ADB 保活，不修改全局
冻结开关、其他应用、Doze 配置或开机脚本。`grant` 的新授权及已有授权分支
均包含此适配，desktop 每次启动伴侣功能前也应调用 `runtime`。

白名单新增前保存基线，按设备、签名、UID、boot_id、system_server PID 隔离；
重复调用无写操作。名单只对当前 system_server 有效；重启后需连接 desktop
重新适配。单包后台能力在退出游戏时保留，待机仍不持有唤醒锁。

`inspect --apk ...` 只读取当前安装状态、权限及 OEM 后台状态，
`revoke --apk ...` 恢复第一次授权记录中的 flags，并仅撤销本工具新增的
当前会话后台豁免。原有白名单不会被撤销；发现外部修改会保留恢复记录。
回滚记录在 `$XDG_STATE_HOME/stepx-companion/permissions`
（默认 `~/.local/state/stepx-companion/permissions`），不会复用 Google 的状态目录。

当前没有安装自动开机授权器，因此状态固定带 `reboot_verified: false`。
手机重启、更新系统或重新安装 APK 后必须重新检测权限，不得把当前成功显示为
永久授权。普通应用能否实际调用背屏 Binder 和输出灯珠，仍由 App 实机回读验证。
