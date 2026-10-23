//! Backscreen editor: edits are local until the user starts playback.
use super::*;
use stepx_companion::{backscreen as bs, backscreen_amoo as ba, companion_app as ca};

#[derive(Clone, Debug)]
pub(super) enum BackScreenAction {
    Tab(u8),
    SelectPreset(bs::Preset),
    SelectNative(bs::NativeFeature),
    SelectApp(String, String),
    RefreshGallery,
    RecentImage(usize),
    DisplayMode(String),
    SaveBrightness,
    SaveMarquee,
    ToggleMarquee,
    CameraOff,
    PrepareClear,
    CancelClear,
    ClearFrame,
    Brightness(u8),
    Fps(u8),
    Duration(u32),
    Invert,
    TogglePreview,
    Play,
    Stop,
    PickImage,
    Dither,
    Threshold,
    Fit,
    AmooMode(bool),
    Generate,
    CancelGenerate,
    Prompt(String),
    Advanced,
    CompanionInspect,
    CompanionInstall,
    GameArm,
    GameStart,
    CompanionStop,
}

enum CompanionEvent {
    Progress(String),
    Inspected(ca::CompanionStatus),
    Applied {
        title: String,
        revision: u64,
        message: String,
        mode: String,
    },
    TemporaryStarted {
        playback: bs::Playback,
        title: String,
        revision: u64,
    },
    Game {
        message: String,
        mode: String,
    },
    Stopped(String),
    Failed(String),
}

pub(super) struct BackScreenUiState {
    pub tab: u8,
    pub source: bs::Source,
    pub config: bs::PlaybackConfig,
    pub image_options: bs::ImageOptions,
    pub image_path: Option<PathBuf>,
    pub native_feature: Option<bs::NativeFeature>,
    pub applying_native: bool,
    pub converting_image: bool,
    pub clear_confirmation: bool,
    pub title: String,
    pub description: String,
    pub origin: String,
    pub frame: Vec<u8>,
    pub preview_running: bool,
    pub playing: bool,
    pub applied_title: String,
    pub applied_status: String,
    pub applied_frame: Vec<u8>,
    pub applied_elapsed_ms: u64,
    pub applied_to_phone: bool,
    pub companion_status: String,
    pub companion_installed: Option<bool>,
    pub companion_busy: bool,
    pub companion_mode: String,
    pub companion_permission: bool,
    revision: u64,
    applied_revision: Option<u64>,
    pending_play: bool,
    companion_checked_device: Option<String>,
    companion_rx: Option<mpsc::Receiver<CompanionEvent>>,
    pub elapsed_ms: u64,
    pub frames_sent: u64,
    pub status: String,
    pub error: Option<String>,
    pub advanced: bool,
    pub amoo_image: bool,
    pub generating: bool,
    pub generator_status: String,
    pub recent_images: Vec<PathBuf>,
    pub scroll: ClippedScrollStateHandle,
    started: Instant,
    playback: Option<bs::Playback>,
    generation: Option<ba::Job>,
    active_device: Option<String>,
    native_rx: Option<mpsc::Receiver<std::result::Result<String, String>>>,
    image_rx: Option<mpsc::Receiver<std::result::Result<(PathBuf, String, Vec<u8>), String>>>,
}

impl Default for BackScreenUiState {
    fn default() -> Self {
        let source = bs::Source::Preset(bs::Preset::Astronaut);
        let config = bs::PlaybackConfig::default();
        let frame = bs::frame(&source, &config, Duration::ZERO, &bs::LiveData::default());
        #[allow(unused_mut)] // Capture builds can select a local image during startup.
        let mut state = Self {
            tab: std::env::args()
                .position(|arg| arg == "--backscreen-tab")
                .and_then(|index| std::env::args().nth(index + 1))
                .map(|tab| match tab.as_str() {
                    "amoo" => 1,
                    "images" => 2,
                    "android" => 3,
                    "games" => 4,
                    _ => 0,
                })
                .unwrap_or(0),
            source,
            config,
            image_options: bs::ImageOptions::default(),
            image_path: None,
            native_feature: None,
            applying_native: false,
            converting_image: false,
            clear_confirmation: false,
            title: bs::Preset::Astronaut.label().into(),
            description: bs::Preset::Astronaut.description().into(),
            origin: "内置动画".into(),
            frame,
            preview_running: true,
            playing: false,
            applied_title: String::new(),
            applied_status: "尚未应用图案".into(),
            applied_frame: vec![0; bs::SLOTS],
            applied_elapsed_ms: 0,
            applied_to_phone: false,
            companion_status: "连接手机后检测 StepX伴侣".into(),
            companion_installed: None,
            companion_busy: false,
            companion_mode: String::new(),
            companion_permission: false,
            revision: 0,
            applied_revision: None,
            pending_play: false,
            companion_checked_device: None,
            companion_rx: None,
            elapsed_ms: 0,
            frames_sent: 0,
            status: "点击图案预览，再应用到手机背屏".into(),
            error: None,
            advanced: false,
            amoo_image: false,
            generating: false,
            generator_status: String::new(),
            recent_images: recent_images(),
            scroll: Default::default(),
            started: Instant::now(),
            playback: None,
            generation: None,
            active_device: None,
            native_rx: None,
            image_rx: None,
        };
        #[cfg(feature = "ui-capture")]
        if let Some(path) = std::env::args()
            .position(|arg| arg == "--backscreen-image")
            .and_then(|index| std::env::args().nth(index + 1))
        {
            state.tab = 2;
            let _ = state.apply_image(PathBuf::from(path), "Amoo 生成图片");
        }
        state
    }
}

impl Drop for BackScreenUiState {
    fn drop(&mut self) {
        if let Some(playback) = &self.playback {
            playback.cancel();
        }
        if let Some(generation) = &self.generation {
            generation.cancel();
        }
    }
}

impl BackScreenUiState {
    pub fn has_unapplied_changes(&self) -> bool {
        self.applied_revision != Some(self.revision)
            || !self.applied_to_phone
            || self.title != self.applied_title
    }
    pub fn temporary_app(&self) -> bool {
        matches!(self.source, bs::Source::App { .. })
    }
    pub fn applying(&self) -> bool {
        self.companion_busy || self.pending_play || self.applying_native
    }
    pub fn selection(&self) -> Option<bs::Preset> {
        match self.source {
            bs::Source::Preset(preset) | bs::Source::App { preset, .. } => Some(preset),
            _ => None,
        }
    }
    fn reset_preview(&mut self) {
        self.started = Instant::now();
        self.elapsed_ms = 0;
        self.preview_running = true;
        self.refresh_frame();
    }
    fn refresh_frame(&mut self) {
        self.frame = bs::frame(
            &self.source,
            &self.config,
            Duration::from_millis(self.elapsed_ms),
            &bs::LiveData::default(),
        );
    }
    fn reconvert(&mut self) -> Result<()> {
        if let Some(path) = self.image_path.clone() {
            let origin = self.origin.clone();
            let description = self.description.clone();
            self.apply_image(path, &origin)?;
            self.description = description;
        }
        Ok(())
    }
    pub fn apply_image(&mut self, path: PathBuf, origin: &str) -> Result<()> {
        anyhow::ensure!(path.is_file(), "图片文件已移动或删除，请重新选择");
        if self.image_path.as_ref() != Some(&path) {
            self.image_options.fit = if origin.starts_with("Amoo") {
                bs::ImageFit::Cover
            } else {
                bs::ImageFit::Contain
            };
        }
        let options = self.image_options.clone();
        let origin = origin.to_string();
        let (sender, receiver) = mpsc::channel();
        self.image_rx = Some(receiver);
        self.converting_image = true;
        self.description = "图片正转为 17 × 23 灰度点阵，可调节裁切、反色和抖动后播放。".into();
        self.error = None;
        self.status = "正在转换图片，完成后更新点阵预览…".into();
        std::thread::spawn(move || {
            let result = bs::image_frame(&path, &options)
                .map(|frame| (path, origin, frame))
                .map_err(|error| format!("{error:#}"));
            let _ = sender.send(result);
        });
        Ok(())
    }
}

impl Companion {
    pub(super) fn handle_backscreen_action(
        &mut self,
        action: &BackScreenAction,
        ctx: &mut ViewContext<Self>,
    ) {
        if let Err(error) = self.do_backscreen_action(action, ctx) {
            self.back_screen.error = Some(format!("{error:#}"));
            self.back_screen.status = format!("{error:#}");
            self.status = format!("{error:#}");
            self.error = true;
        }
    }
    fn do_backscreen_action(
        &mut self,
        action: &BackScreenAction,
        ctx: &mut ViewContext<Self>,
    ) -> Result<()> {
        if self.back_screen.converting_image
            && !matches!(
                action,
                BackScreenAction::Tab(_) | BackScreenAction::Advanced
            )
        {
            anyhow::bail!("正在转换图片，请稍候再修改");
        }
        if self.back_screen.applying_native {
            anyhow::bail!("正在应用手机联动，请等待完成");
        }
        if self.back_screen.companion_busy
            && !matches!(
                action,
                BackScreenAction::Tab(_) | BackScreenAction::Advanced
            )
        {
            anyhow::bail!("正在与 StepX伴侣同步，请稍候");
        }
        if self.back_screen.pending_play
            && !matches!(
                action,
                BackScreenAction::Stop | BackScreenAction::Tab(_) | BackScreenAction::Advanced
            )
        {
            anyhow::bail!("正在切换背屏图案，请等待应用完成");
        }
        if self.back_screen.generating
            && !matches!(
                action,
                BackScreenAction::CancelGenerate
                    | BackScreenAction::Tab(_)
                    | BackScreenAction::Advanced
            )
        {
            anyhow::bail!("Amoo 正在创作，请等待结果或取消后修改设计");
        }
        self.back_screen.error = None;
        if matches!(
            action,
            BackScreenAction::DisplayMode(_)
                | BackScreenAction::SaveBrightness
                | BackScreenAction::SaveMarquee
                | BackScreenAction::CameraOff
                | BackScreenAction::ClearFrame
        ) {
            anyhow::ensure!(
                !self.back_screen.playing
                    && !(self.back_screen.applied_to_phone
                        && self.back_screen.companion_mode != "native"),
                "请先停止当前图案，再保存手机默认设置"
            );
        }
        if matches!(
            action,
            BackScreenAction::SelectPreset(_)
                | BackScreenAction::SelectNative(_)
                | BackScreenAction::SelectApp(_, _)
                | BackScreenAction::Brightness(_)
                | BackScreenAction::Fps(_)
                | BackScreenAction::Duration(_)
                | BackScreenAction::Invert
        ) {
            self.back_screen.revision = self.back_screen.revision.wrapping_add(1);
        }
        match action {
            BackScreenAction::Tab(tab) => {
                self.back_screen.tab = *tab;
                self.back_screen.scroll = Default::default();
            }
            BackScreenAction::SelectPreset(preset) => {
                self.back_screen.native_feature = None;
                self.back_screen.source = bs::Source::Preset(*preset);
                self.back_screen.image_path = None;
                self.back_screen.title = preset.label().into();
                self.back_screen.description = preset.description().into();
                self.back_screen.origin = "内置动画".into();
                self.back_screen.status = "已选择，点击「应用到背屏」后更换手机显示".into();
                self.back_screen.reset_preview();
            }
            BackScreenAction::SelectNative(feature) => {
                self.back_screen.native_feature = Some(*feature);
                self.back_screen.image_path = None;
                self.back_screen.title = feature.label().into();
                self.back_screen.origin = "Android 原生联动".into();
                self.back_screen.description = native_description(*feature).into();
                self.back_screen.source = bs::Source::Preset(match feature {
                    bs::NativeFeature::Clock => bs::Preset::Clock,
                    bs::NativeFeature::Camera => bs::Preset::Countdown,
                    bs::NativeFeature::Notifications => bs::Preset::Heart,
                });
                self.back_screen.status = "已选择，应用后由 Android 系统管理显示".into();
                self.back_screen.reset_preview();
            }
            BackScreenAction::SelectApp(package, label) => {
                self.back_screen.native_feature = None;
                let preset = self.back_screen.selection().unwrap_or(bs::Preset::Meteor);
                self.back_screen.source = bs::Source::App {
                    package: package.clone(),
                    preset,
                };
                self.back_screen.image_path = None;
                self.back_screen.title = format!("{} · {}", label, preset.label());
                self.back_screen.origin = "Android 应用联动".into();
                self.back_screen.description = format!(
                    "在手机打开 {} 时显示{}；离开应用后恢复原显示。保持桌面连接。",
                    label,
                    preset.label()
                );
                self.back_screen.status = "应用联动待播放，桌面不会自动打开手机应用".into();
                self.back_screen.reset_preview();
            }
            BackScreenAction::RefreshGallery => {
                self.back_screen.recent_images = recent_images();
            }
            BackScreenAction::RecentImage(index) => {
                let path = self
                    .back_screen
                    .recent_images
                    .get(*index)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("这张作品已不在图库中，请刷新图库"))?;
                self.back_screen.apply_image(path, "Amoo 作品图库")?;
                self.back_screen.tab = 2;
            }
            BackScreenAction::DisplayMode(mode) => {
                let mode = mode.clone();
                self.start_backscreen_setting(move |adb, device| {
                    bs::set_display_mode(adb, device, &mode)
                })?;
            }
            BackScreenAction::SaveBrightness => {
                let brightness = self.back_screen.config.brightness;
                self.start_backscreen_setting(move |adb, device| {
                    bs::set_brightness(adb, device, brightness)
                })?;
            }
            BackScreenAction::SaveMarquee => {
                let text = self.fields[F_BACK_MARQUEE].text.clone();
                let scroll = self.matrix_marquee_on;
                let invert = self.back_screen.config.invert;
                self.start_backscreen_setting(move |adb, device| {
                    bs::marquee(adb, device, &text, scroll, invert)
                })?;
            }
            BackScreenAction::ToggleMarquee => self.matrix_marquee_on = !self.matrix_marquee_on,
            BackScreenAction::CameraOff => self
                .start_backscreen_setting(|adb, device| bs::set_camera_link(adb, device, false))?,
            BackScreenAction::PrepareClear => self.back_screen.clear_confirmation = true,
            BackScreenAction::CancelClear => self.back_screen.clear_confirmation = false,
            BackScreenAction::ClearFrame => {
                anyhow::ensure!(
                    self.back_screen.clear_confirmation,
                    "请先确认清除旧背屏图案"
                );
                self.back_screen.clear_confirmation = false;
                self.start_backscreen_setting(|adb, device| {
                    device.verify(adb)?;
                    bs::clear_frame(adb, device)?;
                    let state = adb.shell(&device.transport, &["dumpsys", "display_ext"])?;
                    anyhow::ensure!(
                        state.contains("appUsesCustomFrame=false"),
                        "背屏仍被其他应用占用，请先关闭该应用的背屏功能"
                    );
                    Ok("已清除旧自定义图案，恢复手机系统显示；现在可以播放新设计".into())
                })?;
            }
            BackScreenAction::Brightness(level) => {
                self.back_screen.config.brightness = (*level).clamp(1, 8);
                self.back_screen.refresh_frame();
            }
            BackScreenAction::Fps(fps) => {
                self.back_screen.config.fps = *fps;
                self.back_screen.refresh_frame();
            }
            BackScreenAction::Duration(seconds) => {
                self.back_screen.config.duration_secs = *seconds;
                self.back_screen.reset_preview();
            }
            BackScreenAction::Invert => {
                self.back_screen.config.invert = !self.back_screen.config.invert;
                self.back_screen.refresh_frame();
            }
            BackScreenAction::TogglePreview => {
                self.back_screen.preview_running = !self.back_screen.preview_running;
                if self.back_screen.preview_running {
                    self.back_screen.started =
                        Instant::now() - Duration::from_millis(self.back_screen.elapsed_ms);
                }
            }
            BackScreenAction::Play => {
                self.begin_backscreen_apply()?;
            }
            BackScreenAction::Stop => {
                self.back_screen.pending_play = false;
                if let Some(playback) = &self.back_screen.playback {
                    playback.cancel();
                }
                self.back_screen.status = "正在停止，并恢复手机原来的背屏设置…".into();
            }
            BackScreenAction::PickImage => {
                if let Some(path) = native_dialog::FileDialog::new()
                    .set_title("选择图片，预览背屏点阵")
                    .add_filter("图片", &["png", "jpg", "jpeg", "webp", "gif"])
                    .show_open_single_file()?
                {
                    self.back_screen.apply_image(path, "本地图片")?;
                    self.back_screen.tab = 2;
                }
            }
            BackScreenAction::Dither => {
                self.back_screen.image_options.dither = !self.back_screen.image_options.dither;
                self.back_screen.reconvert()?;
            }
            BackScreenAction::Threshold => {
                self.back_screen.image_options.threshold =
                    if self.back_screen.image_options.threshold.is_some() {
                        None
                    } else {
                        Some(128)
                    };
                self.back_screen.reconvert()?;
            }
            BackScreenAction::Fit => {
                self.back_screen.image_options.fit = match self.back_screen.image_options.fit {
                    bs::ImageFit::Contain => bs::ImageFit::Cover,
                    bs::ImageFit::Cover => bs::ImageFit::Contain,
                };
                self.back_screen.reconvert()?;
            }
            BackScreenAction::AmooMode(image) => self.back_screen.amoo_image = *image,
            BackScreenAction::Generate => {
                anyhow::ensure!(!self.back_screen.generating, "Amoo 正在生成，请等待或取消");
                let prompt = self.fields[F_BACK_PROMPT].text.trim().to_string();
                anyhow::ensure!(!prompt.is_empty(), "先描述想要的画面、功能、亮度或播放时长");
                let device = self
                    .selected
                    .clone()
                    .ok_or_else(|| anyhow::anyhow!("请先连接手机 Amoo"))?;
                self.back_screen.active_device = Some(device.serial.clone());
                self.back_screen.generation = Some(ba::start(
                    Adb::discover()?,
                    device,
                    prompt,
                    if self.back_screen.amoo_image {
                        ba::Mode::Image
                    } else {
                        ba::Mode::Auto
                    },
                ));
                self.back_screen.generating = true;
                self.back_screen.generator_status = "正在联系手机 Amoo…".into();
                self.back_screen.tab = 1;
            }
            BackScreenAction::CancelGenerate => {
                if let Some(job) = &self.back_screen.generation {
                    job.cancel();
                }
                self.back_screen.generator_status = "正在取消…".into();
            }
            BackScreenAction::Prompt(prompt) => {
                self.fields[F_BACK_PROMPT].text = prompt.clone();
                self.back_screen.tab = 1;
                self.focus_field(F_BACK_PROMPT, ctx);
            }
            BackScreenAction::Advanced => {
                self.back_screen.advanced = !self.back_screen.advanced;
                self.back_screen.scroll = Default::default();
            }
            BackScreenAction::CompanionInspect
            | BackScreenAction::CompanionInstall
            | BackScreenAction::GameArm
            | BackScreenAction::GameStart
            | BackScreenAction::CompanionStop => self.start_companion_action(action.clone())?,
        }
        Ok(())
    }
    fn begin_backscreen_apply(&mut self) -> Result<()> {
        anyhow::ensure!(!self.back_screen.generating, "请等待 Amoo 生成完成");
        let device = self
            .selected
            .clone()
            .ok_or_else(|| anyhow::anyhow!("请先连接手机"))?;
        if let Some(playback) = &self.back_screen.playback {
            self.back_screen.pending_play = true;
            playback.cancel();
            self.back_screen.applied_status = "正在结束原动画，随后应用所选图案…".into();
            return Ok(());
        }
        let adb = Adb::discover()?;
        let source = self.back_screen.source.clone();
        let config = self.back_screen.config.clone();
        let native = self.back_screen.native_feature;
        let title = self.back_screen.title.clone();
        let revision = self.back_screen.revision;
        let companion_running =
            self.back_screen.applied_to_phone && self.back_screen.companion_mode != "native";
        let (sender, receiver) = mpsc::channel();
        self.back_screen.active_device = Some(device.serial.clone());
        self.back_screen.companion_rx = Some(receiver);
        self.back_screen.companion_busy = true;
        self.back_screen.applied_status = "正在应用到手机背屏…".into();
        std::thread::spawn(move || {
            let result = (|| -> Result<CompanionEvent> {
                if let Some(feature) = native {
                    if companion_running {
                        ca::stop(&adb, &device)?;
                    }
                    let message = bs::configure_native(&adb, &device, feature)?;
                    return Ok(CompanionEvent::Applied {
                        title,
                        revision,
                        message,
                        mode: "native".into(),
                    });
                }
                if matches!(source, bs::Source::App { .. }) {
                    if companion_running {
                        ca::stop(&adb, &device)?;
                    }
                    let playback = bs::start(adb, device, source, config)?;
                    return Ok(CompanionEvent::TemporaryStarted {
                        playback,
                        title,
                        revision,
                    });
                }
                let message = if let bs::Source::Preset(
                    preset @ (bs::Preset::Clock | bs::Preset::Battery | bs::Preset::Countdown),
                ) = &source
                {
                    ca::apply_preset(
                        &adb,
                        &device,
                        preset.key(),
                        config.brightness,
                        config.duration_secs,
                    )?
                } else {
                    let count = match &source {
                        bs::Source::Image(_) => 1,
                        bs::Source::Preset(bs::Preset::Astronaut) => u32::from(config.fps) * 4,
                        _ => u32::from(config.fps) * 8,
                    };
                    let mut render = config.clone();
                    render.brightness = 8;
                    let frames: Vec<_> = (0..count)
                        .map(|index| {
                            bs::frame(
                                &source,
                                &render,
                                Duration::from_secs_f64(f64::from(index) / f64::from(config.fps)),
                                &Default::default(),
                            )
                        })
                        .collect();
                    ca::apply_animation(
                        &adb,
                        &device,
                        &frames,
                        config.fps,
                        config.brightness,
                        &title,
                    )?
                };
                Ok(CompanionEvent::Applied {
                    title,
                    revision,
                    message,
                    mode: "animation".into(),
                })
            })();
            let _ = sender
                .send(result.unwrap_or_else(|error| CompanionEvent::Failed(format!("{error:#}"))));
        });
        Ok(())
    }
    fn start_companion_action(&mut self, action: BackScreenAction) -> Result<()> {
        let device = self
            .selected
            .clone()
            .ok_or_else(|| anyhow::anyhow!("请先连接手机"))?;
        anyhow::ensure!(
            !self.back_screen.playing,
            "请先停止临时联动，再切换手机游戏"
        );
        let adb = Adb::discover()?;
        let (sender, receiver) = mpsc::channel();
        self.back_screen.companion_checked_device = Some(device.serial.clone());
        self.back_screen.active_device = Some(device.serial.clone());
        self.back_screen.companion_rx = Some(receiver);
        self.back_screen.companion_busy = true;
        self.back_screen.companion_status = match action {
            BackScreenAction::CompanionInstall => "正在安装 StepX伴侣，请保持 USB 连接…",
            BackScreenAction::GameArm => "正在启用三敲启动…",
            BackScreenAction::GameStart => "正在启动重力对齐…",
            BackScreenAction::CompanionStop => "正在停止并恢复原显示…",
            _ => "正在检测 StepX伴侣…",
        }
        .into();
        std::thread::spawn(move || {
            let result = (|| -> Result<CompanionEvent> {
                Ok(match action {
                    BackScreenAction::CompanionInstall => {
                        CompanionEvent::Inspected(ca::install(&adb, &device, |progress| {
                            let _ = sender.send(CompanionEvent::Progress(progress.into()));
                        })?)
                    }
                    BackScreenAction::GameArm => CompanionEvent::Game {
                        message: ca::arm_game(&adb, &device)?,
                        mode: "standby".into(),
                    },
                    BackScreenAction::GameStart => CompanionEvent::Game {
                        message: ca::start_game(&adb, &device)?,
                        mode: "game".into(),
                    },
                    BackScreenAction::CompanionStop => {
                        CompanionEvent::Stopped(ca::stop(&adb, &device)?)
                    }
                    _ => CompanionEvent::Inspected(ca::inspect(&adb, &device)?),
                })
            })();
            let _ = sender
                .send(result.unwrap_or_else(|error| CompanionEvent::Failed(format!("{error:#}"))));
        });
        Ok(())
    }
    fn start_backscreen_setting<F>(&mut self, operation: F) -> Result<()>
    where
        F: FnOnce(&Adb, &Device) -> Result<String> + Send + 'static,
    {
        let device = self
            .selected
            .clone()
            .ok_or_else(|| anyhow::anyhow!("请先连接手机"))?;
        let adb = Adb::discover()?;
        let (sender, receiver) = mpsc::channel();
        self.back_screen.active_device = Some(device.serial.clone());
        self.back_screen.native_rx = Some(receiver);
        self.back_screen.applying_native = true;
        self.back_screen.status = "正在保存手机背屏设置…".into();
        std::thread::spawn(move || {
            let _ = sender.send(operation(&adb, &device).map_err(|error| format!("{error:#}")));
        });
        Ok(())
    }
    pub(super) fn cancel_backscreen(&mut self) {
        if let Some(playback) = self.back_screen.playback.take() {
            playback.cancel();
        }
        if let Some(generation) = self.back_screen.generation.take() {
            generation.cancel();
        }
        self.back_screen.playing = false;
        self.back_screen.generating = false;
        self.back_screen.active_device = None;
        self.back_screen.native_rx = None;
        self.back_screen.applying_native = false;
        self.back_screen.companion_rx = None;
        self.back_screen.companion_busy = false;
        self.back_screen.companion_checked_device = None;
        self.back_screen.companion_installed = None;
        self.back_screen.applied_to_phone = false;
        self.back_screen.applied_revision = None;
        self.back_screen.applied_title.clear();
        self.back_screen.applied_status = "等待连接手机后读取状态".into();
        self.back_screen.pending_play = false;
        self.back_screen.image_rx = None;
        self.back_screen.converting_image = false;
        self.back_screen.generator_status = "设备连接已切换，原创作已取消".into();
        self.back_screen.status = "设备连接已切换；如已断开，停止后的硬件恢复结果无法读取".into();
    }
    pub(super) fn poll_backscreen(&mut self) -> bool {
        if self
            .back_screen
            .active_device
            .as_ref()
            .is_some_and(|serial| {
                self.selected.as_ref().map(|device| &device.serial) != Some(serial)
            })
        {
            self.cancel_backscreen();
            return true;
        }
        let mut changed = false;
        if self.selected.is_none() && self.back_screen.companion_checked_device.is_some() {
            self.back_screen.companion_checked_device = None;
            self.back_screen.companion_installed = None;
            self.back_screen.applied_to_phone = false;
            self.back_screen.companion_status = "连接手机后检测 StepX伴侣".into();
            self.back_screen.applied_status = "手机连接已断开，运行状态待检测".into();
            changed = true;
        }
        if self.page == Page::BackScreen
            && !self.back_screen.companion_busy
            && !self.back_screen.playing
            && !self.back_screen.generating
            && !self.back_screen.applying_native
            && self.selected.as_ref().is_some_and(|device| {
                self.back_screen.companion_checked_device.as_ref() != Some(&device.serial)
            })
        {
            if self.back_screen.companion_checked_device.is_some() {
                self.back_screen.companion_mode.clear();
                self.back_screen.applied_title.clear();
                self.back_screen.applied_to_phone = false;
                self.back_screen.applied_revision = None;
            }
            if let Err(error) = self.start_companion_action(BackScreenAction::CompanionInspect) {
                self.back_screen.companion_status = format!("{error:#}");
            }
            changed = true;
        }
        let mut companion_events = Vec::new();
        if let Some(receiver) = &self.back_screen.companion_rx {
            loop {
                match receiver.try_recv() {
                    Ok(event) => companion_events.push(event),
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        if companion_events.is_empty()
                            || companion_events
                                .iter()
                                .all(|event| matches!(event, CompanionEvent::Progress(_)))
                        {
                            companion_events.push(CompanionEvent::Failed(
                                "StepX伴侣操作未返回结果，请重新检测".into(),
                            ));
                        }
                        break;
                    }
                }
            }
        }
        for event in companion_events {
            changed = true;
            if let CompanionEvent::Progress(message) = event {
                self.back_screen.companion_status = message;
                continue;
            }
            self.back_screen.companion_busy = false;
            self.back_screen.companion_rx = None;
            self.back_screen.active_device = None;
            match event {
                CompanionEvent::Inspected(status) => {
                    self.back_screen.companion_installed = Some(status.installed);
                    self.back_screen.companion_permission = status.permission_granted;
                    let phone_message = status.message.clone();
                    self.back_screen.companion_status = if status.version.is_empty() {
                        status.message
                    } else {
                        format!("{} · {}", status.version, status.message)
                    };
                    if status.running || self.back_screen.companion_mode != "native" {
                        self.back_screen.companion_mode = status.mode;
                        self.back_screen.applied_to_phone = status.running;
                        self.back_screen.applied_title = status.applied_title;
                        self.back_screen.applied_status = phone_message;
                    }
                }
                CompanionEvent::Applied {
                    title,
                    revision,
                    message,
                    mode,
                } => {
                    self.back_screen.applied_title = title;
                    self.back_screen.applied_revision = Some(revision);
                    self.back_screen.applied_to_phone = true;
                    self.back_screen.applied_status = message;
                    self.back_screen.companion_mode = mode;
                    self.back_screen.status = "已应用，可以继续预览其他图案".into();
                }
                CompanionEvent::TemporaryStarted {
                    playback,
                    title,
                    revision,
                } => {
                    self.back_screen.playback = Some(playback);
                    self.back_screen.playing = true;
                    self.back_screen.applied_title = title;
                    self.back_screen.applied_revision = Some(revision);
                    self.back_screen.applied_elapsed_ms = 0;
                    self.back_screen.frames_sent = 0;
                    self.back_screen.companion_mode = "temporary".into();
                    self.back_screen.active_device =
                        self.selected.as_ref().map(|device| device.serial.clone());
                }
                CompanionEvent::Game { message, mode } => {
                    self.back_screen.companion_status = message.clone();
                    self.back_screen.applied_status = message;
                    self.back_screen.applied_to_phone = true;
                    self.back_screen.applied_revision = None;
                    self.back_screen.applied_title = if mode == "standby" {
                        "重力对齐 · 三敲待命"
                    } else {
                        "重力对齐"
                    }
                    .into();
                    self.back_screen.companion_mode = mode;
                }
                CompanionEvent::Stopped(message) => {
                    self.back_screen.applied_to_phone = false;
                    self.back_screen.applied_revision = None;
                    self.back_screen.applied_title.clear();
                    self.back_screen.applied_status = message.clone();
                    self.back_screen.companion_status = message;
                    self.back_screen.companion_mode.clear();
                }
                CompanionEvent::Failed(message) => {
                    self.back_screen.error = Some(message.clone());
                    self.back_screen.status = message.clone();
                    self.back_screen.companion_status = message;
                }
                CompanionEvent::Progress(_) => {}
            }
        }
        if let Some(receiver) = &self.back_screen.image_rx {
            let result = match receiver.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err("图片转换未返回结果，请重试".into()))
                }
                Err(mpsc::TryRecvError::Empty) => None,
            };
            if let Some(result) = result {
                self.back_screen.converting_image = false;
                self.back_screen.image_rx = None;
                changed = true;
                match result {
                    Ok((path, origin, frame)) => {
                        self.back_screen.title = path
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or("图片点阵".into());
                        self.back_screen.image_path = Some(path);
                        self.back_screen.origin = origin;
                        self.back_screen.source = bs::Source::Image(frame);
                        self.back_screen.native_feature = None;
                        self.back_screen.revision = self.back_screen.revision.wrapping_add(1);
                        self.back_screen.status = "图片已转换，检查预览后应用到背屏".into();
                        self.back_screen.reset_preview();
                    }
                    Err(error) => {
                        self.back_screen.status = error.clone();
                        self.back_screen.error = Some(error);
                    }
                }
            }
        }
        if let Some(receiver) = &self.back_screen.native_rx {
            let result = match receiver.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Disconnected) => Some(Err("手机联动配置未返回结果".into())),
                Err(mpsc::TryRecvError::Empty) => None,
            };
            if let Some(result) = result {
                changed = true;
                self.back_screen.native_rx = None;
                self.back_screen.active_device = None;
                self.back_screen.applying_native = false;
                match result {
                    Ok(message) => self.back_screen.status = message,
                    Err(error) => {
                        self.back_screen.status = error.clone();
                        self.back_screen.error = Some(error);
                    }
                }
            }
        }
        let mut playback_events = Vec::new();
        if let Some(playback) = &self.back_screen.playback {
            loop {
                match playback.events.try_recv() {
                    Ok(event) => playback_events.push(event),
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        if !playback_events.iter().any(|event| {
                            matches!(
                                event,
                                bs::PlaybackEvent::Finished { .. } | bs::PlaybackEvent::Failed(_)
                            )
                        }) {
                            playback_events.push(bs::PlaybackEvent::Failed(
                                "背屏播放连接已结束，无法确认手机恢复结果".into(),
                            ));
                        }
                        break;
                    }
                }
            }
        }
        for event in playback_events {
            changed = true;
            match event {
                bs::PlaybackEvent::Started { message } => {
                    self.back_screen.applied_status = message;
                }
                bs::PlaybackEvent::Frame { frame, elapsed_ms } => {
                    self.back_screen.applied_frame = frame;
                    self.back_screen.applied_elapsed_ms = elapsed_ms;
                    self.back_screen.applied_to_phone = true;
                    self.back_screen.frames_sent += 1;
                }
                bs::PlaybackEvent::Finished { cancelled, frames } => {
                    self.back_screen.playing = false;
                    self.back_screen.playback = None;
                    self.back_screen.frames_sent = frames;
                    self.back_screen.active_device = None;
                    self.back_screen.applied_to_phone = false;
                    self.back_screen.applied_revision = None;
                    self.back_screen.applied_status = format!(
                        "{} · 已发送 {} 帧，已恢复原有设置",
                        if cancelled {
                            "已停止"
                        } else {
                            "播放完成"
                        },
                        frames
                    );
                    if self.back_screen.pending_play {
                        self.back_screen.pending_play = false;
                        if let Err(error) = self.begin_backscreen_apply() {
                            self.back_screen.error = Some(format!("{error:#}"));
                        }
                    }
                }
                bs::PlaybackEvent::Failed(error) => {
                    self.back_screen.playing = false;
                    self.back_screen.playback = None;
                    self.back_screen.active_device = None;
                    self.back_screen.error = Some(error.clone());
                    self.back_screen.status = error;
                    self.back_screen.pending_play = false;
                    self.back_screen.applied_to_phone = false;
                    self.back_screen.applied_revision = None;
                }
            }
        }
        let mut generation_events = Vec::new();
        if let Some(generation) = &self.back_screen.generation {
            loop {
                match generation.receiver.try_recv() {
                    Ok(event) => generation_events.push(event),
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        if !generation_events.iter().any(|event| {
                            matches!(
                                event,
                                ba::Event::Complete(_)
                                    | ba::Event::Cancelled
                                    | ba::Event::Failed(_)
                            )
                        }) {
                            generation_events
                                .push(ba::Event::Failed("Amoo 创作连接已结束，请重试".into()));
                        }
                        break;
                    }
                }
            }
        }
        for event in generation_events {
            changed = true;
            match event {
                ba::Event::Status(status) => self.back_screen.generator_status = status,
                ba::Event::Complete(generated) => {
                    self.back_screen.revision = self.back_screen.revision.wrapping_add(1);
                    self.back_screen.generating = false;
                    self.back_screen.generation = None;
                    self.back_screen.active_device = None;
                    self.back_screen.config.brightness = generated.brightness;
                    self.back_screen.config.fps = generated.fps;
                    self.back_screen.config.duration_secs = generated.duration_secs;
                    self.back_screen.native_feature = None;
                    self.back_screen.recent_images = recent_images();
                    let result = if let Some(path) = generated.image_path {
                        self.back_screen.apply_image(path, "Amoo 生成图片")
                    } else if let Some(feature) = generated
                        .native_feature
                        .as_deref()
                        .and_then(native_from_key)
                    {
                        self.back_screen.native_feature = Some(feature);
                        self.back_screen.image_path = None;
                        self.back_screen.title = feature.label().into();
                        self.back_screen.origin = "Amoo 编排 · Android 联动".into();
                        self.back_screen.source = bs::Source::Preset(match feature {
                            bs::NativeFeature::Clock => bs::Preset::Clock,
                            bs::NativeFeature::Camera => bs::Preset::Countdown,
                            bs::NativeFeature::Notifications => bs::Preset::Heart,
                        });
                        self.back_screen.reset_preview();
                        Ok(())
                    } else if let Some(preset) =
                        generated.preset.as_deref().and_then(bs::Preset::from_key)
                    {
                        self.back_screen.source = if let Some(package) = &generated.app_package {
                            bs::Source::App {
                                package: package.clone(),
                                preset,
                            }
                        } else {
                            bs::Source::Preset(preset)
                        };
                        self.back_screen.image_path = None;
                        self.back_screen.title = preset.label().into();
                        self.back_screen.origin = "Amoo 编排".into();
                        self.back_screen.reset_preview();
                        Ok(())
                    } else {
                        Err(anyhow::anyhow!("Amoo 未返回可播放的图案或图片"))
                    };
                    match result {
                        Ok(()) => {
                            self.back_screen.description = generated.summary.clone();
                            self.back_screen.generator_status = generated.summary;
                            if !self.back_screen.converting_image {
                                self.back_screen.status =
                                    "Amoo 方案已生成，检查预览后应用到手机".into();
                            }
                        }
                        Err(error) => {
                            self.back_screen.error = Some(format!("{error:#}"));
                            self.back_screen.generator_status = format!("{error:#}");
                            self.back_screen.status = "方案转换失败，未发送到背屏".into();
                        }
                    }
                }
                ba::Event::Failed(error) => {
                    self.back_screen.generating = false;
                    self.back_screen.generation = None;
                    self.back_screen.active_device = None;
                    self.back_screen.error = Some(error.clone());
                    self.back_screen.generator_status = error;
                }
                ba::Event::Cancelled => {
                    self.back_screen.generating = false;
                    self.back_screen.generation = None;
                    self.back_screen.active_device = None;
                    self.back_screen.generator_status = "已取消生成".into();
                }
            }
        }
        if self.page == Page::BackScreen && self.back_screen.preview_running {
            let duration = if self.back_screen.tab == 4 {
                20_000
            } else {
                u64::from(self.back_screen.config.duration_secs.max(1)) * 1000
            };
            self.back_screen.elapsed_ms =
                self.back_screen.started.elapsed().as_millis() as u64 % duration;
            self.back_screen.refresh_frame();
            changed = true;
        }
        changed
    }
}

fn native_from_key(key: &str) -> Option<bs::NativeFeature> {
    match key {
        "notifications" => Some(bs::NativeFeature::Notifications),
        "clock" => Some(bs::NativeFeature::Clock),
        "camera" => Some(bs::NativeFeature::Camera),
        _ => None,
    }
}
pub(super) fn native_description(feature: bs::NativeFeature) -> &'static str {
    match feature {
        bs::NativeFeature::Notifications => {
            "启用手机通知背屏显示，实际效果由 Android 通知到达时触发。"
        }
        bs::NativeFeature::Clock => "由手机系统显示时钟，断开桌面后仍由手机管理。",
        bs::NativeFeature::Camera => "启用系统相机背屏联动，拍摄提示由手机相机与固件触发。",
    }
}

fn recent_images() -> Vec<PathBuf> {
    let Some(base) = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| crate::resources::user_home().map(|home| PathBuf::from(home).join(".local/share")))
    else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(base.join("stepx-companion/backscreen/generated")) else {
        return Vec::new();
    };
    let mut images: Vec<_> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            let metadata = entry.metadata().ok()?;
            if !metadata.is_file() || metadata.len() > 32 * 1024 * 1024 {
                return None;
            }
            let extension = path.extension()?.to_str()?.to_ascii_lowercase();
            if !["png", "jpg", "jpeg", "webp", "gif"].contains(&extension.as_str()) {
                return None;
            }
            Some((metadata.modified().unwrap_or(std::time::UNIX_EPOCH), path))
        })
        .collect();
    images.sort_by(|a, b| b.0.cmp(&a.0));
    images.into_iter().take(24).map(|(_, path)| path).collect()
}
