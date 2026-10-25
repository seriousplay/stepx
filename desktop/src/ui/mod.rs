//! Desktop shell for StepX Companion. Layout and visual rules: `desktop/DESIGN.md`.
//!
//! Module map:
//! - `theme`   design tokens (spacing, type, colour, radius) and font loading
//! - `widgets` reusable components built only from tokens
//! - `shell`   sidebar, page header and status bar
//! - `pages`   one file per page
//! - `actions` input handling and background jobs

mod actions;
mod desktop_update_state;
mod github_stars;
#[cfg(all(feature = "ui-capture", target_os = "linux"))]
mod shell_details_check;
mod shortcuts;
use desktop_update_state::{DesktopUpdateAction, DesktopUpdateState};
mod adb_onboarding;
mod amoo_composer;
mod amoo_display;
mod amoo_sync;
mod amoo_ui_state;
mod connection_watch;
mod context_menu;
mod expansion_state;
mod reply_motion;
mod speech_state;
mod text_interaction;
use amoo_ui_state::{AmooUiAction, AmooUiOutcome, AmooUiState};
mod backscreen_state;
#[cfg(feature = "ui-capture")]
mod chat_first_check;
#[cfg(feature = "ui-capture")]
mod compact_live_check;
#[cfg(feature = "ui-capture")]
mod composer_check;
#[cfg(feature = "ui-capture")]
mod demo;
#[cfg(feature = "ui-capture")]
mod enter_check;
mod icons;
mod interaction;
mod led_grid;
mod mini_modes;
mod mirror_controls;
mod motion;
mod pages;
mod perf;
#[cfg(feature = "ui-capture")]
mod performance_check;
mod preview_cache;
mod push_state;
mod push_toast;
#[cfg(feature = "ui-capture")]
mod responsive_check;
#[cfg(feature = "ui-capture")]
mod responsive_live_check;
#[cfg(feature = "ui-capture")]
mod scenario_check;
#[cfg(feature = "ui-capture")]
mod settings_check;
mod settings_state;
mod shell;
mod smooth_scroll;
#[cfg(feature = "ui-capture")]
mod workspace_check;
use settings_state::{SettingsAction, SettingsUi};
mod studio_lab_state;
mod studio_state;
mod theme;
mod update_state;
mod widgets;
#[cfg(feature = "ui-capture")]
mod window_memory_check;
mod work_state;
mod workspace_state;
use backscreen_state::{BackScreenAction, BackScreenUiState};
use push_state::{PushAction, PushUiState, ShellMode};
use studio_lab_state::{StudioLabAction, StudioLabOutcome, StudioLabUiState};
use studio_state::{StudioAction, StudioOutcome, StudioUiState};
use update_state::{UpdateAction, UpdateState};
use work_state::{WorkAction, WorkOutcome, WorkUiState};
use workspace_state::{WorkspaceAction, WorkspaceUi};

use anyhow::Result;
use facloud_ui::{
    elements::*,
    event::{DispatchedEvent, Event},
    fonts::{Cache as FontCache, FamilyId},
    platform::{self, WindowBounds},
    AppContext, Element, Entity, SingletonEntity, View, ViewContext, ViewHandle,
};
use pathfinder_geometry::vector::{vec2f, Vector2F};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    path::PathBuf,
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};
use stepx_companion::{
    adb::Adb,
    amoo,
    apps::{self, AppInfo},
    bridge::{self, AmooSnapshot, Capabilities, SmsPlan, SmsSendStatus},
    companion_app::{self, CompanionStatus},
    device::{
        default_express_parcels, default_system_services, parse_devices, BackScreenPattern,
        CallRecord, ContactEntry, Device, ExpressParcelInfo, GoogleSuiteStatus, SystemServiceInfo,
        Transport,
    },
    mirror::MirrorSession,
    openclaw::{self, RealAmooData},
    pairing::{self, Preferences, QrSession, Service},
    phone,
    sms::Message,
    storage::{self, Entry, Trashed},
    toolbox::{self, StorageReport, ToolOperation},
};
use theme::{Palette, ThemeMode};

// Indices into `Companion::fields`.
const F_PAIR_ADDR: usize = 0;
const F_PAIR_CODE: usize = 1;
const F_CONNECT: usize = 2;
const F_SMS_TO: usize = 3;
const F_SMS_BODY: usize = 4;
const F_AMOO: usize = 5;
const F_SMS_SEARCH: usize = 6;
const F_FILE_SEARCH: usize = 7;
const F_APP_SEARCH: usize = 8;
const F_FILE_NAME: usize = 9;
const F_OV_TEXT: usize = 10;
const F_BACK_PROMPT: usize = 11;
const F_BACK_MARQUEE: usize = 12;
const F_STUDIO_NAME: usize = 13;
const F_STUDIO_URL: usize = 14;
const F_PHONE_DIAL: usize = 15;
const F_CONTACT_SEARCH: usize = 16;
const F_AMOO_SEARCH: usize = 17;
const F_WORK_PROMPT: usize = 18;
const F_WORK_SEARCH: usize = 19;
const F_WORK_CAP_SEARCH: usize = 20;
const F_WORK_AGENT_NAME: usize = 21;
const F_AMOO_GUI_APP: usize = 22;
const F_STUDIO_SEARCH: usize = 23;
const F_LAB_ENDPOINT: usize = 24;
const F_LAB_KEY: usize = 25;
const F_LAB_MODEL: usize = 26;
const F_LAB_PROMPT: usize = 27;
const F_PUSH_SEARCH: usize = 28;
const F_PUSH_REPLY: usize = 29;
const F_IM_REPLY: usize = 30;
const F_TOAST_REPLY: usize = 31;
const FIELD_COUNT: usize = 32;

const FILES_PER_PAGE: usize = 100;

struct DesktopAssets;
impl facloud_ui::assets::AssetProvider for DesktopAssets {
    fn get(&self, path: &str) -> Result<std::borrow::Cow<'_, [u8]>> {
        match path {
            "stepx_companion_icon.png" => Ok(std::borrow::Cow::Borrowed(include_bytes!(
                "../../assets/stepx_companion_icon.png"
            ))),
            "neo_front_frame.png" => Ok(std::borrow::Cow::Borrowed(include_bytes!(
                "../../assets/neo_front_frame.png"
            ))),
            "neo_rear_housing.png" => Ok(std::borrow::Cow::Borrowed(include_bytes!(
                "../../assets/neo_rear_housing.png"
            ))),
            "stepx_home_screen.png" => Ok(std::borrow::Cow::Borrowed(include_bytes!(
                "../../assets/stepx_home_screen.png"
            ))),
            "amoo_desktop_icon.png" => Ok(std::borrow::Cow::Borrowed(include_bytes!(
                "../../assets/stepx_companion_icon.png"
            ))),
            "amoo_icon.png" => Ok(std::borrow::Cow::Borrowed(include_bytes!(
                "../../assets/amoo_icon.png"
            ))),
            _ => icons::asset(path)
                .map(std::borrow::Cow::Borrowed)
                .ok_or_else(|| anyhow::anyhow!("找不到内置资源：{path}")),
        }
    }
}

pub fn run() -> Result<()> {
    let capture = std::env::args()
        .position(|arg| arg == "--capture-ui")
        .and_then(|i| std::env::args().nth(i + 1))
        .map(PathBuf::from);
    let instance_dir = if capture.is_some() {
        std::env::temp_dir().join(format!("stepx-companion-capture-{}", std::process::id()))
    } else {
        stepx_companion::resources::user_config().join("stepx-companion/runtime")
    };
    let stepx_companion::single_instance::Claim::Primary(instance) =
        stepx_companion::single_instance::Instance::claim(&instance_dir)?
    else {
        return Ok(());
    };
    let activation = instance.activation_requests();
    let mut builder = platform::AppBuilder::new(
        platform::AppCallbacks {
            // The framework only tears down closed windows when this callback is set.
            on_window_will_close: Some(Box::new(|_, _| {})),
            ..Default::default()
        },
        Box::new(DesktopAssets),
        None,
    );
    #[cfg(target_os = "linux")]
    {
        use facloud_ui::platform::linux::AppBuilderExt;
        builder.set_window_class("stepx-companion".into());
        // Share XWayland coordinates with scrcpy so its sidebar can follow it.
        if std::env::var_os("DISPLAY").is_some() {
            builder.force_x11(true);
        }
        builder.set_window_icon_png(std::borrow::Cow::Borrowed(include_bytes!(
            "../../assets/stepx_companion_icon.png"
        )));
    }
    let size = mini_modes::WindowMemory::new().preferences.full;
    let (default_w, default_h) = (size.width, size.height);
    builder.run(move |ctx| {
        ctx.add_window(
            facloud_ui::AddWindowOptions {
                title: Some("StepX Companion".into()),
                // Capture runs may ask for a taller window so a long panel is
                // not cut off at the fold.
                window_bounds: WindowBounds::ExactSize(vec2f(
                    std::env::var("STEPX_CAPTURE_WIDTH")
                        .ok()
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(default_w),
                    std::env::var("STEPX_CAPTURE_HEIGHT")
                        .ok()
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(default_h),
                )),
                ..Default::default()
            },
            move |ctx| Root {
                child: ctx.add_typed_action_view(|ctx| Companion::new(ctx, capture, activation)),
            },
        );
    })?;
    drop(instance);
    Ok(())
}

struct Root {
    child: ViewHandle<Companion>,
}
impl Entity for Root {
    type Event = ();
}
impl facloud_ui::TypedActionView for Root {
    type Action = ();
}
impl View for Root {
    fn ui_name() -> &'static str {
        "StepXRoot"
    }
    fn render(&self, _: &AppContext) -> Box<dyn Element> {
        ChildView::new(&self.child).finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Overview,
    Expansion,
    BackScreen,
    Amoo,
    Account,
    About,
    Settings,
    Work,
    Studio,
    Sms,
    Notifications,
    Phone,
    Files,
    Apps,
    Tools,
    Connect,
}

impl Page {
    fn title(self) -> &'static str {
        match self {
            Self::Overview => "首页",
            Self::Expansion => "存储扩容",
            Self::BackScreen => "背屏",
            Self::Amoo => "Amoo",
            Self::Account => "账号与用量",
            Self::About => "设置",
            Self::Settings => "设置",
            Self::Work => "Amoo",
            Self::Studio => "Amoo",
            Self::Sms => "消息与短信",
            Self::Notifications => "手机推送",
            Self::Phone => "电话",
            Self::Files => "文件",
            Self::Apps => "应用",
            Self::Tools => "小工具",
            Self::Connect => "连接设备",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Overview => "◈",
            Self::Expansion => "💾",
            Self::BackScreen => "❖",
            Self::Amoo => "✦",
            Self::Account => "◉",
            Self::About => "◉",
            Self::Settings => "⚙",
            Self::Work => "💼",
            Self::Studio => "⚡",
            Self::Sms => "✉",
            Self::Notifications => "🔔",
            Self::Phone => "☎",
            Self::Files => "▤",
            Self::Apps => "⊞",
            Self::Tools => "⊟",
            Self::Connect => "⇄",
        }
    }

    /// Pages that need a connected phone to show anything useful.
    fn needs_device(self) -> bool {
        !matches!(
            self,
            Self::Connect
                | Self::Tools
                | Self::Work
                | Self::Amoo
                | Self::Account
                | Self::About
                | Self::Settings
                | Self::Studio
                | Self::Expansion
        )
    }

    /// Pages that manage their own 0-padding panes (split columns).
    fn fills_height(self) -> bool {
        matches!(
            self,
            Self::Settings
                | Self::About
                | Self::Amoo
                | Self::Work
                | Self::Studio
                | Self::Sms
                | Self::Notifications
                | Self::Phone
                | Self::Account
        )
    }

    fn from_arg(arg: &str) -> Self {
        match arg {
            "backscreen" | "back" => Self::BackScreen,
            "amoo" => Self::Amoo,
            "work" | "amoo-work" => Self::Amoo,
            "studio" => Self::Amoo,
            "sms" | "chat" | "im" => Self::Sms,
            "notifications" | "push" | "notify" => Self::Notifications,
            "phone" | "call" => Self::Phone,
            "files" => Self::Files,
            "expansion" => Self::Expansion,
            "apps" => Self::Apps,
            "tools" | "toolbox" => Self::Tools,
            "account" => Self::Account,
            "about" => Self::About,
            "settings" => Self::Settings,
            "pair" | "connect" => Self::Connect,
            _ => Self::Overview,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ToolPage {
    Updates,
    Storage,
    Google,
    Companion,
    Neo,
    Apk,
    Screen,
    Services,
    InputMethod,
    Device,
}
impl ToolPage {
    const ALL: [Self; 10] = [
        Self::Updates,
        Self::Storage,
        Self::Google,
        Self::Companion,
        Self::Neo,
        Self::Apk,
        Self::Services,
        Self::InputMethod,
        Self::Screen,
        Self::Device,
    ];
    fn title(self) -> &'static str {
        match self {
            Self::Updates => "手机系统更新",
            Self::Storage => "释放空间",
            Self::Google => "Google Play",
            Self::Companion => "StepX伴侣",
            Self::Neo => "内置 GUI Agent",
            Self::Apk => "APK 安装",
            Self::Screen => "截图与投屏",
            Self::Services => "系统服务",
            Self::InputMethod => "手机输入法",
            Self::Device => "设备检测",
        }
    }
    fn description(self) -> &'static str {
        match self {
            Self::Updates => "查询手机 OTA、测试渠道和安装条件。",
            Self::Storage => "清理缓存、重复文件，压缩文档并在需要时还原。",
            Self::Google => "检测 Google 服务，安装适配套件或打开 Play 商店。",
            Self::Companion => "安装手机伴侣，管理后台连接、扫码配对与背屏权限。",
            Self::Neo => "更新 StepX伴侣内置 GUI Agent，手机独立看屏、规划与执行。",
            Self::Apk => "从电脑选择 APK，安装到当前连接的手机。",
            Self::Screen => "截取手机画面，或在独立窗口中操作手机。",
            Self::Services => "查看可管理的系统服务，按需停用或恢复。",
            Self::InputMethod => "安装小企鹅中文输入法，切换默认并停用搜狗；支持恢复。",
            Self::Device => "读取设备、电量与存储信息，复制检测结果。",
        }
    }
    fn category(self) -> u8 {
        match self {
            Self::Storage => 1,
            Self::Google | Self::Neo | Self::Apk | Self::Services | Self::InputMethod => 2,
            _ => 3,
        }
    }
    fn from_arg(value: &str) -> Option<Self> {
        match value {
            "updates" => Some(Self::Updates),
            "storage" => Some(Self::Storage),
            "google" => Some(Self::Google),
            "companion" => Some(Self::Companion),
            "neo" | "gui-agent" => Some(Self::Neo),
            "apk" => Some(Self::Apk),
            "screen" => Some(Self::Screen),
            "services" => Some(Self::Services),
            "input-method" => Some(Self::InputMethod),
            "device" => Some(Self::Device),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum PairTab {
    #[default]
    Qr,
    Code,
    Manual,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SmsFilter {
    #[default]
    All,
    Incoming,
    Outgoing,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum FileSort {
    #[default]
    Name,
    Size,
    Kind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FileModal {
    NewFolder,
    Rename(usize),
}

#[derive(Clone, Debug)]
enum Action {
    ReviewConfirm,
    ReviewCancel,
    ReviewHistory,
    Expansion(expansion_state::ExpansionAction),
    AmooUi(AmooUiAction),
    AmooToggleSchedules,
    AmooToggleDetails,
    AmooOpenScheduledSession(String),
    // Navigation & appearance & window
    Page(Page),
    Push(PushAction),
    Work(WorkAction),
    Workspace(WorkspaceAction),
    BackScreen(BackScreenAction),
    Updates(UpdateAction),
    DesktopUpdate(DesktopUpdateAction),
    CycleTheme,
    Settings(SettingsAction),
    QuitApplication,
    ContextRefresh,
    ScanStorage,
    OpenStorageSettings,
    ProbeSystemCompression,
    PrepareToolOperation(ToolOperation),
    ConfirmToolOperation,
    CancelToolOperation,
    LocateToolFile(String),
    ToolTab(u8),
    ToolResultPage(usize),
    MinimizeWindow,
    MaximizeWindow,
    CloseWindow,
    // Connection
    Refresh,
    Select(String),
    Forget,
    SetPreferredDevice(String),
    RemoveRememberedDevice(String),
    ConnectEndpoint(String),
    PairTab(PairTab),
    ToggleShowSecret,
    PairCode,
    Connect,
    Qr,
    // Text input
    Focus(usize),
    InputPointer(usize, usize, bool, u32),
    InputPointerUp,
    ExtendSelection(i8),
    MessageSelection(Option<String>),
    ClearField(usize),
    FocusSearch,
    NextField(bool),
    SubmitFocused,
    Escape,
    Insert(String),
    Preedit(String),
    Backspace,
    BackspaceLine,
    Delete,
    Left,
    Right,
    Home,
    End,
    SelectAll,
    CopyInput,
    UndoInput,
    RedoInput,
    ContextOpen(context_menu::Target, Vector2F),
    ContextClose,
    ContextNavigate(i8),
    ContextActivate,
    ContextRun(Box<Action>),
    Speak(String, String),
    CutInput,
    Paste,
    // Device & Overview
    Summary,
    Mirror,
    StopMirror,
    Screenshot,
    SaveScreenshot,
    Key(u32),
    OverviewBackPreview(bool),
    SendFocusText,
    // Amoo
    Amoo,
    AmooSnapshot,
    AmooNewChat,
    AmooPrepare,
    AmooSend,
    AmooTemplate(String),
    AmooSelectCap(usize, String),
    AmooSelectAgent(usize),
    AmooSelectSession(usize),
    AmooToggleTts,
    AmooToggleVoicePlay,
    AmooToggleThinking,
    AmooPushArtifactBolt,
    AmooToggleInvoiceDetail,
    AmooConfirmSafety(bool),
    AmooRetryReply,
    AmooRetryMessage { session_key: String, text: String },
    AmooToggleToolRecords(String),
    AmooTogglePlus,
    AmooAttachFile(bool),
    AmooDropFiles(Vec<PathBuf>),
    AmooAttachPhoneAlbum,
    AmooRemoveAttachment(usize),
    AmooToggleRecording,
    AmooCancelRecording,
    AmooSendRecording,
    AmooToggleMemory,
    AmooCycleStyle,
    AmooCycleCommute,
    AmooAddMemory,
    AmooUseMemory(String),
    AmooToggleSkillManage,
    AmooClickSkill(usize),
    AmooCycleSafetyPolicy,
    AmooUndoSafety,
    AmooExportMemory,
    AmooOpenMemory,
    AmooOpenPersona,
    AmooOpenSafety,
    AmooOpenModels,
    AmooOpenVoiceSettings,
    AmooOpenTasks,
    AmooResetSession,
    ReadVisible,
    AppendVisibleLine(String),
    Studio(StudioAction),
    StudioLab(StudioLabAction),
    StudioOpenUrl(String),
    // SMS
    ReadSms,
    OpenSms,
    SmsFilter(SmsFilter),
    SelectSim(Option<i32>),
    SelectSmsThread(Option<String>),
    NewSms,
    PrepareDirectSms,
    ConfirmDirectSms,
    CancelDirectSms,
    CheckSmsStatus(String),
    DraftSms,
    // Phone & Contacts
    ReadPhone,
    PhoneFilterMissed(bool),
    PhoneSelectContact(usize),
    PhoneDialAppend(String),
    PhoneDialBackspace,
    PhoneDialClear,
    PhoneCall(String),
    PhoneConfirmCall,
    PhoneCancelCall,
    PhoneSelectSim(i32),
    PhoneSmsTo(String),
    // Files
    Browse(String),
    FileSort(FileSort),
    SelectFile(usize),
    ActivateFile(usize),
    OpenFile(usize),
    DownloadFile(usize),
    CompactChat,
    MoveFileSelection(isize),
    ActivateSelectedFile,
    ParentDirectory,
    OpenNewFolder,
    OpenRename,
    RenameFile(usize),
    TrashFile(usize),
    SubmitFileModal,
    CancelFileModal,
    FilePage(usize),
    Upload,
    TransferFiles(Vec<PathBuf>, Option<String>),
    Download,
    Trash,
    DeleteFile,
    ConfirmTrash,
    CancelTrash,
    Restore,
    // Apps & Privileged System Management
    Apps,
    ToggleSystemApps(bool),
    ExportApp(String),
    AppSettings(String),
    RequestUninstallApp(String),
    CancelUninstallApp,
    ConfirmUninstallApp(String),
    InstallApk,
    OpenApp(String),
    ForceStopApp(String),
    ToggleSystemService(String, bool),
    DebloatPresetServices,
    RestoreAllSystemServices,
    SwitchInputMethodFcitx5(bool),
    InspectInputMethod,
    OpenTool(Option<ToolPage>),
    ToolCategory(u8),
    PrepareUtility(Box<Action>),
    ConfirmUtility,
    CancelUtility,
    InspectServices,
    OpenInputSettings,
    GoogleSuiteInstall,
    GoogleSuiteInspect,
    GoogleSuiteMaintain,
    GoogleSuiteRollback,
    GoogleSuiteSnapshot,
    CompanionInspect,
    CompanionInstall,
    NeoInspect,
    NeoInstall,
    NeoActivate,
    NeoRestore,
    NeoOpenPhone,
    NeoIme(bool),
    CompanionBackground(bool),
    CompanionSendFiles,
    CompanionReceiveFolder,
    ConnectGuide(Option<u8>),
    AdbGuide(Option<u8>),
    AdbGuideContinue,
    GrantDeepAmooPerms,
    GrantDeepBackScreenPerms,
    BackScreenToggleHwBreath,
    // Amoo Desktop Experience Enhancements
    AmooSelectModel(usize),
    AmooToggleLightbox,
    AmooToggleLightboxZoom,
    AmooSaveLightboxImage,
    AmooToggleExpressCard,
    AmooQueryExpress,
    AmooPushPickupCode(String),
    AmooTogglePcControl,
    AmooToggleBgKeepAlive,
    AmooExecPcCommand(String),
    // Phone extra
    PhoneHangup,
    PhoneCreateContact,
    // Files extra
    FilePushToBackScreen(String),
    // Clipboard
    Copy,
    CopyText(String, String),
    ClearContent,
}

type ReadResult<T> = std::result::Result<Vec<T>, String>;

fn read_result<T>(result: Result<Vec<T>>) -> ReadResult<T> {
    result.map_err(|error| format!("{error:#}"))
}

enum Outcome {
    InputMethod(stepx_companion::input_method::Status, String),
    AmooUi(AmooUiOutcome),
    Work(WorkOutcome),
    StorageScanned(String, StorageReport, Option<toolbox::SystemCompression>),
    SystemCompression(String, toolbox::SystemCompression),
    ToolOperated(
        String,
        String,
        toolbox::OperationResult,
        Option<StorageReport>,
    ),
    Sms(ReadResult<Message>, Option<Capabilities>),
    SmsPrepared(SmsPlan),
    SmsSent(SmsSendStatus, ReadResult<Message>),
    SmsStatus(SmsSendStatus),
    PhoneCallPrepared(phone::DialPlan),
    DialSubmitted(String, String),
    CallHangup(String),
    AmooState(AmooSnapshot, String, bool),
    AmooRealData(RealAmooData, String),
    AmooObservationUnavailable,
    AmooCreated(String),
    AmooSent(amoo::PendingReply),
    WorkspaceDelivered(String, PathBuf, String, std::result::Result<String, String>),
    Transports(Vec<Transport>, Vec<Service>),
    Reattached(Device),
    Selected(
        Device,
        String,
        Option<Capabilities>,
        Option<RealAmooData>,
        ReadResult<Message>,
        ReadResult<CallRecord>,
        ReadResult<ContactEntry>,
    ),
    /// Content to show on the page that started the job (e.g. screen text).
    Text(String),
    /// Result that only needs a line in the status bar.
    Notice(String),
    Summary(String, Option<Capabilities>),
    Files(String, Vec<Entry>, Option<String>),
    Apps(Vec<AppInfo>),
    ApkInstalled(String, Vec<AppInfo>),
    Mirror(String, String, MirrorSession),
    Screenshot(tempfile::TempDir, PathBuf),
    Trash(Trashed),
    Restored,
    Qr(Option<String>),
    Paired(String),
    BackScreenImportedImage(PathBuf),
    Studio(StudioOutcome),
    StudioLab(StudioLabOutcome),
    PhoneData(ReadResult<CallRecord>, ReadResult<ContactEntry>),
    GoogleSuite(GoogleSuiteStatus, CompanionStatus, String),
    CompanionStatus(CompanionStatus, String),
    NeoStatus(stepx_companion::neo_agent::NeoStatus),
    SystemServices(Vec<SystemServiceInfo>, String),
    DeepAmooGranted(String),
    DeepBackScreenGranted(String),
}
type Job = Box<dyn FnOnce(&Adb) -> Result<Outcome> + Send>;

#[derive(Default)]
struct Input {
    text: String,
    cursor: usize,
    all: bool,
    preedit: String,
    anchor: Option<usize>,
    dragging: bool,
    undo: Vec<(String, usize)>,
    redo: Vec<(String, usize)>,
    last_edit: Option<Instant>,
}

impl Input {
    fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
        self.all = false;
        self.preedit.clear();
        self.anchor = None;
        self.dragging = false;
        self.undo.clear();
        self.redo.clear();
        self.last_edit = None;
    }

    fn set(&mut self, text: &str) {
        self.clear();
        self.insert(text);
    }

    fn insert(&mut self, text: &str) {
        self.delete_selection();
        let clean: String = text.chars().filter(|c| *c != '\0' && *c != '\r').collect();
        if self.text.len() + clean.len() > 16_384 {
            return;
        }
        self.text.insert_str(self.cursor, &clean);
        self.cursor = (self.cursor + clean.len()).min(self.text.len());
        self.preedit.clear();
    }

    fn previous(&self) -> usize {
        self.text[..self.cursor.min(self.text.len())]
            .char_indices()
            .last()
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    fn next(&self) -> usize {
        let cur = self.cursor.min(self.text.len());
        self.text[cur..]
            .chars()
            .next()
            .map(|c| cur + c.len_utf8())
            .unwrap_or(cur)
    }
}

/// One conversation in the SMS list.
struct SmsThread {
    address: String,
    count: usize,
    last_ms: i64,
    last_body: String,
    unread: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AmooAttachment {
    name: String,
    size_label: String,
    is_image: bool,
    local_path: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SafetyDecision {
    #[default]
    Pending,
    Approved,
    Rejected,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum AmooChatItem {
    Attachment {
        name: String,
        kind: String,
        mime: String,
        url: String,
        size_bytes: Option<u64>,
        path: String,
    },
    ToolRecords {
        key: String,
        records: Vec<amoo_display::ToolRecord>,
    },
    UserText {
        attachment: Option<(String, String, bool)>,
        text: String,
    },
    UserVoice {
        transcript: String,
        duration_sec: u32,
    },
    AssistantRich {
        thinking_time: String,
        thinking_steps: Vec<String>,
        tools_summary: String,
        text: Option<String>,
        has_bolt_artifact: bool,
        has_invoice_artifact: bool,
        has_safety_confirm: bool,
        time_label: String,
    },
}

#[derive(Clone, Debug)]
struct AmooSessionItem {
    session_key: String,
    title: String,
    agent: String,
    status: String,
    status_kind: u8,
    meta: String,
    items: Vec<AmooChatItem>,
}

#[derive(Clone, Debug)]
struct AmooMemoryEntry {
    title: String,
    sub: String,
}

#[derive(Clone, Debug)]
struct AmooSkillEntry {
    skill_key: String,
    name: String,
    enabled: bool,
    prompt: String,
}

fn default_amoo_sessions() -> Vec<AmooSessionItem> {
    vec![
        AmooSessionItem {
            session_key: "demo:1".into(),
            title: "发票入册与背屏图案生成".into(),
            agent: "Amoo 助手".into(),
            status: "待确认".into(),
            status_kind: 1,
            meta: "2 个产物 · 刚刚".into(),
            items: vec![
                AmooChatItem::UserText {
                    attachment: Some(("报销发票_1021.png".into(), "1.4 MB · 已上传".into(), true)),
                    text: "把这张发票归档到报销表，再帮我生成一个赛博闪电像素图放到手机背屏显示。".into(),
                },
                AmooChatItem::UserVoice {
                    transcript: "“顺便发短信告诉李明我马上过去”".into(),
                    duration_sec: 4,
                },
                AmooChatItem::AssistantRich {
                    thinking_time: "1.8s".into(),
                    thinking_steps: vec![
                        "1. 视觉识别「报销发票_1021.png」：提取高德打车电子发票 ¥68.00，归档至 10 月报销单（累计 ¥386.00）".into(),
                        "2. 背屏点阵生成：合成 23×17「赛博闪电」像素矩阵（391 颗 LED）".into(),
                        "3. 通讯录匹配「李明」(138 0013 8000)，拟定短信正文并触发敏感操作确认".into(),
                    ],
                    tools_summary: "归档发票 · 生成背屏图 · 拟定短信".into(),
                    text: None,
                    has_bolt_artifact: true,
                    has_invoice_artifact: true,
                    has_safety_confirm: true,
                    time_label: "刚刚".into(),
                },
            ],
        },
        AmooSessionItem {
            session_key: "demo:2".into(),
            title: "金融热点整理与周报草拟".into(),
            agent: "深度研究".into(),
            status: "执行中".into(),
            status_kind: 2,
            meta: "步骤 3/4 · 14:22".into(),
            items: vec![
                AmooChatItem::UserText {
                    attachment: Some(("评审方案_v2.pdf".into(), "640 KB · 已上传".into(), false)),
                    text: "结合这份评审方案，整理本周端侧大模型与智能硬件热点并草拟部门周报。".into(),
                },
                AmooChatItem::AssistantRich {
                    thinking_time: "3.2s".into(),
                    thinking_steps: vec![
                        "1. 解析附件「评审方案_v2.pdf」核心指标与里程碑".into(),
                        "2. 检索本周端侧 AI 与智能硬件行业动态（共 14 篇信源）".into(),
                        "3. 正在生成结构化周报草稿与下周排期建议（步骤 3/4）".into(),
                    ],
                    tools_summary: "文档解析 · 网页检索 · 周报生成".into(),
                    text: Some("已完成前 3 步信源聚合：\n• 端侧 Qwen3 混合推理延迟降低 28%，NPU 算力利用率达 82%\n• 背屏 23×17 光显矩阵新增 4 项互动场景\n• 正在为你汇总成飞书周报文档…".into()),
                    has_bolt_artifact: false,
                    has_invoice_artifact: false,
                    has_safety_confirm: false,
                    time_label: "14:22".into(),
                },
            ],
        },
        AmooSessionItem {
            session_key: "demo:3".into(),
            title: "同步飞书日历与下午会议".into(),
            agent: "飞书协同".into(),
            status: "已完成".into(),
            status_kind: 0,
            meta: "已写日历 · 11:40".into(),
            items: vec![
                AmooChatItem::UserText {
                    attachment: None,
                    text: "把下午两点 3 楼大会议室的产品评审同步到飞书日历，并提前 15 分钟在背屏闪烁提醒。".into(),
                },
                AmooChatItem::AssistantRich {
                    thinking_time: "1.1s".into(),
                    thinking_steps: vec![
                        "1. 校验飞书日历今日日程冲突：14:00 - 15:00 空闲".into(),
                        "2. 写入日程「第四季度产品评审 · 3 楼大会议室」".into(),
                        "3. 注册 13:45 背屏消息闪烁与语音提醒".into(),
                    ],
                    tools_summary: "飞书日历 · 背屏提醒".into(),
                    text: Some("已为你写入飞书日历「14:00 第四季度产品评审（3 楼大会议室）」，并已设置 13:45 手机背屏点阵闪烁提醒。".into()),
                    has_bolt_artifact: true,
                    has_invoice_artifact: false,
                    has_safety_confirm: false,
                    time_label: "11:40".into(),
                },
            ],
        },
        AmooSessionItem {
            session_key: "demo:4".into(),
            title: "整理相册截图并提取待办".into(),
            agent: "Amoo 助手".into(),
            status: "已完成".into(),
            status_kind: 0,
            meta: "4 条待办 · 昨天".into(),
            items: vec![
                AmooChatItem::UserText {
                    attachment: Some(("会议白板截图.jpg".into(), "3.2 MB · 手机相册".into(), true)),
                    text: "帮我从今天拍摄的会议白板和聊天截图里提取所有待办事项。".into(),
                },
                AmooChatItem::AssistantRich {
                    thinking_time: "2.4s".into(),
                    thinking_steps: vec![
                        "1. 扫描手机相册 /sdcard/DCIM/Screenshots 最新 6 张图片".into(),
                        "2. 端侧 OCR 识别白板手写字迹与群聊截图".into(),
                        "3. 去重并按优先级归类为 4 条待办事项".into(),
                    ],
                    tools_summary: "相册扫描 · 端侧识屏 · 待办提取".into(),
                    text: Some("已从相册最新截图中提取 4 条待办：\n1. 周三前完成背屏点阵工坊 4 款预设联调\n2. 核对 10 月差旅电子发票并归档报销单（¥386.00）\n3. 周四 14:00 参加 3 楼大会议室版本规划会\n4. 验证 OpenClaw :18789 局域网反向代理连通性".into()),
                    has_bolt_artifact: false,
                    has_invoice_artifact: true,
                    has_safety_confirm: false,
                    time_label: "昨天".into(),
                },
            ],
        },
    ]
}

fn default_amoo_skills() -> Vec<AmooSkillEntry> {
    vec![
        AmooSkillEntry {
            skill_key: "feishu-calendar".into(),
            name: "飞书日历".into(),
            enabled: true,
            prompt: "查询今日飞书日历安排，并把下午的会议同步到背屏提醒".into(),
        },
        AmooSkillEntry {
            skill_key: "wecom".into(),
            name: "企业微信".into(),
            enabled: true,
            prompt: "汇总企业微信未读工作群消息并提取核心待办".into(),
        },
        AmooSkillEntry {
            skill_key: "amap".into(),
            name: "高德出行".into(),
            enabled: true,
            prompt: "查询现在去公司的实时路况，并规划最快通勤路线".into(),
        },
        AmooSkillEntry {
            skill_key: "web-search".into(),
            name: "网页检索".into(),
            enabled: true,
            prompt: "深度检索今日科技与金融热点并整理 3 条核心摘要".into(),
        },
        AmooSkillEntry {
            skill_key: "call-helper".into(),
            name: "通话助手".into(),
            enabled: true,
            prompt: "查看最近未接来电并帮我拟定礼貌回电短信".into(),
        },
        AmooSkillEntry {
            skill_key: "backscreen-matrix".into(),
            name: "背屏点阵".into(),
            enabled: true,
            prompt: "帮我设计一个 23×17 的背屏赛博闪电像素图案并同步到背屏".into(),
        },
    ]
}

struct Companion {
    font: FamilyId,
    mono_font: FamilyId,
    theme_mode: ThemeMode,
    settings: SettingsUi,
    shortcuts: shortcuts::Shortcuts,
    github_stars: github_stars::GithubStars,
    desktop_update: DesktopUpdateState,
    interaction: interaction::InteractionUi,
    motion_started: Instant,
    job_started: Instant,
    palette: Cell<Palette>,
    page: Page,
    compact: mini_modes::CompactWindow,
    window_memory: mini_modes::WindowMemory,
    compact_start_pending: bool,
    // Connection
    devices: Vec<Transport>,
    mdns_services: Vec<Service>,
    selected: Option<Device>,
    phone_link: Option<stepx_companion::companion_link::Link>,
    expansion: expansion_state::ExpansionUi,
    phone_link_notice: Option<(String, bool)>,
    preferences: Preferences,
    allow_first_connect: bool,
    pairing_hosts: Vec<std::net::IpAddr>,
    pair_tab: PairTab,
    connect_guide_step: Option<u8>,
    adb_guide_step: Option<u8>,
    show_secret: bool,
    qr: Option<Arc<QrSession>>,
    // Device & Overview
    summary: String,
    storage_report: Option<StorageReport>,
    tools_pending: Option<ToolOperation>,
    tool_page: Option<ToolPage>,
    tool_category: u8,
    tool_confirmation: Option<Box<Action>>,
    tools_feedback: Option<(Option<ToolPage>, String, bool)>,
    job_tool: Option<ToolPage>,
    google_progress: Arc<std::sync::Mutex<String>>,
    tools_tab: u8,
    tools_result_page: usize,
    system_compression: Option<toolbox::SystemCompression>,
    tools_last_result: Option<String>,
    capabilities: Option<Capabilities>,
    mirror: Option<MirrorSession>,
    mirror_controls: Option<mirror_controls::ControlWindow>,
    screenshot: Option<(tempfile::TempDir, PathBuf)>,
    overview_back_preview: bool,
    // BackScreen (23×17 Dot-Matrix)
    back_screen: BackScreenUiState,
    updates: UpdateState,
    matrix_marquee_on: bool,
    // Amoo Work
    work: WorkUiState,
    workspace: WorkspaceUi,
    // Amoo
    demo_mode: bool,
    amoo_gateway_version: String,
    amoo_gateway_online: bool,
    amoo_cron_count: usize,
    amoo_cron_jobs: Vec<openclaw::RealCronInfo>,
    amoo_show_schedules: bool,
    amoo_show_details: bool,
    amoo_ui: AmooUiState,
    amoo_bm25_entries: usize,
    amoo_total_skills: usize,
    amoo_snapshot: Option<AmooSnapshot>,
    amoo_poll_remaining: u8,
    amoo_pending_reply: HashMap<String, amoo::PendingReply>,
    last_amoo_poll: Instant,
    amoo_agent_idx: usize,
    amoo_session_idx: usize,
    amoo_sessions: Vec<AmooSessionItem>,
    amoo_expanded_tool_records: std::collections::HashSet<String>,
    amoo_cap_idx: usize,
    amoo_tts_on: bool,
    amoo_voice_playing: bool,
    amoo_thinking_expanded: bool,
    amoo_bolt_pushed: bool,
    amoo_invoice_open: bool,
    amoo_safety_decision: SafetyDecision,
    amoo_plus_open: bool,
    amoo_recording: bool,
    amoo_record_secs: u32,
    amoo_attachments: Vec<AmooAttachment>,
    amoo_memory_enabled: bool,
    amoo_style_idx: usize,
    amoo_commute_idx: usize,
    amoo_memories: Vec<AmooMemoryEntry>,
    amoo_skills: Vec<AmooSkillEntry>,
    amoo_skill_manage: bool,
    amoo_safety_policy_idx: usize,
    amoo_guard_count: usize,
    // Studio & Reverse Proxy
    studio: StudioUiState,
    studio_lab: StudioLabUiState,
    // Push Notifications & IM Chat Hub & Compact Modes
    push: PushUiState,
    // SMS
    sms: Vec<Message>,
    sms_loaded: bool,
    phone_loaded: bool,
    sms_error: Option<String>,
    call_logs_error: Option<String>,
    contacts_error: Option<String>,
    sms_filter: SmsFilter,
    selected_thread: Option<String>,
    selected_sim: Option<i32>,
    pending_sms_plan: Option<SmsPlan>,
    last_sms_status: Option<SmsSendStatus>,
    // Phone & Contacts
    call_logs: Vec<CallRecord>,
    contacts: Vec<ContactEntry>,
    selected_contact: usize,
    call_filter_missed: bool,
    active_call_number: Option<String>,
    pending_call: Option<phone::DialPlan>,
    selected_call_sim: Option<i32>,
    // Files
    directory: String,
    files: Vec<Entry>,
    file_page: usize,
    file_sort: FileSort,
    selected_file: Option<usize>,
    file_modal: Option<FileModal>,
    trash_confirm: bool,
    permanent_delete: bool,
    trashed: Option<Trashed>,
    // Apps & Privileged System Management
    apps: Vec<AppInfo>,
    apps_loaded: bool,
    apps_error: Option<String>,
    pending_app_uninstall: Option<String>,
    include_system_apps: bool,
    google_suite: GoogleSuiteStatus,
    companion_status: CompanionStatus,
    neo_status: stepx_companion::neo_agent::NeoStatus,
    neo_enable_ime: bool,
    system_services: Vec<SystemServiceInfo>,
    fcitx5_active: bool,
    input_method: stepx_companion::input_method::Status,
    deep_amoo_granted: bool,
    deep_backscreen_granted: bool,
    backscreen_hw_breath: bool,
    // Amoo Desktop Experience Enhancements
    amoo_model_idx: usize,
    amoo_lightbox_open: bool,
    amoo_lightbox_zoom: u8,
    amoo_express_open: bool,
    express_parcels: Vec<ExpressParcelInfo>,
    amoo_pc_control_enabled: bool,
    amoo_bg_keepalive: bool,
    // Job output
    content: String,
    output_page: Page,
    job_page: Page,
    // Status & jobs
    status: String,
    error: bool,
    busy: bool,
    initializing: bool,
    loading_frame: usize,
    sender: mpsc::Sender<Job>,
    receiver: mpsc::Receiver<Result<Outcome>>,
    last_scan: Instant,
    connection_watch: connection_watch::ConnectionWatch,
    text_selection: SelectionHandle,
    selected_text: Option<String>,
    context_menu: Option<context_menu::Menu>,
    reply_motion: reply_motion::ReplyMotion,
    speech: speech_state::SpeechUi,
    // Input
    fields: Vec<Input>,
    focus: Option<usize>,
    // View state
    scroll: ClippedScrollStateHandle,
    scroll_aux: ClippedScrollStateHandle,
    push_list_scroll: ClippedScrollStateHandle,
    push_detail_scroll: ClippedScrollStateHandle,
    sms_threads_scroll: ClippedScrollStateHandle,
    sms_messages_scroll: ClippedScrollStateHandle,
    hover_states: RefCell<HashMap<String, (u64, MouseStateHandle)>>,
    render_epoch: Cell<u64>,
    chat_scroll_metrics: std::rc::Rc<std::cell::Cell<(f32, f32)>>,
    chat_tail_requested: std::rc::Rc<std::cell::Cell<bool>>,
    wheel_motions: RefCell<HashMap<(&'static str, u32), smooth_scroll::WheelHandle>>,
    capture: Option<PathBuf>,
    capture_at: Instant,
    last_clock_second: u64,
    activation_requests: Arc<std::sync::atomic::AtomicBool>,
}

fn is_stepx_model(model: &str) -> bool {
    let m = model.to_lowercase();
    m.contains("pz261a") || m.contains("stepx") || m.contains("pandora")
}

fn is_wireless(transport: &str) -> bool {
    transport.contains(':') || transport.contains("_tcp")
}

impl Companion {
    fn new(
        ctx: &mut ViewContext<Self>,
        capture: Option<PathBuf>,
        activation_requests: Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        let (font, mono_font) =
            FontCache::handle(ctx).update(ctx, |cache, _| theme::load_fonts(cache));
        let (sender, jobs) = mpsc::channel::<Job>();
        let (results, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(job) = jobs.recv() {
                let outcome = Adb::discover().and_then(|adb| job(&adb));
                if results.send(outcome).is_err() {
                    break;
                }
            }
        });
        let (preferences, warning) = match Preferences::load() {
            Ok(p) => (p, None),
            Err(e) => (Preferences::default(), Some(e.to_string())),
        };
        let mut theme_mode = match std::env::var("STEPX_THEME").ok().as_deref() {
            Some("dark") => ThemeMode::Dark,
            Some("light") => ThemeMode::Light,
            _ => match stepx_companion::settings::load().appearance {
                stepx_companion::settings::Appearance::System => ThemeMode::System,
                stepx_companion::settings::Appearance::Light => ThemeMode::Light,
                stepx_companion::settings::Appearance::Dark => ThemeMode::Dark,
            },
        };
        if std::env::args().any(|arg| arg == "--dark") {
            theme_mode = ThemeMode::Dark;
        } else if std::env::args().any(|arg| arg == "--light") {
            theme_mode = ThemeMode::Light;
        }
        // Paint the home/loading state before device discovery or ADB work.
        let devices: Vec<Transport> = Vec::new();
        let page = std::env::args()
            .position(|arg| arg == "--page")
            .and_then(|i| std::env::args().nth(i + 1))
            .map(|p| Page::from_arg(&p))
            .unwrap_or(if std::env::args().any(|a| a == "--pair-qr") {
                Page::Connect
            } else if std::env::args().any(|a| a == "--mini-ai") {
                Page::Amoo
            } else if std::env::args().any(|a| a == "--mini-notify") {
                Page::Notifications
            } else {
                Page::Overview
            });
        let status = warning.clone().unwrap_or_else(|| {
            if devices.is_empty() {
                "未连接设备".into()
            } else {
                format!("发现 {} 台设备", devices.len())
            }
        });
        let mut fields: Vec<Input> = (0..FIELD_COUNT).map(|_| Input::default()).collect();
        fields[F_BACK_PROMPT].set("让猫咪眨眼陪我工作，亮度 4 级，播放 30 秒");
        fields[F_BACK_MARQUEE].set("STEPX AI");

        let demo_mode = cfg!(feature = "ui-capture") && std::env::args().any(|a| a == "--demo");
        let mut me = Self {
            font,
            mono_font,
            theme_mode,
            settings: SettingsUi::default(),
            shortcuts: Default::default(),
            github_stars: Default::default(),
            desktop_update: Default::default(),
            interaction: Default::default(),
            motion_started: Instant::now(),
            job_started: Instant::now(),
            palette: Cell::new(Palette::for_theme(false)),
            page,
            compact: Default::default(),
            window_memory: mini_modes::WindowMemory::new(),
            compact_start_pending: mini_modes::WindowMemory::new().startup_mode()
                != ShellMode::Full,
            devices,
            mdns_services: vec![],
            selected: None,
            phone_link: None,
            expansion: Default::default(),
            phone_link_notice: None,
            allow_first_connect: true,
            connect_guide_step: if preferences.first_connect_done {
                None
            } else {
                Some(0)
            },
            adb_guide_step: (!demo_mode && preferences.should_show_adb_guide()).then_some(0),
            preferences,
            pairing_hosts: vec![],
            pair_tab: PairTab::Qr,
            show_secret: false,
            qr: None,
            summary: String::new(),
            storage_report: None,
            tools_pending: None,
            tool_page: std::env::args()
                .position(|a| a == "--tool")
                .and_then(|i| std::env::args().nth(i + 1))
                .and_then(|a| ToolPage::from_arg(&a)),
            tool_category: 0,
            tool_confirmation: None,
            tools_feedback: None,
            job_tool: None,
            google_progress: Default::default(),
            tools_tab: 0,
            tools_result_page: 0,
            system_compression: None,
            tools_last_result: None,
            capabilities: None,
            mirror: None,
            mirror_controls: None,
            screenshot: None,
            overview_back_preview: std::env::var_os("STEPX_BACK_PREVIEW").is_some(),
            back_screen: BackScreenUiState::default(),
            updates: UpdateState::default(),
            matrix_marquee_on: true,
            demo_mode,
            work: WorkUiState::default(),
            workspace: WorkspaceUi::default(),
            amoo_gateway_version: String::new(),
            amoo_gateway_online: false,
            amoo_cron_count: 0,
            amoo_cron_jobs: Vec::new(),
            amoo_show_schedules: std::env::args().any(|arg| arg == "--amoo-tasks"),
            amoo_show_details: std::env::args().any(|arg| arg == "--amoo-details"),
            amoo_ui: AmooUiState::default(),
            amoo_bm25_entries: 0,
            amoo_total_skills: 0,
            amoo_snapshot: None,
            amoo_poll_remaining: 0,
            amoo_pending_reply: HashMap::new(),
            last_amoo_poll: Instant::now(),
            amoo_expanded_tool_records: Default::default(),
            amoo_agent_idx: 0,
            amoo_session_idx: 0,
            amoo_sessions: if demo_mode {
                default_amoo_sessions()
            } else {
                Vec::new()
            },
            amoo_cap_idx: 1,
            amoo_tts_on: false,
            amoo_voice_playing: false,
            amoo_thinking_expanded: false,
            amoo_bolt_pushed: false,
            amoo_invoice_open: false,
            amoo_safety_decision: SafetyDecision::Pending,
            amoo_plus_open: std::env::var_os("STEPX_AMOO_INTERACT").is_some(),
            amoo_recording: std::env::var_os("STEPX_AMOO_INTERACT").is_some(),
            amoo_record_secs: 5,
            amoo_attachments: if demo_mode {
                vec![
                    AmooAttachment {
                        name: "活动海报.jpg".into(),
                        size_label: "2.1M".into(),
                        is_image: true,
                        local_path: None,
                    },
                    AmooAttachment {
                        name: "评审方案_v2.pdf".into(),
                        size_label: "640K".into(),
                        is_image: false,
                        local_path: None,
                    },
                ]
            } else {
                Vec::new()
            },
            amoo_memory_enabled: demo_mode,
            amoo_style_idx: 0,
            amoo_commute_idx: 0,
            amoo_memories: if demo_mode {
                vec![
                    AmooMemoryEntry {
                        title: "高德打车默认开具电子发票".into(),
                        sub: "报销习惯 · 自动归档".into(),
                    },
                    AmooMemoryEntry {
                        title: "每周五 16:00 提醒汇总周报".into(),
                        sub: "日程记忆 · 飞书同步".into(),
                    },
                ]
            } else {
                Vec::new()
            },
            amoo_skills: if demo_mode {
                default_amoo_skills()
            } else {
                Vec::new()
            },
            amoo_skill_manage: false,
            amoo_safety_policy_idx: 0,
            amoo_guard_count: if demo_mode { 12 } else { 0 },
            studio: StudioUiState {
                tab: 2,
                cloud_category: 3,
                ..Default::default()
            },
            studio_lab: StudioLabUiState::default(),
            push: PushUiState::default(),
            sms: Vec::new(),
            sms_loaded: false,
            phone_loaded: false,
            sms_error: None,
            call_logs_error: None,
            contacts_error: None,
            sms_filter: SmsFilter::All,
            selected_thread: None,
            selected_sim: None,
            pending_sms_plan: None,
            last_sms_status: None,
            call_logs: Vec::new(),
            contacts: Vec::new(),
            selected_contact: 0,
            call_filter_missed: false,
            pending_call: None,
            selected_call_sim: None,
            active_call_number: if demo_mode {
                Some("138 0013 8000".into())
            } else {
                None
            },
            directory: "/sdcard".into(),
            files: vec![],
            file_page: 0,
            file_sort: FileSort::Name,
            selected_file: None,
            file_modal: None,
            trash_confirm: false,
            permanent_delete: false,
            trashed: None,
            apps: vec![],
            apps_loaded: false,
            apps_error: None,
            pending_app_uninstall: None,
            include_system_apps: false,
            google_suite: if demo_mode {
                GoogleSuiteStatus::default_verified()
            } else {
                GoogleSuiteStatus::default()
            },
            companion_status: CompanionStatus::default(),
            neo_status: Default::default(),
            neo_enable_ime: false,
            system_services: if demo_mode {
                default_system_services()
            } else {
                Vec::new()
            },
            fcitx5_active: demo_mode,
            input_method: Default::default(),
            deep_amoo_granted: demo_mode,
            deep_backscreen_granted: demo_mode,
            backscreen_hw_breath: false,
            amoo_model_idx: 0,
            amoo_lightbox_open: std::env::var_os("STEPX_AMOO_LIGHTBOX").is_some()
                || std::env::args().any(|a| a == "--amoo-lightbox"),
            amoo_lightbox_zoom: if std::env::var_os("STEPX_AMOO_LIGHTBOX").is_some()
                || std::env::args().any(|a| a == "--amoo-lightbox")
            {
                150
            } else {
                100
            },
            amoo_express_open: demo_mode,
            express_parcels: if demo_mode {
                default_express_parcels()
            } else {
                Vec::new()
            },
            amoo_pc_control_enabled: demo_mode,
            amoo_bg_keepalive: demo_mode,
            content: String::new(),
            output_page: Page::Overview,
            job_page: Page::Overview,
            status,
            error: warning.is_some(),
            busy: false,
            initializing: !demo_mode,
            loading_frame: 0,
            sender,
            receiver,
            last_scan: Instant::now(),
            connection_watch: Default::default(),
            text_selection: Default::default(),
            selected_text: None,
            context_menu: None,
            reply_motion: Default::default(),
            speech: Default::default(),
            fields,
            focus: None,
            scroll: Default::default(),
            scroll_aux: Default::default(),
            push_list_scroll: Default::default(),
            push_detail_scroll: Default::default(),
            sms_threads_scroll: Default::default(),
            sms_messages_scroll: Default::default(),
            hover_states: RefCell::new(HashMap::new()),
            render_epoch: Cell::new(0),
            wheel_motions: Default::default(),
            chat_scroll_metrics: Default::default(),
            chat_tail_requested: Default::default(),
            capture,
            capture_at: Instant::now(),
            last_clock_second: 0,
            activation_requests,
        };
        #[cfg(feature = "ui-capture")]
        if me.page == Page::Expansion && std::env::args().any(|a| a == "--expansion-settings") {
            me.expansion.settings = true;
        }
        #[cfg(feature = "ui-capture")]
        if me.page == Page::Studio {
            if let Some(i) = std::env::args().position(|a| a == "--studio-source") {
                me.studio.source = match std::env::args().nth(i + 1).as_deref() {
                    Some("phone") => 1,
                    Some("saved") => 2,
                    _ => 0,
                };
            }
            if let Some(i) = std::env::args().position(|a| a == "--studio-section") {
                me.studio.tab = match std::env::args().nth(i + 1).as_deref() {
                    Some("create") => 2,
                    Some("settings") => 3,
                    _ => 0,
                };
            }
            if demo_mode && std::env::args().any(|a| a == "--verify-studio-key-mask") {
                // Deliberately non-secret fixture; never load real keys into a capture.
                me.studio.tab = 3;
                me.fields[F_LAB_KEY].set("MASK_TEST_NOT_SECRET");
                me.fields[F_LAB_KEY].preedit = "输入测试".into();
                me.focus = Some(F_LAB_KEY);
            }
        }
        me.back_screen
            .set_preview_visible(me.page == Page::BackScreen);
        if me.page == Page::Amoo {
            me.reveal_latest_chat();
        } else if me.page == Page::Sms {
            Self::scroll_to_end(&me.scroll_aux);
        }
        let has_stepx_candidate = me
            .devices
            .iter()
            .any(|d| d.state == "device" && is_stepx_model(&d.model));
        #[cfg(feature = "ui-capture")]
        if let Some(panel) = std::env::args()
            .position(|a| a == "--studio-tab")
            .and_then(|i| std::env::args().nth(i + 1))
        {
            me.studio.tab = match panel.as_str() {
                "work" => 2,
                "settings" => 3,
                _ => 0,
            };
            if demo_mode && me.page == Page::Studio {
                me.studio_lab.loaded = true;
                me.studio_lab.artifacts =
                    stepx_companion::studio_lab::list_artifacts().unwrap_or_default();
                me.studio_lab.selected = (!me.studio_lab.artifacts.is_empty()).then_some(0);
                if let Some(a) = me.studio_lab.artifacts.first() {
                    if let Ok(report) = stepx_companion::studio_lab::read_report(a) {
                        me.studio_lab.report = Some((a.id.clone(), report));
                    }
                }
            }
        }
        #[cfg(feature = "ui-capture")]
        if std::env::args().any(|a| a == "--demo") {
            me.load_demo();
            if let Some(index) = std::env::args().position(|a| a == "--adb-guide-preview") {
                me.adb_guide_step = Some(
                    std::env::args()
                        .nth(index + 1)
                        .and_then(|s| s.parse::<u8>().ok())
                        .unwrap_or(0)
                        .min(2),
                );
            }
            if std::env::args().any(|a| a == "--amoo-screen-view") {
                me.amoo_ui.screen_mode = true;
                me.amoo_ui.gui_desktop = true;
                me.fields[F_AMOO_GUI_APP].set("设置");
                me.fields[F_AMOO].set("打开关于手机，查看 Android 版本，不修改设置");
            }
            if let Some(index) = std::env::args().position(|arg| arg == "--ota-report") {
                if let Some(path) = std::env::args().nth(index + 1) {
                    if let Ok(bytes) = std::fs::read(path) {
                        if let Ok(report) =
                            serde_json::from_slice::<stepx_companion::ota::OtaReport>(&bytes)
                        {
                            me.updates.phone = Some(stepx_companion::updates::PhoneVersion {
                                serial: report.serial.clone(),
                                firmware: report.current_version.clone(),
                                security_patch: "—".into(),
                                has_update_page: true,
                            });
                            me.updates.report = Some(report);
                        }
                    }
                }
            }
            ctx.focus_self();
            me.initialize_shortcut(ctx);
            me.initialize_stars(ctx);
            me.initialize_desktop_updates(ctx);
            me.schedule_tick(ctx);
            return me;
        }
        if has_stepx_candidate {
            me.auto_select();
        } else {
            me.refresh();
        }
        ctx.focus_self();
        me.initialize_shortcut(ctx);
        me.initialize_stars(ctx);
        me.initialize_desktop_updates(ctx);
        me.schedule_tick(ctx);
        me
    }

    fn schedule_tick(&self, ctx: &mut ViewContext<Self>) {
        let interval = if self.reply_motion.active()
            && !self.settings.hidden
            && matches!(self.page, Page::Amoo | Page::Work)
        {
            32
        } else if self.mirror.is_some()
            || self.busy
            || self.work.running
            || self.amoo_ui.gui_running
            || self.work_previews_loading()
            || self.amoo_ui.snapshot_job.is_some()
            || (self.page == Page::BackScreen && self.back_screen.preview_running)
        {
            100
        } else {
            500
        };
        ctx.spawn(
            async move {
                facloud_ui::r#async::Timer::after(Duration::from_millis(interval)).await;
            },
            |me, _, ctx| {
                me.tick(ctx);
                me.schedule_tick(ctx);
            },
        );
    }

    fn mouse_state(&self, id: impl Into<String>) -> MouseStateHandle {
        let mut states = self.hover_states.borrow_mut();
        let entry = states.entry(id.into()).or_default();
        entry.0 = self.render_epoch.get();
        entry.1.clone()
    }

    fn submit(&mut self, label: &str, job: impl FnOnce(&Adb) -> Result<Outcome> + Send + 'static) {
        if self.sender.send(Box::new(job)).is_ok() {
            self.amoo_ui.snapshot_generation = self.amoo_ui.snapshot_generation.wrapping_add(1);
            self.busy = true;
            self.job_page = self.page;
            self.job_started = Instant::now();
            self.loading_frame = 0;
            self.job_tool = self.tool_page;
            self.error = false;
            self.status = label.into();
        }
    }

    fn with_device(
        &mut self,
        label: &str,
        job: impl FnOnce(&Adb, Device) -> Result<Outcome> + Send + 'static,
    ) {
        match self.selected.clone() {
            Some(device) => self.submit(label, move |adb| job(adb, device)),
            None => {
                self.page = Page::Connect;
                self.status = "请先连接设备".into();
                self.error = true;
            }
        }
    }

    fn refresh(&mut self) {
        self.last_scan = Instant::now();
        let prefs = self.preferences.clone();
        let pairing_hosts = self.pairing_hosts.clone();
        self.submit("正在查找设备…", move |adb| {
            let (devices, services) =
                stepx_companion::reconnect::scan(adb, &prefs, &pairing_hosts)?;
            Ok(Outcome::Transports(devices, services))
        });
        self.job_tool = None;
    }

    fn auto_load_for_page(&mut self) {
        if self.page == Page::Studio && !self.studio.loaded && !self.busy {
            self.refresh_studio();
            return;
        }
        if self.busy || self.selected.is_none() {
            return;
        }
        match self.page {
            Page::Tools
                if self.tool_page == Some(ToolPage::Storage)
                    && self.storage_report.is_none()
                    && !self.demo_mode =>
            {
                self.with_device("正在扫描手机公开目录…", |adb, d| {
                    Ok(Outcome::StorageScanned(
                        d.serial.clone(),
                        toolbox::scan(adb, &d)?,
                        toolbox::probe_system(adb, &d).ok(),
                    ))
                });
            }
            Page::Connect if !self.demo_mode && !self.companion_status.checked => {
                self.handle_device_action(&Action::CompanionInspect);
            }
            Page::Tools if !self.demo_mode => match self.tool_page {
                Some(ToolPage::Google) if !self.google_suite.checked => {
                    self.handle_device_action(&Action::GoogleSuiteInspect)
                }
                Some(ToolPage::Companion) if !self.companion_status.checked => {
                    self.handle_device_action(&Action::CompanionInspect)
                }
                Some(ToolPage::Neo) if !self.neo_status.checked => {
                    self.handle_device_action(&Action::NeoInspect)
                }
                Some(ToolPage::InputMethod) if !self.input_method.checked => {
                    self.handle_device_action(&Action::InspectInputMethod);
                }
                Some(ToolPage::Services) if self.system_services.is_empty() => {
                    self.handle_device_action(&Action::InspectServices)
                }
                Some(ToolPage::Device) if self.summary.is_empty() => {
                    self.handle_device_action(&Action::Summary)
                }
                _ => {}
            },
            Page::Overview if !self.demo_mode => {
                self.with_device("正在刷新设备状态…", |adb, d| {
                    Ok(Outcome::Summary(
                        d.summary(adb)?,
                        bridge::capabilities(adb, &d).ok(),
                    ))
                });
            }
            Page::Work if !self.demo_mode => self.refresh_work(),
            Page::Account if !self.demo_mode => {
                self.handle_amoo_ui_action(&AmooUiAction::RefreshUsage)
            }
            Page::Amoo if !self.demo_mode => {
                let key = self
                    .amoo_sessions
                    .get(self.amoo_session_idx)
                    .map(|s| s.session_key.clone());
                self.with_device("正在从后台同步 Amoo 会话…", move |adb, d| {
                    let real = amoo::fetch(adb, &d, key.as_deref())?;
                    let msg = format!("后台已同步 {} 个真实会话", real.sessions.len());
                    Ok(Outcome::AmooRealData(real, msg))
                });
            }
            Page::Files if self.files.is_empty() => self.browse(self.directory.clone()),
            Page::Apps if !self.apps_loaded => {
                self.with_device("正在读取应用…", move |adb, d| {
                    Ok(Outcome::Apps(apps::catalog(adb, &d)?))
                });
            }
            Page::Sms if !self.demo_mode => {
                self.with_device("正在读取短信…", |adb, d| {
                    let caps = bridge::capabilities(adb, &d).ok();
                    Ok(Outcome::Sms(read_result(d.messages(adb)), caps))
                });
            }
            Page::Phone if !self.demo_mode => {
                self.with_device("正在读取通话记录与通讯录…", |adb, d| {
                    let logs = read_result(d.call_logs(adb));
                    let contacts = read_result(d.contacts(adb));
                    Ok(Outcome::PhoneData(logs, contacts))
                });
            }
            Page::Studio => {}
            _ => {}
        }
    }

    fn clear_device_state(&mut self) {
        self.clear_text_interaction();
        self.speech = Default::default();
        let mode = self.push.shell_mode;
        let pin = self.push.pin_window;
        self.push = PushUiState::default();
        self.push.shell_mode = mode;
        self.push.pin_window = pin;
        for index in [F_IM_REPLY, F_PUSH_REPLY, F_TOAST_REPLY] {
            self.fields[index].clear();
        }
        self.studio_lab.cloud_models.clear();
        self.studio_lab.cloud_model = None;
        self.studio_lab.cloud_scope = None;
        self.studio.phone_note = None;
        self.studio.cloud = None;
        self.studio.web = None;
        self.studio.web_selected = None;
        self.studio.web_error = None;
        self.studio.cloud_selected = None;
        self.studio.cloud_checked = None;
        self.studio.cloud_error = None;
        self.studio.connecting_until = None;
        let native_gui_unconfirmed =
            (self.amoo_ui.gui_running && !self.amoo_ui.gui_desktop) || self.amoo_ui.gui_unconfirmed;
        self.google_suite = GoogleSuiteStatus::default();
        self.companion_status = CompanionStatus::default();
        self.neo_status = Default::default();
        self.phone_link_notice = None;
        self.deep_backscreen_granted = false;
        self.deep_amoo_granted = false;
        self.amoo_bg_keepalive = false;
        self.system_services.clear();
        self.tool_confirmation = None;
        self.tools_feedback = None;
        self.storage_report = None;
        self.tools_pending = None;
        self.tools_result_page = 0;
        self.system_compression = None;
        self.tools_last_result = None;
        self.stop_mirror();
        self.capabilities = None;
        self.amoo_snapshot = None;
        self.amoo_poll_remaining = 0;
        self.amoo_pending_reply.clear();
        self.input_method = Default::default();
        self.amoo_ui = AmooUiState::default();
        self.amoo_attachments.clear();
        self.amoo_cron_jobs.clear();
        self.amoo_gateway_online = false;
        self.work.disconnect();
        self.back_screen = BackScreenUiState::default();
        self.updates = UpdateState::default();
        if !self.demo_mode {
            self.amoo_sessions.clear();
            self.amoo_skills.clear();
            self.amoo_memories.clear();
            self.amoo_cron_count = 0;
            self.amoo_bm25_entries = 0;
        }
        self.pending_sms_plan = None;
        self.last_sms_status = None;
        self.selected_thread = None;
        self.content.clear();
        self.sms.clear();
        self.sms_loaded = false;
        self.phone_loaded = false;
        self.pending_call = None;
        self.selected_call_sim = None;
        self.call_logs.clear();
        self.contacts.clear();
        self.sms_error = None;
        self.call_logs_error = None;
        self.contacts_error = None;
        self.selected_sim = None;
        self.active_call_number = None;
        self.selected_contact = 0;
        for field in [
            F_SMS_TO,
            F_SMS_BODY,
            F_PHONE_DIAL,
            F_CONTACT_SEARCH,
            F_SMS_SEARCH,
        ] {
            self.fields[field].clear();
        }
        self.summary.clear();
        self.files.clear();
        self.apps.clear();
        self.apps_loaded = false;
        self.apps_error = None;
        self.pending_app_uninstall = None;
        self.screenshot = None;
        self.selected_file = None;
        self.file_modal = None;
        self.trash_confirm = false;
        if native_gui_unconfirmed {
            self.status = "已停止桌面等待，手机原生操作可能仍继续，请在手机上核对".into();
        }
    }

    fn tick(&mut self, ctx: &mut ViewContext<Self>) {
        let _timing = perf::Span::new("tick");
        #[cfg(all(feature = "ui-capture", target_os = "linux"))]
        self.verify_text_interaction(ctx);
        #[cfg(all(feature = "ui-capture", target_os = "linux"))]
        self.verify_shell_details(ctx);
        self.poll_desktop_update(ctx);
        self.tick_speech(ctx);
        self.tick_reply_motion(ctx);
        self.tick_connection_watch(ctx);
        self.tick_settings(ctx);
        #[cfg(feature = "ui-capture")]
        self.verify_settings(ctx);
        self.remember_window(ctx, false);
        #[cfg(feature = "ui-capture")]
        self.verify_window_memory(ctx);
        if self
            .activation_requests
            .swap(false, std::sync::atomic::Ordering::AcqRel)
        {
            let id = ctx.window_id();
            self.settings.hidden = false;
            ctx.windows().show_window_and_focus_app(id);
        }
        let link_matches = self
            .selected
            .as_ref()
            .zip(self.phone_link.as_ref())
            .is_some_and(|(device, link)| {
                device.serial == link.serial && device.transport == link.transport
            });
        if !self.demo_mode && !link_matches {
            self.phone_link = None;
            if let Some(device) = self.selected.clone() {
                if let Ok(adb) = Adb::discover() {
                    self.phone_link =
                        Some(stepx_companion::companion_link::Link::start(adb, device));
                }
            }
        }
        if let Some(event) = self.phone_link.as_ref().and_then(|link| link.event()) {
            if !self.busy {
                self.status = event.message.clone();
            }
            self.phone_link_notice = Some((event.message, event.error));
            ctx.notify();
        }
        if self.tick_expansion() && matches!(self.page, Page::Overview | Page::Expansion) {
            ctx.notify();
        }
        let mut preview_visible = self.page == Page::BackScreen && !self.settings.hidden;
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        if let Some(window) =
            facloud_ui::windowing::winit::winit_window_for_id(ctx, ctx.window_id())
        {
            preview_visible &= !window.is_minimized().unwrap_or(false);
        }
        if self.poll_backscreen(preview_visible) && preview_visible {
            ctx.notify();
        }
        if self.poll_updates() && self.page == Page::Tools {
            ctx.notify();
        }
        if self.poll_work() && matches!(self.page, Page::Work | Page::Amoo) {
            ctx.notify();
        }
        if self.poll_amoo_gui() && self.page == Page::Amoo {
            ctx.notify();
        }
        if !self.demo_mode
            && self.page == Page::Overview
            && self.selected.is_some()
            && !self.busy
            && !self.amoo_ui.usage_attempted
        {
            self.start_account_usage_sync();
            ctx.notify();
        }
        if self.poll_account_usage_sync() {
            ctx.notify();
        }
        if self.push.syncing
            && !self.push.loaded
            && self.push.sync_started.elapsed() > Duration::from_millis(150)
            && self.push.sync_started.elapsed() < Duration::from_millis(750)
            && matches!(self.page, Page::Notifications | Page::Sms)
        {
            ctx.notify();
        }
        if self.busy
            && self.loading_frame == 0
            && self.job_started.elapsed() > Duration::from_millis(150)
        {
            self.loading_frame = 1;
            ctx.notify();
        }
        while let Ok(result) = self.receiver.try_recv() {
            let completed_page = self.job_page;
            let completed_tool = self.job_tool;
            let tool_result = result.as_ref().map_or(true, |outcome| {
                !matches!(outcome, Outcome::Transports(..) | Outcome::Selected(..) | Outcome::Reattached(..))
            });
            self.busy = false;
            if let Ok(mut progress) = self.google_progress.lock() {
                progress.clear();
            }
            self.error = result.is_err();
            if let Err(e) = &result {
                self.record_notice("操作未完成", format!("{e:#}"));
            }
            match result {
                Err(e) => {
                    self.initializing = false;
                    if self.job_page == Page::Apps {
                        self.apps_loaded = true;
                        self.apps_error = Some(format!("{e:#}"));
                    }
                    // A transient read error must not discard an accepted request.
                    if matches!(self.job_page, Page::Amoo | Page::Work) {
                        self.amoo_gateway_online = false;
                    }
                    if self.job_page == Page::Work {
                        self.work.catalog_loading = false;
                        self.work.error = Some(format!("{e:#}"));
                    }
                    self.status = format!("{e:#}");
                    if self.workspace.running.as_ref().is_some_and(|r| {
                        !r.accepted
                            && matches!(
                                r.executor,
                                stepx_companion::amoo_workspace::Executor::Phone
                                    | stepx_companion::amoo_workspace::Executor::Image
                            )
                    }) {
                        self.workspace_finish(
                            stepx_companion::amoo_workspace::Phase::Failed,
                            self.status.clone(),
                        );
                    }
                }
                Ok(outcome) => {
                    self.status = "就绪".into();
                    self.apply(outcome);
                }
            }
            if completed_page == Page::Tools
                && completed_tool.is_some()
                && tool_result
                && !self.busy
            {
                self.tools_feedback = Some((completed_tool, self.status.clone(), self.error));
            }
            if !self.busy && self.selected.is_some() && self.page != completed_page {
                self.auto_load_for_page();
            }
            ctx.notify();
        }
        if self.poll_amoo_sync() {
            ctx.notify();
        }
        if self.page == Page::Studio && !self.studio.loaded && !self.busy {
            self.refresh_studio();
        }
        if self.tick_studio_lab() && matches!(self.page, Page::Studio | Page::Amoo) {
            ctx.notify();
        }
        self.tick_studio_cloud();
        if self.tick_workspace(ctx) {
            ctx.notify();
        }
        if self.compact_start_pending {
            self.compact_start_pending = false;
            let mode = self.window_memory.startup_mode();
            self.set_compact_mode(mode, ctx);
        }
        self.tick_push_sync(ctx);
        if let Some(child) = &mut self.mirror {
            if let Ok(Some(status)) = child.try_wait() {
                self.error = !status.success();
                self.status = if status.success() {
                    "投屏已结束".into()
                } else {
                    format!("投屏已中断：{}", child.exit_detail(status))
                };
                self.mirror = None;
                self.tools_feedback =
                    Some((Some(ToolPage::Screen), self.status.clone(), self.error));
                ctx.notify();
            }
        }
        #[cfg(feature = "ui-capture")]
        self.verify_ui_performance(ctx);
        #[cfg(feature = "ui-capture")]
        self.verify_compact_window(ctx);
        #[cfg(feature = "ui-capture")]
        self.verify_compact_live(ctx);
        #[cfg(feature = "ui-capture")]
        self.verify_composer(ctx);
        #[cfg(feature = "ui-capture")]
        self.verify_workspace(ctx);
        self.sync_mirror_controls(ctx);
        #[cfg(feature = "ui-capture")]
        self.verify_mirror_controls(ctx);
        let clock_second = self.capture_at.elapsed().as_secs();
        let clock_changed = clock_second != self.last_clock_second;
        self.last_clock_second = clock_second;
        if clock_changed && self.qr.is_some() && self.page == Page::Connect {
            // Countdown.
            ctx.notify();
        }
        if clock_changed && self.amoo_recording && self.capture.is_none() {
            self.amoo_record_secs = (self.amoo_record_secs + 1).min(59);
            ctx.notify();
        }
        let selected_session_empty = self
            .amoo_sessions
            .get(self.amoo_session_idx)
            .is_some_and(|s| s.items.is_empty());
        let needs_initial_amoo_fetch = self.page == Page::Amoo
            && (self.amoo_sessions.is_empty()
                || (selected_session_empty && self.amoo_poll_remaining > 0));
        if !self.busy
            && !self.demo_mode
            && self.selected.is_some()
            && self.amoo_ui.snapshot_job.is_none()
            && !self.work.running
            && (!self.amoo_pending_reply.is_empty() || matches!(self.page, Page::Amoo | Page::Work))
            && self.last_amoo_poll.elapsed()
                >= if needs_initial_amoo_fetch {
                    Duration::from_millis(400)
                } else if self.amoo_poll_remaining > 0 {
                    Duration::from_millis(if !self.settings.hidden && self.chat_waiting() {
                        1200
                    } else {
                        2400
                    })
                } else if self.amoo_pending_reply.is_empty() && !self.amoo_gateway_online {
                    Duration::from_secs(30)
                } else {
                    Duration::from_secs(10)
                }
        {
            self.amoo_poll_remaining = self.amoo_poll_remaining.saturating_sub(1);
            self.last_amoo_poll = Instant::now();
            let key = self
                .amoo_sessions
                .get(self.amoo_session_idx)
                .map(|s| s.session_key.clone());
            let pending: Vec<String> = self.amoo_pending_reply.keys().cloned().collect();
            self.begin_amoo_sync(key, pending, needs_initial_amoo_fetch);
        }
        if !self.busy
            && self.amoo_ui.snapshot_job.is_none()
            && self.last_scan.elapsed() > Duration::from_secs(5)
        {
            if self.page == Page::Connect
                && self.pair_tab == PairTab::Qr
                && self.connect_guide_step.is_none_or(|step| step >= 4)
            {
                if let Some(qr) = self.qr.clone() {
                    if qr.expired() {
                        self.qr = QrSession::new().ok().map(Arc::new);
                        ctx.notify();
                    } else {
                        self.last_scan = Instant::now();
                        self.submit("等待扫码…", move |adb| {
                            Ok(Outcome::Qr(qr.try_pair(adb)?))
                        });
                    }
                } else if !self.error {
                    self.refresh();
                }
            }
        }
        if !self.demo_mode && self.last_scan.elapsed() > Duration::from_secs(5) {
            self.last_scan = Instant::now();
            self.begin_connection_watch();
        }
        #[cfg(feature = "ui-capture")]
        if std::env::args().any(|a| a == "--verify-google-cert") && !self.initializing {
            static SENT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            if !SENT.swap(true, std::sync::atomic::Ordering::SeqCst) {
                // Land on Tools → Google so the certification panel is on screen
                // when --capture-ui fires.
                self.page = Page::Tools;
                self.tool_page = Some(ToolPage::Google);
                eprintln!("GOOGLE_CERT_UI_VERIFICATION: landing on Tools → Google");
            }
        }
        #[cfg(feature = "ui-capture")]
        if std::env::args().any(|a| a == "--verify-amoo-gui")
            && !self.busy
            && !self.initializing
            && self.page == Page::Amoo
            && self
                .selected
                .as_ref()
                .is_some_and(|d| d.serial == "ZYPZ261A01M692000LP")
        {
            static SENT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            if !SENT.swap(true, std::sync::atomic::Ordering::SeqCst) {
                self.amoo_ui.screen_mode = false;
                self.fields[F_AMOO].set("打开京东，我的订单");
                eprintln!("GUI_UI_VERIFICATION: normal Amoo composer sends user request");
                self.send_amoo_draft();
            }
        }
        if self.capture.is_some()
            && !std::env::args().any(|a| {
                a == "--verify-settings"
                    || a == "--verify-compact"
                    || a == "--verify-compact-live"
                    || a == "--verify-composer"
                    || a == "--verify-workspace"
                    || a == "--verify-workspace-live"
                    || a == "--verify-window-memory"
            })
            && self.capture_at.elapsed() > Duration::from_millis(900)
            && (self.demo_mode
                || (!self.busy
                    && self.selected.is_some()
                    && !self.work_previews_loading()
                    && (self.page != Page::Notifications
                        || self.push.loaded
                        || self.push.error.is_some())
                    && (self.page != Page::Expansion
                        || self.expansion.snapshot.online
                        || self.expansion.snapshot.error)
                    && (self.page != Page::Studio
                        || self.studio.web.is_some()
                        || self.studio.web_error.is_some()))
                || self.capture_at.elapsed()
                    > Duration::from_millis(
                        if matches!(
                            self.page,
                            Page::Tools | Page::Work | Page::Amoo | Page::Studio
                        ) {
                            60000
                        } else {
                            12000
                        },
                    ))
        {
            let path = self.capture.take().unwrap();
            if let Some(window) = ctx.windows().platform_window(ctx.window_id()) {
                window
                    .as_ctx()
                    .request_frame_capture(Box::new(move |frame| {
                        let result = image::save_buffer(
                            &path,
                            &frame.data,
                            frame.width,
                            frame.height,
                            image::ColorType::Rgba8,
                        );
                        if let Err(e) = result {
                            eprintln!("UI capture failed: {e}");
                            std::process::exit(1);
                        }
                        std::process::exit(0);
                    }));
                ctx.notify();
            }
        }
    }

    fn apply(&mut self, outcome: Outcome) {
        match outcome {
            Outcome::Work(result) => self.apply_work(result),
            Outcome::AmooUi(result) => self.apply_amoo_ui(result),
            Outcome::StorageScanned(serial, report, system) => {
                if self.selected.as_ref().is_some_and(|d| d.serial == serial) {
                    self.status = format!(
                        "已扫描 {} 个公开文件；{} 个文件未完成重复校验",
                        report.scanned_files, report.hash_skipped
                    );
                    self.storage_report = Some(report);
                    self.system_compression = system;
                    self.tools_pending = None;
                    self.tools_result_page = 0;
                }
            }
            Outcome::SystemCompression(serial, result) => {
                if self.selected.as_ref().is_some_and(|d| d.serial == serial) {
                    self.status = if result.volume_support {
                        "数据分区支持压缩，仍需验证写入与还原兼容性"
                    } else {
                        "数据分区未启用 compression，系统权限无法单独开启透明压缩"
                    }
                    .into();
                    self.system_compression = Some(result);
                }
            }
            Outcome::ToolOperated(serial, label, result, report) => {
                if self.selected.as_ref().is_some_and(|d| d.serial == serial) {
                    let delta = result.available_after as i128 - result.available_before as i128;
                    let change = format!(
                        "{}{}",
                        if delta < 0 { "−" } else { "+" },
                        storage::format_bytes(delta.unsigned_abs() as u64)
                    );
                    let text = format!(
                        "{label}{} · 手机可用空间变化 {change} · {} → {}",
                        if result.completed {
                            "完成"
                        } else {
                            "未完全完成"
                        },
                        storage::format_bytes(result.available_before),
                        storage::format_bytes(result.available_after)
                    );
                    self.status = text.clone();
                    self.tools_last_result = Some(format!("{text}\n{}", result.result_path));
                    self.storage_report = report;
                    self.tools_pending = None;
                    self.tools_result_page = 0;
                }
            }
            Outcome::Transports(devices, services) => {
                if self.selected.as_ref().is_some_and(|d| {
                    !devices
                        .iter()
                        .any(|t| t.id == d.transport && t.state == "device")
                }) {
                    self.connection_watch.detached_serial =
                        self.selected.as_ref().map(|d| d.serial.clone());
                    self.selected = None;
                    self.amoo_gateway_online = false;
                    self.stop_mirror();
                    self.status = "连接暂时断开，正在自动重连；草稿与附件已保留".into();
                }
                self.devices = devices;
                self.mdns_services = services;
                if self.selected.is_none() {
                    self.auto_select();
                }
            }
            Outcome::Reattached(device) => {
                if self.connection_watch.detached_serial.as_deref() == Some(device.serial.as_str()) {
                    self.connection_watch.detached_serial = None;
                    self.connection_watch.identity_at = None;
                    self.selected = Some(device);
                    self.initializing = false;
                    self.error = false;
                    self.status = "已自动恢复连接 · 会话与草稿已保留".into();
                }
            }
            Outcome::Selected(device, summary, caps, real_amoo, sms, call_logs, contacts) => {
                self.allow_first_connect = false;
                let resumed = self.connection_watch.detached_serial.take().as_deref()
                    == Some(device.serial.as_str());
                if !resumed {
                    self.clear_device_state();
                }
                self.connection_watch.identity_at = None;
                self.apply_capabilities(caps);
                self.apply_sms_data(sms);
                self.apply_phone_data(call_logs, contacts);
                if !self.demo_mode && self.selected_thread.is_none() {
                    if let Some(first) = self.sms_threads().first() {
                        let addr = first.address.clone();
                        self.fields[F_SMS_TO].set(&addr);
                        self.selected_thread = Some(addr);
                    }
                }
                if let Some(real) = real_amoo {
                    self.apply_real_amoo_data(real);
                }
                self.preferences
                    .remember(device.serial.clone(), device.model.clone());
                if let Err(e) = self.preferences.save() {
                    self.status = format!("已连接，但保存设备失败：{e}");
                    self.error = true;
                } else {
                    self.status = format!("已连接 {}", device.model);
                }
                self.initializing = false;
                self.selected = Some(device);
                self.summary = summary;
                // Keep an explicitly opened connection page visible after discovery.
                // Normal launches already start on Overview; active guides advance here.
                if self.page == Page::Connect && self.connect_guide_step.is_some() {
                    self.connect_guide_step = Some(
                        if self.connect_guide_step.is_some_and(|step| step >= 3)
                            && self
                                .selected
                                .as_ref()
                                .is_some_and(|d| is_wireless(&d.transport))
                        {
                            5
                        } else {
                            2
                        },
                    );
                }
                if self.page == Page::Overview {
                    self.with_device("正在读取应用数量与图标…", |adb, d| {
                        Ok(Outcome::Apps(apps::catalog(adb, &d)?))
                    });
                } else {
                    self.auto_load_for_page();
                }
            }
            Outcome::Text(text) => {
                self.content = text;
                self.output_page = self.job_page;
            }
            Outcome::Notice(text) => {
                if text != "就绪" && text != "已记住的设备不在线" {
                    self.record_notice("执行反馈", text.clone());
                }
                self.initializing = false;
                self.status = text;
            }
            Outcome::Summary(text, caps) => {
                self.summary = text;
                self.apply_capabilities(caps);
            }
            Outcome::Sms(messages, caps) => {
                self.apply_capabilities(caps);
                self.apply_sms_data(messages);
                if !self.demo_mode && self.selected_thread.is_none() {
                    if let Some(first) = self.sms_threads().first() {
                        let addr = first.address.clone();
                        self.fields[F_SMS_TO].set(&addr);
                        self.selected_thread = Some(addr);
                    }
                }
            }
            Outcome::SmsPrepared(plan) => {
                self.status = format!(
                    "已就绪：将通过 {} 发送至 {}，请点击「确认发送」",
                    plan.subscription_label, plan.number
                );
                let target = self
                    .selected
                    .as_ref()
                    .map(|d| format!("手机：{} · {}", d.model, d.serial))
                    .unwrap_or_default();
                self.review_action(
                    &Action::ConfirmDirectSms,
                    "确认发送这条短信？",
                    target,
                    vec![
                        format!("收件人：{} · 使用 {}", plan.number, plan.subscription_label),
                        plan.body.clone(),
                        format!(
                            "共 {} 段短信，运营商可能按套餐计费。确认后实际从手机发出。",
                            plan.parts
                        ),
                    ],
                    "确认发送",
                    true,
                    false,
                );
                self.pending_sms_plan = Some(plan);
            }
            Outcome::SmsSent(send_status, messages) => {
                self.pending_sms_plan = None;
                if send_status.state == "sent" {
                    self.fields[F_SMS_BODY].clear();
                }
                self.status = send_status.summary_label();
                self.error = send_status.state == "failed";
                self.last_sms_status = Some(send_status);
                let send_notice = self.status.clone();
                self.apply_sms_data(messages);
                self.status = if self.sms_error.is_some() {
                    format!("{send_notice}；短信列表读取失败，请刷新")
                } else {
                    send_notice
                };
                Self::scroll_to_end(&self.scroll_aux);
            }
            Outcome::SmsStatus(send_status) => {
                self.status = send_status.summary_label();
                self.error = send_status.state == "failed";
                self.last_sms_status = Some(send_status);
            }
            Outcome::AmooState(snapshot, msg, start_poll) => {
                let count = snapshot.turns.len();
                if !self.demo_mode && !snapshot.turns.is_empty() {
                    if let Some(sess) = self.amoo_sessions.get_mut(self.amoo_session_idx) {
                        let mut new_items = Vec::new();
                        for t in &snapshot.turns {
                            if t.is_user {
                                new_items.push(AmooChatItem::UserText {
                                    attachment: None,
                                    text: t.text.clone(),
                                });
                            } else {
                                let has_bolt = t.text.contains("背屏") || t.text.contains("点阵");
                                new_items.push(AmooChatItem::AssistantRich {
                                    thinking_time: "已完成".into(),
                                    thinking_steps: vec![
                                        format!("会话：{}", sess.title),
                                        "来源：手机 Amoo (com.stepos.ailauncher) 实时同步".into(),
                                    ],
                                    tools_summary: "手机 Amoo · 实机消息".into(),
                                    text: Some(t.text.clone()),
                                    has_bolt_artifact: has_bolt,
                                    has_invoice_artifact: false,
                                    has_safety_confirm: false,
                                    time_label: sess.status.clone(),
                                });
                            }
                        }
                        if !new_items.is_empty() {
                            sess.items = new_items;
                        }
                    }
                } else if let Some(last_ai) = snapshot
                    .turns
                    .iter()
                    .rev()
                    .find(|t| !t.is_user && !t.text.trim().is_empty())
                {
                    if let Some(sess) = self.amoo_sessions.get_mut(self.amoo_session_idx) {
                        if let Some(AmooChatItem::AssistantRich {
                            text,
                            tools_summary,
                            ..
                        }) = sess.items.last_mut()
                        {
                            if text.is_some() {
                                *text = Some(last_ai.text.clone());
                                *tools_summary = "手机端 Amoo 实时回复".into();
                            }
                        }
                    }
                }
                self.amoo_snapshot = Some(snapshot);
                if self.page == Page::Amoo {
                    self.reveal_latest_chat();
                }
                if start_poll {
                    self.fields[F_AMOO].clear();
                    self.amoo_poll_remaining = 4;
                    self.last_amoo_poll = Instant::now();
                }
                self.status = if msg.is_empty() {
                    format!("已同步 {count} 条消息")
                } else {
                    msg
                };
            }
            Outcome::AmooCreated(key) => {
                self.amoo_sessions.insert(
                    0,
                    AmooSessionItem {
                        session_key: key,
                        title: "新对话".into(),
                        agent: "Amoo 助手".into(),
                        status: "待提问".into(),
                        status_kind: 0,
                        meta: "后台已创建 · 尚无消息".into(),
                        items: Vec::new(),
                    },
                );
                self.amoo_session_idx = 0;
                self.amoo_poll_remaining = 0;
                self.status = "已在后台创建 Amoo 会话，可直接输入消息".into();
            }
            Outcome::WorkspaceDelivered(task, path, serial, result) => {
                let note = result
                    .clone()
                    .unwrap_or_else(|e| format!("发送未完成：{e}"));
                self.status = note.clone();
                self.error = result.is_err();
                if let Some(task) = self
                    .workspace
                    .journal
                    .tasks
                    .iter_mut()
                    .find(|t| t.id == task)
                {
                    task.receipts
                        .push(stepx_companion::amoo_workspace::Receipt {
                            path,
                            note,
                            success: result.is_ok(),
                            device: serial,
                            account: None,
                            cloud_url: None,
                        });
                }
                self.workspace_persist();
            }
            Outcome::AmooSent(receipt) => {
                self.workspace_bind_session(&receipt.key);
                let key = receipt.key.clone();
                let text = receipt.text.clone();
                self.amoo_ui
                    .session_started
                    .insert(key.clone(), Instant::now());
                self.amoo_pending_reply.insert(key.clone(), receipt);
                if let Some(session) = self.amoo_sessions.iter_mut().find(|s| s.session_key == key)
                {
                    session.items.push(AmooChatItem::UserText {
                        attachment: None,
                        text,
                    });
                    session.status = "等待回复".into();
                    session.status_kind = 1;
                    session.meta = "后台已接收 · 正在等待回复".into();
                }
                self.amoo_poll_remaining = 100;
                self.last_amoo_poll = Instant::now();
                self.status = "Amoo 后台已接收消息，正在等待回复".into();
                self.reveal_latest_chat();
            }
            Outcome::AmooObservationUnavailable => {
                // Idle reads are not task failures. Retain the loaded history,
                // draft and attachments while marking the connection unconfirmed.
                self.amoo_gateway_online = false;
                self.amoo_poll_remaining = 0;
                self.status = "手机 Amoo 暂未连接，已保留上次内容；发送任务时会重新连接".into();
            }
            Outcome::AmooRealData(real, mut msg) => {
                // Measure completion before removing the matching pending run.
                self.record_amoo_usage(&real);
                self.reply_motion
                    .live
                    .extend(self.amoo_pending_reply.keys().cloned());
                let was_pending = self.amoo_pending_reply.len();
                let finished_error = self.amoo_pending_reply.values().find_map(|pending| {
                    let session = real.sessions.iter().find(|s| pending.finished(s))?;
                    session
                        .messages
                        .iter()
                        .rev()
                        .find(|m| m.run_id == pending.run_id && m.is_error)
                        .map(|m| {
                            m.error_message
                                .clone()
                                .unwrap_or_else(|| "手机返回了错误，请查看会话后继续".into())
                        })
                });
                if let Some(run) = self.workspace.running.clone().filter(|r| {
                    matches!(
                        r.executor,
                        stepx_companion::amoo_workspace::Executor::Phone
                            | stepx_companion::amoo_workspace::Executor::Image
                    )
                }) {
                    let key = self
                        .workspace
                        .journal
                        .tasks
                        .iter()
                        .find(|t| t.id == run.task)
                        .and_then(|t| t.session.as_ref());
                    let completed = key
                        .and_then(|key| self.amoo_pending_reply.get(key))
                        .and_then(|pending| {
                            real.sessions
                                .iter()
                                .find(|s| pending.finished(s))
                                .map(|s| (s, pending))
                        });
                    if let Some((session, pending)) = completed {
                        let failure = session
                            .messages
                            .iter()
                            .rev()
                            .find(|m| m.run_id == pending.run_id && m.is_error)
                            .map(|m| {
                                m.error_message
                                    .clone()
                                    .unwrap_or("手机返回错误，请查看回复".into())
                            });
                        let note = session
                            .messages
                            .iter()
                            .filter(|m| m.run_id == pending.run_id && m.role == "assistant")
                            .filter_map(|m| {
                                stepx_companion::work_protocol::display_text(&m.text, false)
                            })
                            .filter(|t| !t.trim().is_empty())
                            .collect::<Vec<_>>()
                            .join("\n\n");
                        if let Some(error) = failure {
                            self.workspace_finish(
                                stepx_companion::amoo_workspace::Phase::Failed,
                                error,
                            );
                        } else if run.executor == stepx_companion::amoo_workspace::Executor::Image {
                            self.workspace_deliver_images(
                                session
                                    .messages
                                    .iter()
                                    .filter(|m| m.run_id == pending.run_id && m.role != "user")
                                    .cloned()
                                    .collect(),
                                note,
                            );
                        } else {
                            self.workspace_finish(
                                stepx_companion::amoo_workspace::Phase::Done,
                                note,
                            );
                        }
                    }
                }
                self.amoo_pending_reply
                    .retain(|_, pending| !real.sessions.iter().any(|s| pending.finished(s)));
                if self.amoo_pending_reply.len() < was_pending {
                    if self.amoo_pending_reply.is_empty() {
                        self.amoo_poll_remaining = 0;
                    }
                    if let Some(error) = finished_error {
                        msg = error.clone();
                        self.work.error = Some(error);
                        self.error = true;
                    } else {
                        msg = "已收到 Amoo 本次请求的结果".into();
                    }
                } else if !self.amoo_pending_reply.is_empty() && self.amoo_poll_remaining == 0 {
                    msg = "Amoo 尚未返回完整结果，后台会继续同步；请勿重复发送".into();
                }
                let follow_tail = self.chat_near_end() && self.selected_text.is_none();
                let previous = self
                    .amoo_sessions
                    .get(self.amoo_session_idx)
                    .map(|s| (s.session_key.clone(), s.items.clone()));
                self.apply_real_amoo_data_recorded(real);
                let current = self
                    .amoo_sessions
                    .get(self.amoo_session_idx)
                    .map(|s| (s.session_key.clone(), s.items.clone()));
                if matches!(self.page, Page::Amoo | Page::Work)
                    && previous != current
                    && follow_tail
                {
                    self.reveal_latest_chat();
                }
                if !msg.is_empty() {
                    self.status = msg;
                }
            }
            Outcome::Files(path, files, notice) => {
                if path != self.directory {
                    self.scroll = Default::default();
                    self.fields[F_FILE_SEARCH].clear();
                    if self.focus == Some(F_FILE_SEARCH) {
                        self.focus = None;
                    }
                }
                self.directory = path;
                self.files = files;
                self.file_page = 0;
                self.selected_file = None;
                self.file_modal = None;
                self.trash_confirm = false;
                if let Some(msg) = notice {
                    self.record_notice("文件操作", msg.clone());
                    self.status = msg;
                }
            }
            Outcome::Apps(apps) => {
                self.apps_loaded = true;
                self.apps_error = None;
                self.status = format!("已读取 {} 个应用", apps.len());
                self.apps = apps;
            }
            Outcome::ApkInstalled(msg, apps) => {
                self.apps_loaded = true;
                self.apps_error = None;
                self.status = msg;
                self.apps = apps;
            }
            Outcome::Mirror(serial, transport, child) => {
                if self
                    .selected
                    .as_ref()
                    .is_some_and(|d| d.serial == serial && d.transport == transport)
                {
                    self.status = child.started_message().into();
                    self.mirror = Some(child);
                } else {
                    // Dropping the session closes a window started for a stale selection.
                    self.status = "设备已切换，已取消原设备的投屏".into();
                }
            }
            Outcome::Screenshot(directory, path) => {
                self.screenshot = Some((directory, path));
                self.status = "已截图".into();
            }
            Outcome::Trash(record) => {
                self.trashed = Some(record);
                self.status = "已移到回收站".into();
                self.browse(self.directory.clone());
            }
            Outcome::Restored => {
                self.trashed = None;
                self.status = "已恢复".into();
                self.browse(self.directory.clone());
            }
            Outcome::Qr(endpoint) => {
                if let Some(endpoint) = endpoint {
                    self.remember_pairing_host(&endpoint);
                    self.qr = None;
                    self.status = "配对成功，正在连接…".into();
                    self.refresh();
                } else {
                    self.refresh();
                    self.status = "等待扫码 · 正在检查设备通道…".into();
                }
            }
            Outcome::Paired(endpoint) => {
                self.remember_pairing_host(&endpoint);
                self.fields[F_PAIR_CODE].clear();
                self.status = format!("已连接 {endpoint}，正在验证…");
                self.refresh();
            }
            Outcome::BackScreenImportedImage(path) => {
                self.page = Page::BackScreen;
                self.back_screen.tab = 2;
                match self.back_screen.apply_image(path, "手机文件") {
                    Ok(()) => {
                        self.status = "正在转换手机图片，请预览后播放".into();
                    }
                    Err(error) => {
                        self.status = error.to_string();
                        self.error = true;
                    }
                }
            }
            Outcome::Studio(result) => self.apply_studio(result),
            Outcome::StudioLab(result) => self.apply_studio_lab(result),
            Outcome::PhoneData(logs, contacts) => {
                self.apply_phone_data(logs, contacts);
            }
            Outcome::PhoneCallPrepared(plan) => {
                if self
                    .selected
                    .as_ref()
                    .is_some_and(|d| d.serial == plan.device_serial)
                    && phone::normalize_number(&self.fields[F_PHONE_DIAL].text)
                        .ok()
                        .as_deref()
                        == Some(plan.number.as_str())
                    && self
                        .selected_call_sim
                        .map_or(true, |id| id == plan.subscription)
                {
                    self.status = format!(
                        "请确认通过 {} 呼叫 {}",
                        plan.subscription_label, plan.number
                    );
                    let target = format!("手机：{}", plan.device_serial);
                    self.review_action(
                        &Action::PhoneConfirmCall,
                        "确认用手机拨打？",
                        target,
                        vec![
                            format!("号码：{}", plan.number),
                            format!("SIM：{}", plan.subscription_label),
                            "确认后手机将发起实际通话，可能产生运营商费用。".into(),
                        ],
                        "确认拨打",
                        true,
                        false,
                    );
                    self.pending_call = Some(plan);
                }
            }
            Outcome::DialSubmitted(number, notice) => {
                self.active_call_number = Some(number);
                self.status = notice;
            }
            Outcome::CallHangup(notice) => {
                self.active_call_number = None;
                self.status = notice;
            }
            Outcome::GoogleSuite(suite, companion, msg) => {
                self.google_suite = suite;
                self.companion_status = companion;
                self.status = msg;
            }
            Outcome::NeoStatus(neo) => {
                self.status = neo.message.clone();
                self.neo_status = neo;
            }
            Outcome::CompanionStatus(companion, msg) => {
                self.companion_status = companion;
                self.status = msg;
            }
            Outcome::InputMethod(report, message) => {
                self.fcitx5_active = report.active;
                self.input_method = report;
                self.status = message;
            }
            Outcome::SystemServices(services, msg) => {
                self.fcitx5_active = services
                    .iter()
                    .find(|s| s.package == "org.fcitx.fcitx5.android")
                    .map(|s| s.status_note.contains("当前默认"))
                    .unwrap_or(self.fcitx5_active);
                self.system_services = services;
                self.status = msg;
            }
            Outcome::DeepAmooGranted(msg) => {
                self.deep_amoo_granted = true;
                self.amoo_bg_keepalive = true;
                self.status = msg;
            }
            Outcome::DeepBackScreenGranted(msg) => {
                self.deep_backscreen_granted = true;
                self.status = msg;
            }
        }
    }

    fn apply_capabilities(&mut self, caps: Option<Capabilities>) {
        let selected = caps.as_ref().and_then(|c| {
            self.selected_sim
                .filter(|id| c.subscriptions.iter().any(|s| s.id == *id))
                .or_else(|| {
                    c.subscriptions
                        .iter()
                        .find(|s| s.id == c.default_subscription)
                        .map(|s| s.id)
                })
                .or_else(|| (c.subscriptions.len() == 1).then(|| c.subscriptions[0].id))
        });
        if self.selected_sim != selected {
            self.pending_sms_plan = None;
        }
        self.selected_sim = selected;
        self.capabilities = caps;
    }

    fn apply_sms_data(&mut self, result: ReadResult<Message>) {
        self.sms_loaded = result.is_ok();
        match result {
            Ok(messages) => {
                self.status = format!("已从手机读取 {} 条短信", messages.len());
                self.sms = messages;
                self.sms_error = None;
                // Never leave a deleted thread selected or a previous recipient in the composer.
                if self.selected_thread.as_deref().is_some_and(|addr| {
                    addr != "__new__"
                        && !self.sms.iter().any(|m| {
                            stepx_companion::sms::normalize_address(&m.address)
                                == stepx_companion::sms::normalize_address(addr)
                        })
                }) {
                    self.selected_thread = None;
                    self.fields[F_SMS_TO].clear();
                    self.pending_sms_plan = None;
                }
            }
            Err(error) => {
                self.sms.clear();
                self.status = format!("短信读取失败：{error}");
                self.sms_error = Some(error);
                self.error = true;
            }
        }
    }

    fn apply_phone_data(
        &mut self,
        logs: ReadResult<CallRecord>,
        contacts: ReadResult<ContactEntry>,
    ) {
        self.phone_loaded = true;
        match logs {
            Ok(logs) => {
                self.call_logs = logs;
                self.call_logs_error = None;
            }
            Err(error) => {
                self.call_logs.clear();
                self.call_logs_error = Some(error);
            }
        }
        match contacts {
            Ok(contacts) => {
                self.contacts = contacts;
                self.contacts_error = None;
            }
            Err(error) => {
                self.contacts.clear();
                self.contacts_error = Some(error);
            }
        }
        self.error = self.call_logs_error.is_some() || self.contacts_error.is_some();
        self.status = if self.error {
            "部分手机数据读取失败，详情见页面".into()
        } else {
            format!(
                "已同步 {} 条通话记录与 {} 位联系人",
                self.call_logs.len(),
                self.contacts.len()
            )
        };
        self.selected_contact = 0;
    }

    /// Reconnect the remembered phone, or auto-select the only StepX phone on first launch.
    fn auto_select(&mut self) {
        let candidates: Vec<_> = self
            .devices
            .iter()
            .filter(|d| d.state == "device" && is_stepx_model(&d.model))
            .map(|d| d.id.clone())
            .collect();
        if let Some(preferred) = self.preferences.selected_serial.clone() {
            let resuming = self.connection_watch.detached_serial.clone();
            if !candidates.is_empty() {
                let allow_fallback = self.allow_first_connect && candidates.len() == 1;
                self.submit("正在连接 StepX 设备…", move |adb| {
                    for id in &candidates {
                        if let Ok(device) = Device::inspect(adb, id) {
                            if device.serial == preferred || allow_fallback {
                                if resuming.as_deref() == Some(device.serial.as_str()) {
                                    return Ok(Outcome::Reattached(device));
                                }
                                let summary = device.summary(adb)?;
                                let caps = bridge::capabilities(adb, &device).ok();
                                let real_amoo = None; // Load conversations lazily on the Amoo page.
                                let sms = read_result(device.messages(adb));
                                let call_logs = read_result(device.call_logs(adb));
                                let contacts = read_result(device.contacts(adb));
                                return Ok(Outcome::Selected(
                                    device, summary, caps, real_amoo, sms, call_logs, contacts,
                                ));
                            }
                        }
                    }
                    Ok(Outcome::Notice("已记住的设备不在线".into()))
                });
            } else {
                self.initializing = false;
                self.status = "已记住的设备不在线".into();
            }
        } else if candidates.len() == 1 && self.allow_first_connect {
            let id = candidates[0].clone();
            self.select(id);
        } else if self.devices.is_empty() {
            self.initializing = false;
            self.status = "未连接设备".into();
        } else {
            self.initializing = false;
            self.status = format!("发现 {} 台设备", self.devices.len());
        }
    }

    fn select(&mut self, id: String) {
        self.submit("正在验证设备…", move |adb| {
            let device = Device::inspect(adb, &id)?;
            let summary = device.summary(adb)?;
            let caps = bridge::capabilities(adb, &device).ok();
            let real_amoo = None; // Load conversations lazily on the Amoo page.
            let sms = read_result(device.messages(adb));
            let call_logs = read_result(device.call_logs(adb));
            let contacts = read_result(device.contacts(adb));
            Ok(Outcome::Selected(
                device, summary, caps, real_amoo, sms, call_logs, contacts,
            ))
        });
    }

    fn apply_real_amoo_data(&mut self, real: RealAmooData) {
        self.record_amoo_usage(&real);
        self.apply_real_amoo_data_recorded(real);
    }

    fn apply_real_amoo_data_recorded(&mut self, real: RealAmooData) {
        self.record_work_attachment_identities(&real);
        for session in &real.sessions {
            if !session.model.is_empty() {
                self.work
                    .session_models
                    .insert(session.key.clone(), session.model.clone());
            }
        }
        self.amoo_gateway_version = real.gateway_version;
        self.amoo_gateway_online = real.gateway_online;
        self.amoo_cron_count = real.cron_jobs.len();
        self.amoo_cron_jobs = real.cron_jobs.clone();
        self.amoo_bm25_entries = real.memory_bm25_entries;
        self.amoo_total_skills = real.total_skills_count;
        self.amoo_memory_enabled = real.memory_initialized;

        let current_key = self
            .amoo_sessions
            .get(self.amoo_session_idx)
            .map(|s| s.session_key.clone());

        let current_key = real.selected_session.or(current_key);
        let mut new_sessions = Vec::new();
        for s in real.sessions {
            let mut items = if s.history_loaded || !s.messages.is_empty() {
                Vec::new()
            } else {
                self.amoo_sessions
                    .iter()
                    .find(|old| old.session_key == s.key)
                    .map(|old| old.items.clone())
                    .unwrap_or_default()
            };
            items.extend(amoo_display::chat_items(&s.key, s.messages, s.status_kind));
            let pending = self.amoo_pending_reply.contains_key(&s.key);
            new_sessions.push(AmooSessionItem {
                session_key: s.key,
                title: s.title,
                agent: s.agent,
                status: if pending {
                    "等待完整结果".into()
                } else {
                    s.status
                },
                status_kind: if pending { 1 } else { s.status_kind },
                meta: s.meta,
                items,
            });
        }

        for old in &self.amoo_sessions {
            if (old.status == "待提问" || self.amoo_pending_reply.contains_key(&old.session_key))
                && !new_sessions
                    .iter()
                    .any(|s| s.session_key == old.session_key)
            {
                new_sessions.push(old.clone());
            }
        }
        if !self.settings.value.reduced_motion && !self.settings.hidden {
            for next in &new_sessions {
                if current_key.as_deref() != Some(&next.session_key)
                    || !(self.amoo_pending_reply.contains_key(&next.session_key)
                        || self.reply_motion.live.contains(&next.session_key))
                {
                    continue;
                }
                let Some(old) = self
                    .amoo_sessions
                    .iter()
                    .find(|s| s.session_key == next.session_key)
                else {
                    continue;
                };
                for (i, item) in next.items.iter().enumerate() {
                    if let AmooChatItem::AssistantRich {
                        text: Some(text), ..
                    } = item
                    {
                        let previous = match old.items.get(i) {
                            Some(AmooChatItem::AssistantRich { text: Some(t), .. }) => t.as_str(),
                            _ => "",
                        };
                        let before = stepx_companion::work_protocol::display_text(previous, false)
                            .unwrap_or_default();
                        let after = stepx_companion::work_protocol::display_text(text, false)
                            .unwrap_or_default();
                        self.reply_motion
                            .track(next.session_key.clone(), i, &before, &after);
                    }
                }
            }
        }
        self.reply_motion.live.clear();
        self.amoo_sessions = new_sessions;
        // Passive updates preserve the user's session-list scroll position.
        if let Some(key) = current_key {
            if let Some(pos) = self.amoo_sessions.iter().position(|s| s.session_key == key) {
                self.amoo_session_idx = pos;
            } else {
                self.amoo_session_idx = 0;
            }
        } else {
            self.amoo_session_idx = 0;
        }

        {
            self.amoo_skills = real
                .skills
                .into_iter()
                .map(|sk| AmooSkillEntry {
                    skill_key: sk.skill_key,
                    name: sk.display_name,
                    enabled: sk.enabled,
                    prompt: sk.prompt,
                })
                .collect();
        }

        let mut user_mems: Vec<AmooMemoryEntry> = self
            .amoo_memories
            .iter()
            .filter(|m| m.sub.starts_with("桌面") || m.sub.starts_with("对话"))
            .cloned()
            .collect();
        for (title, sub) in real.memory_entries {
            user_mems.push(AmooMemoryEntry { title, sub });
        }
        self.amoo_memories = user_mems;
        self.load_work_previews();
    }

    fn remember_pairing_host(&mut self, endpoint: &str) {
        if let Ok(address) = pairing::validate_endpoint(endpoint) {
            if !self.pairing_hosts.contains(&address.ip()) {
                self.pairing_hosts.push(address.ip());
            }
        }
    }

    fn browse(&mut self, path: String) {
        if self.busy {
            return;
        }
        self.with_device("正在读取目录…", move |adb, device| {
            Ok(Outcome::Files(
                path.clone(),
                storage::list(adb, &device, &path)?,
                None,
            ))
        });
    }

    /// Chat-like panes start at the newest message; the scroll view clamps the offset.
    fn scroll_to_end(state: &ClippedScrollStateHandle) {
        state.scroll_to(facloud_ui::units::Pixels::new(1.0e7));
    }

    fn stop_mirror(&mut self) {
        if let Some(child) = self.mirror.take() {
            child.stop_in_background();
        }
    }

    fn filtered_files(&self) -> Vec<(usize, &Entry)> {
        let query = self.fields[F_FILE_SEARCH].text.trim().to_lowercase();
        let mut items: Vec<(usize, &Entry)> = self
            .files
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                query.is_empty()
                    || e.name.to_lowercase().contains(&query)
                    || e.kind_badge().contains(&query)
            })
            .collect();
        match self.file_sort {
            FileSort::Name => {}
            FileSort::Size => items.sort_by(|(_, a), (_, b)| {
                b.directory
                    .cmp(&a.directory)
                    .then(b.bytes.cmp(&a.bytes))
                    .then(a.name.cmp(&b.name))
            }),
            FileSort::Kind => items.sort_by(|(_, a), (_, b)| {
                b.directory
                    .cmp(&a.directory)
                    .then(a.kind_badge().cmp(b.kind_badge()))
                    .then(a.name.cmp(&b.name))
            }),
        }
        items
    }

    pub(super) fn thread_messages(&self, address: &str) -> Vec<&Message> {
        let norm_target = stepx_companion::sms::normalize_address(address);
        let mut items: Vec<&Message> = self
            .sms
            .iter()
            .filter(|m| stepx_companion::sms::normalize_address(&m.address) == norm_target)
            .collect();
        items.sort_by_key(|m| m.date_ms);
        items
    }

    pub(super) fn sms_threads(&self) -> Vec<SmsThread> {
        let query = self.fields[F_SMS_SEARCH].text.trim().to_lowercase();
        let mut threads: Vec<SmsThread> = Vec::new();
        for m in &self.sms {
            let matches_filter = match self.sms_filter {
                SmsFilter::All => true,
                SmsFilter::Incoming => m.kind == 1,
                SmsFilter::Outgoing => m.kind == 2,
            };
            if !matches_filter
                || (!query.is_empty()
                    && !m.address.to_lowercase().contains(&query)
                    && !m.body.to_lowercase().contains(&query)
                    && !self.contacts.iter().any(|c| {
                        stepx_companion::sms::normalize_address(&c.number)
                            == stepx_companion::sms::normalize_address(&m.address)
                            && c.name.to_lowercase().contains(&query)
                    }))
            {
                continue;
            }
            let addr = m.address.trim().to_string();
            if let Some(t) = threads.iter_mut().find(|t| {
                stepx_companion::sms::normalize_address(&t.address)
                    == stepx_companion::sms::normalize_address(&addr)
            }) {
                t.count += 1;
                t.unread |= m.kind == 1 && !m.read;
                if m.date_ms >= t.last_ms {
                    t.last_ms = m.date_ms;
                    t.last_body = m.body.clone();
                }
            } else {
                threads.push(SmsThread {
                    address: addr,
                    count: 1,
                    last_ms: m.date_ms,
                    last_body: m.body.clone(),
                    unread: m.kind == 1 && !m.read,
                });
            }
        }
        threads.sort_by(|a, b| b.last_ms.cmp(&a.last_ms));
        threads
    }

    fn filtered_apps(&self) -> Vec<&AppInfo> {
        let query = self.fields[F_APP_SEARCH].text.trim();
        self.apps
            .iter()
            .filter(|app| {
                (self.include_system_apps || app.in_default_scope()) && app.matches_query(query)
            })
            .collect()
    }
}

impl Entity for Companion {
    type Event = ();
}

impl View for Companion {
    fn ui_name() -> &'static str {
        "StepXCompanion"
    }

    fn on_window_closed(&mut self, ctx: &mut ViewContext<Self>) {
        self.remember_window(ctx, true);
        self.stop_mirror();
        self.close_mirror_controls(ctx);
        self.qr = None;
    }

    fn active_cursor_position(&self, ctx: &ViewContext<Self>) -> Option<facloud_ui::CursorInfo> {
        let index = self.focus?;
        Some(facloud_ui::CursorInfo {
            position: ctx.element_position_by_id(format!("input-caret:{index}"))?,
            font_size: theme::BODY,
        })
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let _timing = perf::Span::new("render");
        let epoch = self.render_epoch.get().wrapping_add(1);
        self.render_epoch.set(epoch);
        if self.hover_states.borrow().len() > 512 {
            self.hover_states
                .borrow_mut()
                .retain(|_, (last, _)| epoch.wrapping_sub(*last) <= 2);
        }
        let p = Palette::for_theme(self.theme_mode.is_dark(app.system_theme()));
        self.palette.set(p);

        let tree = if self.push.shell_mode != ShellMode::Full {
            self.compact_shell()
        } else {
            let body = if self.initializing && self.busy && self.page.needs_device() {
                self.loading_page()
            } else if self.page.needs_device()
                && self.selected.is_none()
                && self.page != Page::Overview
            {
                self.no_device()
            } else if matches!(self.page, Page::Tools | Page::Connect) {
                self.scrollable(self.scroll.clone(), self.page_content())
            } else if self.page.fills_height() {
                self.page_content()
            } else {
                Container::new(self.page_content())
                    .with_uniform_padding(theme::S5)
                    .finish()
            };

            let body = self.loading_feedback(body);
            let body = motion::enter(
                body,
                self.motion_started,
                !self.settings.value.reduced_motion && !self.settings.hidden,
            );
            let main = Container::new(
                Flex::column()
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                    .with_child(self.header())
                    .with_child(Expanded::new(1., body).finish())
                    .finish(),
            )
            .with_background_color(p.workspace)
            .with_corner_radius(CornerRadius::with_top_right(Radius::Pixels(theme::R_PANEL)))
            .finish();

            let top = Flex::row()
                .with_main_axis_size(MainAxisSize::Max)
                .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                .with_child(self.sidebar())
                .with_child(Expanded::new(1., main).finish())
                .finish();

            let outer_border_color = if p.dark {
                p.border_strong
            } else {
                theme::rgb(0xDC, 0xDC, 0xE2)
            };
            let tree = Container::new(
                Flex::column()
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                    .with_child(Expanded::new(1., top).finish())
                    .with_child(self.status_bar())
                    .finish(),
            )
            .with_background_color(p.workspace)
            .with_border(Border::all(1.).with_border_color(outer_border_color))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(12.)))
            .finish();

            tree
        };
        #[cfg(feature = "ui-capture")]
        let tree = self.text_test_fixture().unwrap_or(tree);
        #[cfg(feature = "ui-capture")]
        let tree = SavePosition::new(tree, "test-window-layout")
            .for_single_frame()
            .finish();
        // SavePosition commits in a Stack paint scope, including the pinned composer.
        let tree = Stack::new().with_child(tree).finish();
        let tree = if let Some(sheet) = self.desktop_update_overlay() {
            Stack::new()
                .with_child(adb_onboarding::Inert(tree).finish())
                .with_child(sheet)
                .finish()
        } else if let Some(menu) = self.context_overlay() {
            Stack::new()
                .with_child(adb_onboarding::Inert(tree).finish())
                .with_child(menu)
                .finish()
        } else if let Some(sheet) = self.interaction_overlay() {
            Stack::new()
                .with_child(adb_onboarding::Inert(tree).finish())
                .with_child(sheet)
                .finish()
        } else if let Some(step) = self.adb_guide_step {
            Stack::new()
                .with_child(adb_onboarding::Inert(tree).finish())
                .with_child(self.adb_guide_modal(step))
                .finish()
        } else if let Some(viewer) = self.amoo_image_viewer() {
            Stack::new()
                .with_child(adb_onboarding::Inert(tree).finish())
                .with_child(viewer)
                .finish()
        } else if let Some(toast_overlay) = self.push_toast_overlay() {
            Stack::new()
                .with_child(tree)
                .with_child(toast_overlay)
                .finish()
        } else {
            tree
        };
        Keyboard {
            accept_attachments: self.page == Page::Amoo
                && !self.busy
                && !self.workspace_busy()
                && self.context_menu.is_none()
                && self.workspace.proposal.is_none()
                && self.interaction_overlay().is_none()
                && self.amoo_image_viewer().is_none(),
            context_menu: self.context_menu.is_some(),
            child: perf::Measured(tree).finish(),
            shortcut: self.settings.value.compact_shortcut.clone(),
            recording_shortcut: self.shortcuts.recording,
            active: self.focus.is_some(),
            multiline: matches!(
                self.focus,
                Some(F_WORK_PROMPT | F_AMOO | F_SMS_BODY | F_LAB_PROMPT)
            ),
            composing: self
                .focus
                .is_some_and(|i| !self.fields[i].preedit.is_empty()),
        }
        .finish()
    }
}

impl Drop for Companion {
    fn drop(&mut self) {
        self.workspace_flush();
        self.stop_mirror();
    }
}

/// Handles committed IME text separately from key presses so Chinese input is neither lost
/// nor inserted twice, and maps Cmd (macOS) / Ctrl (Windows, Linux) shortcuts.
struct Keyboard {
    accept_attachments: bool,
    context_menu: bool,
    shortcut: stepx_companion::settings::CompactShortcut,
    recording_shortcut: bool,
    child: Box<dyn Element>,
    active: bool,
    multiline: bool,
    composing: bool,
}
impl Element for Keyboard {
    fn layout(&mut self, c: SizeConstraint, ctx: &mut LayoutContext, app: &AppContext) -> Vector2F {
        self.child.layout(c, ctx, app)
    }
    fn after_layout(&mut self, ctx: &mut AfterLayoutContext, app: &AppContext) {
        self.child.after_layout(ctx, app);
    }
    fn paint(&mut self, o: Vector2F, ctx: &mut PaintContext, app: &AppContext) {
        self.child.paint(o, ctx, app);
        if self.active {
            app.report_active_cursor_position_update();
        }
    }
    fn size(&self) -> Option<Vector2F> {
        self.child.size()
    }
    fn origin(&self) -> Option<Point> {
        self.child.origin()
    }
    fn dispatch_event(
        &mut self,
        event: &DispatchedEvent,
        ctx: &mut EventContext,
        app: &AppContext,
    ) -> bool {
        #[cfg(feature = "ui-capture")]
        if std::env::var_os("STEPX_ENTER_TEST").is_some() {
            if let Event::KeyDown {
                keystroke,
                is_composing,
                ..
            } = event.raw_event()
            {
                eprintln!(
                    "ENTER_KEY key={} shift={} active={} composing={}/{}",
                    keystroke.key, keystroke.shift, self.active, self.composing, is_composing
                );
            }
            match event.raw_event() {
                Event::TypedCharacters { chars } => eprintln!(
                    "ENTER_TEXT bytes={} control={:?}",
                    chars.len(),
                    chars.chars().filter(|c| c.is_control()).collect::<Vec<_>>()
                ),
                Event::SetMarkedText { marked_text, .. } => {
                    eprintln!("ENTER_PREEDIT bytes={}", marked_text.len())
                }
                Event::ClearMarkedText => eprintln!("ENTER_PREEDIT clear"),
                _ => {}
            }
        }
        if self.context_menu {
            if let Event::KeyDown { keystroke, .. } = event.raw_event() {
                let a = match keystroke.key.as_str() {
                    "up" => Some(Action::ContextNavigate(-1)),
                    "down" => Some(Action::ContextNavigate(1)),
                    "enter" | "return" => Some(Action::ContextActivate),
                    "escape" | "tab" => Some(Action::ContextClose),
                    _ => None,
                };
                if let Some(a) = a {
                    ctx.dispatch_typed_action(a);
                    return true;
                }
            }
        }
        if let Event::KeyDown {
            keystroke,
            is_composing: false,
            ..
        } = event.raw_event()
        {
            if !self.composing {
                let action = if self.recording_shortcut {
                    Some(Action::Settings(if keystroke.key == "escape" {
                        SettingsAction::CancelShortcut
                    } else {
                        SettingsAction::CaptureShortcut(shortcuts::Shortcuts::captured(keystroke))
                    }))
                } else if shortcuts::Shortcuts::matches(&self.shortcut, keystroke) {
                    Some(Action::CompactChat)
                } else {
                    None
                };
                if let Some(action) = action {
                    ctx.dispatch_typed_action(action);
                    return true;
                }
            }
        }
        if self.recording_shortcut
            && matches!(
                event.raw_event(),
                Event::TypedCharacters { .. } | Event::SetMarkedText { .. }
            )
        {
            return true;
        }
        if self.child.dispatch_event(event, ctx, app) {
            return true;
        }
        let action = self.action_for_event(event.raw_event());
        if let Some(action) = action {
            ctx.dispatch_typed_action(action);
            true
        } else {
            false
        }
    }
}

impl Keyboard {
    fn action_for_event(&self, event: &Event) -> Option<Action> {
        match event {
            Event::DragAndDropFiles { paths, .. } if self.accept_attachments => Some(
                Action::AmooDropFiles(paths.iter().map(PathBuf::from).collect()),
            ),
            Event::TypedCharacters { chars } if self.active => Some(Action::Insert(chars.clone())),
            Event::SetMarkedText { marked_text, .. } if self.active => {
                Some(Action::Preedit(marked_text.clone()))
            }
            Event::ClearMarkedText if self.active => Some(Action::Preedit(String::new())),
            Event::KeyDown {
                keystroke: k,
                is_composing: false,
                ..
            } if !self.composing => {
                let cmd = k.ctrl || k.cmd;
                let typing = self.active;
                match k.key.as_str() {
                    "1" if cmd => Some(Action::Page(Page::Overview)),
                    "2" if cmd => Some(Action::Page(Page::Amoo)),
                    "3" if cmd => Some(Action::Page(Page::Sms)),
                    "4" if cmd => Some(Action::Page(Page::Files)),
                    "5" if cmd => Some(Action::Page(Page::Apps)),
                    "6" if cmd => Some(Action::Page(Page::Connect)),
                    "r" if cmd => Some(Action::ContextRefresh),
                    "f5" => Some(Action::ContextRefresh),
                    "m" if cmd => Some(Action::Mirror),
                    "t" if cmd => Some(Action::CycleTheme),
                    "f" if cmd => Some(Action::FocusSearch),
                    "n" if cmd && !typing => Some(Action::OpenNewFolder),
                    "escape" => Some(Action::Escape),
                    "tab" => Some(Action::NextField(k.shift)),
                    "enter" | "return" if typing && k.shift && self.multiline => {
                        Some(Action::Insert("\n".into()))
                    }
                    "enter" | "return" if typing => Some(Action::SubmitFocused),
                    "enter" | "return" => Some(Action::ActivateSelectedFile),
                    "up" if !typing => Some(Action::MoveFileSelection(-1)),
                    "down" if !typing => Some(Action::MoveFileSelection(1)),
                    "backspace" if !typing => Some(Action::ParentDirectory),
                    "delete" if !typing => Some(Action::Trash),
                    "f2" if !typing => Some(Action::OpenRename),
                    "z" if cmd && k.shift && typing => Some(Action::RedoInput),
                    "z" if cmd && typing => Some(Action::UndoInput),
                    "y" if cmd && typing => Some(Action::RedoInput),
                    "v" if cmd && typing => Some(Action::Paste),
                    "c" if cmd && typing => Some(Action::CopyInput),
                    "c" if cmd => Some(Action::Copy),
                    "x" if cmd && typing => Some(Action::CutInput),
                    "a" if cmd && typing => Some(Action::SelectAll),
                    "backspace" if cmd => Some(Action::BackspaceLine),
                    "backspace" => Some(Action::Backspace),
                    "delete" => Some(Action::Delete),
                    "left" if k.shift && typing => {
                        Some(Action::ExtendSelection(if cmd { -2 } else { -1 }))
                    }
                    "right" if k.shift && typing => {
                        Some(Action::ExtendSelection(if cmd { 2 } else { 1 }))
                    }
                    "home" if k.shift && typing => Some(Action::ExtendSelection(-2)),
                    "end" if k.shift && typing => Some(Action::ExtendSelection(2)),
                    "left" if k.cmd && typing => Some(Action::Home),
                    "right" if k.cmd && typing => Some(Action::End),
                    "left" if typing => Some(Action::Left),
                    "right" if typing => Some(Action::Right),
                    "home" if typing => Some(Action::Home),
                    "end" if typing => Some(Action::End),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod input_tests {
    use super::*;

    fn keyboard(composing: bool) -> Keyboard {
        Keyboard {
            accept_attachments: false,
            context_menu: false,
            child: Empty::new().finish(),
            shortcut: Default::default(),
            recording_shortcut: false,
            active: true,
            multiline: true,
            composing,
        }
    }

    fn key(name: &str, is_composing: bool) -> Event {
        Event::KeyDown {
            keystroke: facloud_ui::keymap::Keystroke {
                key: name.into(),
                ..Default::default()
            },
            chars: String::new(),
            details: Default::default(),
            is_composing,
        }
    }

    #[test]
    fn ime_candidate_keys_do_not_submit_or_edit_committed_text() {
        for name in [
            "enter",
            "return",
            "backspace",
            "left",
            "right",
            "escape",
            "tab",
        ] {
            assert!(keyboard(true).action_for_event(&key(name, false)).is_none());
            assert!(keyboard(false).action_for_event(&key(name, true)).is_none());
        }
        assert!(matches!(
            keyboard(false).action_for_event(&key("enter", false)),
            Some(Action::SubmitFocused)
        ));
    }

    #[test]
    fn enter_and_shift_enter_share_input_rules() {
        for name in ["enter", "return"] {
            let mut event = key(name, false);
            assert!(matches!(
                keyboard(false).action_for_event(&event),
                Some(Action::SubmitFocused)
            ));
            if let Event::KeyDown { keystroke, .. } = &mut event {
                keystroke.shift = true;
            }
            assert!(
                matches!(keyboard(false).action_for_event(&event), Some(Action::Insert(text)) if text == "\n")
            );
            assert!(keyboard(true).action_for_event(&event).is_none());
        }
    }

    #[test]
    fn dropped_files_only_enter_an_available_composer() {
        let event = Event::DragAndDropFiles {
            paths: vec!["/tmp/example.csv".into()],
            location: Vector2F::new(20., 20.),
        };
        let mut input = keyboard(false);
        assert!(input.action_for_event(&event).is_none());
        input.accept_attachments = true;
        assert!(
            matches!(input.action_for_event(&event), Some(Action::AmooDropFiles(paths)) if paths == vec![PathBuf::from("/tmp/example.csv")])
        );
    }

    #[test]
    fn chinese_commit_replaces_selection_once_and_preserves_utf8_cursor() {
        let mut input = Input::default();
        input.set("旧内容");
        input.all = true;
        let action = keyboard(true)
            .action_for_event(&Event::SetMarkedText {
                marked_text: "ni hao".into(),
                selected_range: 0..6,
            })
            .unwrap();
        if let Action::Preedit(text) = action {
            input.preedit = text;
        } else {
            panic!("expected preedit");
        }
        assert_eq!(input.text, "旧内容");
        let action = keyboard(true)
            .action_for_event(&Event::TypedCharacters {
                chars: "你好🙂".into(),
            })
            .unwrap();
        if let Action::Insert(text) = action {
            input.insert(&text);
        } else {
            panic!("expected commit");
        }
        assert_eq!(input.text, "你好🙂");
        assert!(input.preedit.is_empty());
        input.cursor = input.previous();
        input.insert("，Amoo");
        assert_eq!(input.text, "你好，Amoo🙂");
        assert!(input.text.is_char_boundary(input.cursor));
    }
}
