# 原厂 GUI 执行器研究（2026-10-23）

当前状态：**尚未通过模型实际操作验收，不得作为已启用功能发布。**

## 已验证的边界

仅针对已授权 StepX PZ261A / Android 16 固件、固定原厂 Ability APK 摘要运行。备份和原始证据在 `diagnostics/native-gui-backup-20261022-1910/`。

`NativeGuiTaskHost` 使用独立、限时的 UID 1000 进程加载原厂 SDK；不替换原 APK，不修改分区或 SELinux，不运行原 Application.onCreate，不重启原厂 Ability/system_server。任务数据库仅在内存中，临时文件在带 PID、启动时间、boot ID 和 nonce 所有权证明的专属目录内。

研究伴侣 APK 由 `scripts/build-companion-app.sh --native-gui-probe` 单独生成。默认构建不包含研究 Provider，研究产物不覆盖 `desktop/assets/stepx-companion.apk`。

## 真机结果

- 原厂 SDK 能启动设置首页、截图并提交模型请求。这不等于模型已执行点击。
- 伴侣受保护控制接口的暂停、继续、停止均收到原厂回调；停止后自有任务、显示和资源已释放。证据：`fixed-settings-controls-fe0e1d5c.json` 与对应 `task-host-fe0e1d5cfbdf4062a345b076209b567f.json`。
- 默认模型请求反复收到 `msgType=4`、符合 ErrorResp 结构的 `code=10002`。错误原因在内存中匹配到原厂默认模型 ID、`instance` 和 `not_found`。观察器不保留错误正文、账号、令牌或签名图片地址。
- SDK 将该错误包当 ChatCompletionResponse 读取，得到空 body 后抛出 JSONException。最终模型动作数为 0。证据：`task-host-134a8d61c41f442686f9b3ffd8cb8486.json`。
- 这指向默认 GUI 模型实例查找失败；不能仅凭错误码断言账号无权限，也不能靠提高伴侣权限解决。
- 纯 Python 元数据读取已在真机通过 CRC/结构/稳定性校验：原配置存在，但没有保存当前模型选择，因此没有可直接复用的其他模型配置。证据：`original-model-metadata.json`。

## 已知失败路径

`NativeGuiModelPreferences` 尝试将原厂单一模型配置 MMKV 文件复制到隔离目录，以原厂 READ_ONLY_MODE 打开副本。真机独立研究进程发生 SIGSEGV，未发送 GUI 任务。**此原生读取路径不得重试或进入生产。**对应记录：`task-host-cc47a3185a2b45c080b47be31c6fd1df.json`。保留源码用于定位；`gui-native-task-host.py` 的启动摘要门已关闭，收集与恢复工具继续可用。

模型配置元数据应使用严格、有界、校验 CRC 的纯解析器读取；不得以“只读”选项为原生实现绝不会写内存或文件的证据。

该失败原生读取类已从 Host 调用和构建列表移除，仅保留源码作故障证据。崩溃进程的临时目录、DEX 和空结果文件已通过独立审计后删除，原失败记录保持不变。

## 本轮结束状态

已恢复研究前 `companion.before-probe.apk` 的精确字节，保留 UID 和用户数据，研究 Provider 已移除。15 项权限、Doze/OEM 豁免及系统配对服务回读通过。普通 Amoo 取得真实云端会话回执；自有测试租约建立和释放通过，结束时租约数为 0、唤醒锁已释放。没有重复进行完整扫码、长时间熄屏或手机重启验收。

原厂 Ability 和 system_server 进程、原始系统文件及显示基线均保持一致。只读诊断加载的类可能保留到手机重启，因此不宣称完成全局内存回滚。生产一键安装入口保持关闭，桌面版没有因本轮研究重新发布或重启。最终汇总为 `research-status.json`。

## 验收与回滚原则

`collect` 的 `ok=true` 只说明该研究进程资源清理通过，不能替代 GUI 行为成功，更不能替代全局回滚验收。必须分别核验：

1. 原厂文件摘要、进程身份、系统启动属性未变。
2. 目标研究进程已退出；仅删除其记录中匹配 inode 的 DEX/结果文件及 owner 匹配的临时目录。
3. 自有原厂任务、查询/主显示 watcher、虚拟显示、重定向引用、Provider 引用、数据库、MMKV 和云连接已释放。
4. 恢复研究前 **exact APK** `companion.before-probe.apk`，保留 UID/用户数据；回读原授权、配对服务和后台连接。
5. 真正“看屏→模型动作→页面变化→正确结果”通过后，才能集成一键安装、启停、暂停/继续和回滚 UI。

没有启动结果的崩溃记录必须保留为失败；不伪造 finished/collected 记录。崩溃回收走独立审计，要求精确原启动证明、进程已亡、结果文件确实为空和完整文件所有权链。

## 后续修复入口

原 `ModelConfigInner` 可读取 `model_config_prefs` 的 `current_model_id` 和 `model_map`。原厂单任务覆盖参数是 `gui_model_name`，但本地保存值或静态模型目录均不代表该账号实际可调用。普通 Amoo 自动对话模型验证不能替代 GUI 模型部署验证。

只有取得来源明确的有效 GUI 配置后，才考虑在独立研究任务中覆盖模型；不要猜部署 ID、绕过授权，或把另写的桌面 harness 称为原厂执行器。
