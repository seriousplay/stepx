# X 原生 Google 登录：真机失败定位

日期：2026-10-25。客户端：StepX Companion 0.2.28。

**结论：尚未修好，未通过原生登录验收，因此未将任何候选方案集成为“一键修复成功”。本轮没有使用网页版替代验证。**

## 复现环境

- StepX PZ261A / pandora，Android 16，172 固件。
- X：12.37.2-prod.01，versionCode 312372001，安装来源与发起安装者均为 Google Play。
- Google Play services：26.41.31；Play：53.6.15；账号保留。
- 自动日期、时间与时区均已启用。
- 直接启动 X 原生 `com.x.android.main.MainActivity`，在系统 Google 账号选择页点击“继续”。结果仍是“请使用官方 X 应用继续操作，或稍后重试”。

## 实际调用链

| 环节 | 证据 | 结果 |
| --- | --- | --- |
| Google 账号选择 | 系统 Credential Manager 展示账号确认页 | 正常 |
| 凭据返回 | 系统记录 `onFinalResponseReceived`，X 进程记录 `GetCredentialResponse returned from framework` | 已返回给 X |
| Play Integrity 服务绑定 | X 的 PlayCore 成功绑定商店 `ExpressIntegrityService` | 没有发生缺服务或绑定失败 |
| Integrity 对象获取 | X 内部日志记录 `Got attestationObj, sending to graphql` | 客户端拿到对象并提交 |
| X 验证 Integrity 对象 | `GenerateAttestationTokenV2` 返回 `GraphQLError(code=214, name=BadRequestError, path=[verify_play_integrity_attestation_v2], message=BadRequest: Validation Failed)` | 被 X 服务端拒绝 |
| X 验证硬件密钥 | `VerifyAndroidAttestationKeysMutation` 后记录 `server verification failed`，并记录 `server could not verify private keys` | 被 X 服务端拒绝 |
| 原生 Google SSO | `/onboarding/actions/sso` 前记录 `No verified keys for signing` | 缺少已验证的证明，界面报错 |

这些错误来自当前 X 自身的 Crashlytics 日志与非致命异常事件，而非独立测试应用的推测。X 在同一前台运行期间自动进行了重试，六次 Integrity 校验均返回 214，六次硬件证明均失败；本轮没有反复人工提交登录，也没有请求重置账号。

`214` 是 X GraphQL 的业务错误码，**不是 Google StandardIntegrityErrorCode**。不能把它误解成“Google 服务版本低”或“绑定权限不足”。

## X 实际提交的证书链

从 X 记录的公开证明证书解析出四级链：

1. Android Keystore Key
2. Stepx Android Attestation ECC Leaf
3. Stepx ECC Intermediate CA
4. Stepx Root CA

离线验证结果：四级签名验证通过；根证书与根公钥均不匹配本轮实时下载的 Google 官方两个信任根。这是 X 本次请求使用的证书链，不是另一个探针生成的替代样本。没有读取或导出 AndroidKeyStore 私钥。

Google 的[硬件密钥证明验证文档](https://developer.android.com/privacy-and-security/security-key-attestation)要求服务端验证证书链、信任根和吊销状态；[官方根证书列表](https://android.googleapis.com/attestation/root)不包含此 Stepx 根。

因此，已确认失败位于服务端的设备/应用证明校验阶段；Google 登录授权接口本身已走通。厂商自签链不受 Google 信任是明确的兼容缺口。X 未返回其全部判定字段，不能仅凭 214 进一步断言唯一失败条件、具体 Integrity verdict 或风控规则。

## 为什么没有再加入权限补丁

现有 Google 通用服务健康检查全部通过，153 项授权仍有效。继续授予普通应用权限、在手机添加受信任证书，均不能改变 X 服务器对硬件证明链的信任。更改客户端显示或跳过本地提示也不构成登录成功。

需由设备厂商解决受支持的硬件证明/远程密钥配置与固件兼容，或由 X 明确支持该设备证明链；修复后仍需重新验证原生 Google SSO。仅修改其中一项不能预先保证 X 接受。没有安装未经验证的认证补丁，没有替换其他设备的身份或密钥，没有降级/重签 X。

## 备份与收尾

- 保存当前 X 的 4 个 APK 及哈希、安装来源、时间和网络设置。
- 停止 X 后，在其自身 UID 与 SELinux 上下文备份 CE/DE 应用文件，共 77 个文件；手机保留私有备份，电脑副本的 SHA-256 与手机一致。
- 保留此前系统账号与应用数据快照；没有清除账号、缓存、应用数据或 AndroidKeyStore。
- 本轮仅进行读取、备份和正常界面登录；未写入修复配置，因此无需覆盖恢复原数据。备份文件继续保留。
- 诊断结束检查启动属性、系统进程、Google 账号数量与现有授权。私有 APK、账号数据备份及原始日志已排除 Git，不能随公开报告上传。

本机原始证据保存在 `diagnostics/x-native-login-20261025/`。对外反馈可使用本报告，不要附带原始账号数据、完整证明链、用户标识或登录凭据。

## 厂商与 X 侧复测条件

1. 提供适用于 PZ261A / 172 后续固件的正式硬件证明或远程密钥配置方案。
2. 在正版 X 中重新发起 Google 登录，确认硬件证明验证和 `GenerateAttestationTokenV2` 均成功。
3. 确认原生 SSO 后进入已登录主界面；冷启动和手机重启后保留登录。
4. 满足上述实际条件后，才将可复现、可备份和回退的本机部分纳入一键修复。

## GitHub 现成补丁与本机加载条件复核（2026-10-25）

本节是用户要求继续检查 PIF、TrickyStore 后的新增实测。**没有安装这些模块，不能将下面的原生登录复测描述成“打补丁后的结果”。**

### 上游候选

| 候选 | 本轮核查 | 对本机的限制 |
| --- | --- | --- |
| [PlayIntegrityFork v18](https://github.com/osm0sis/PlayIntegrityFork/releases/tag/v18) | 下载官方发布包，SHA-256 与发布页 digest 一致；阅读当前 README 与安装脚本 | 需要 Magisk / KernelSU / APatch 和 Zygisk。上游区分 Android 13 之前与之后的完整性规则，并未承诺修复本机 X 登录 |
| [TrickyStore 1.4.1](https://github.com/5ec1cff/TrickyStore/releases/tag/1.4.1) | 下载官方发布包，SHA-256 与发布页一致；核查 `customize.sh`、`service.sh` 和 `sepolicy.rule` | 安装脚本要求受支持的 root 模块环境；需要 keystore 服务挂接及 SELinux 规则支持，不能用普通 APK 安装替代 |
| [KOWX712/PlayIntegrityFix](https://github.com/KOWX712/PlayIntegrityFix/tree/inject_s) | 核对当前维护分支说明 | 同样需要 root + Zygisk。维护者列出 `spoofVendingSdk` 在 Android 14+ 可能导致账号状态空白、更新失败或商店崩溃；不将该选项作为通用登录修复 |
| `ShaggyLorean/x-attestation-fix` | 从 [microG 同类 X 问题](https://github.com/microg/GmsCore/issues/3504)和 [Piko 登录问题](https://github.com/crimera/piko/issues/1494)的评论找到；GitHub API、直接仓库地址和 README 均返回 404 | 搜索缓存中的构建脚本标明它是 LSPosed 模块。当前无法取得完整、可审核的发布来源，且本机没有 LSPosed；未安装来源不明的镜像包 |

官方包哈希：

- PlayIntegrityFork v18：`625ebe65631f761e1521009f10a205d74255adf2e35732923d65106643cad9a6`
- TrickyStore 1.4.1：`2f5e73fcba0e4e43b6e96b38f333cbe394873e3a81cf8fe1b831c2fbd6c46ea9`

两个发布包与此前检查的版本相同，没有发现已验证支持 PZ261A / 172 固件的新增修复。TrickyStore 发布包仍包含公开软件证明示例，其 EC 中间证书已于 2026-01-08 过期；安装模块本身不会获得厂商的有效硬件证明材料。没有下载或使用其他设备泄露的证明私钥。

### 本机安装条件实测

- 当前仍为 Android 16 / 172，Bootloader locked，AVB green，SELinux Enforcing。
- 没有发现 Magisk、KernelSU、Zygisk、LSPosed 或 TrickyStore 的运行进程及管理应用；内核的 `kernelsu` / `ksu` 模块路径不存在。
- 在现有高权限诊断上下文读取 capability：`CAP_SYS_MODULE` 在 effective、permitted、bounding 三组中均不存在。本轮没有调用模块加载 syscall，也没有重做自研加载器。
- `/data/adb` 相关模块目录访问返回 `EACCES`；六个 `boot`、`init_boot`、`vbmeta` A/B 分区的只读打开也返回 `EACCES`。应用数据备份不等于可用于刷写恢复的启动分区备份。
- [Magisk 官方首次安装流程](https://github.com/topjohnwu/Magisk/blob/master/docs/install.md)要求解锁引导加载程序。没有在本轮执行解锁、刷写、关闭 SELinux 或更改设备密钥。

因此，现成模块的加载条件尚不满足，不能通过复制 ZIP、安装管理器 APK 或增加普通 Android 权限补齐。当前没有符合“不清账号数据、失败可回滚”要求的已验证安装路径。

### 备份、原生复测和恢复

- 在停止 X 后重新备份当前 CE/DE 应用文件，共 76 个文件、240291 字节；手机私有副本与电脑副本 SHA-256 一致。保留之前的备份，没有覆盖它。
- 直接启动已安装的官方 X `12.37.2-prod.01`，在 Android Credential Manager 界面点击一次“继续”；Google 凭据成功返回，原生 X 再次显示“请使用官方 X 应用继续操作，或稍后重试”。读取本次会话内部日志的错误计数，仍有 6 次 `214 / Validation Failed` 和 6 次硬件密钥验证失败；这是 X 的自动重试，不是重复人工提交。没有用网页或第三方客户端替代验收。
- 未安装候选模块、未修改认证配置、未清除账号/应用数据，因此没有需要撤回的模块补丁。诊断临时启动属性已恢复，现有 Google 权限基线与系统进程保持不变。最终检查 Google 通用健康状态正常、账号数量不变、X 四个 APK 哈希不变；测试后关闭 X，避免继续自动重试。
- 本轮原始备份、发布包及设备诊断保存在私有 `diagnostics/x-module-review-20261025/`，不随公开文档提交。仍未达到加入“一键修复”的成功验收条件。
