use super::*;
#[derive(Default)]
pub(super) struct SpeechUi {
    pub key: Option<String>,
    pub playback: Option<stepx_companion::speech::Playback>,
}
impl Companion {
    pub(super) fn speak(&mut self, key: &str, text: &str) {
        let stopping = self.speech.key.as_deref() == Some(key);
        self.speech.playback = None;
        self.speech.key = None;
        if stopping {
            self.status = "已停止朗读".into();
            return;
        }
        match stepx_companion::speech::Playback::start(text) {
            Ok(p) => {
                self.speech.key = Some(key.into());
                self.speech.playback = Some(p);
                self.status = "正在电脑上朗读 · 再次点击可停止".into();
                self.error = false;
            }
            Err(e) => {
                self.status = format!("无法朗读：{e:#}");
                self.error = true;
            }
        }
    }
    pub(super) fn tick_speech(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(p) = self.speech.playback.as_mut() else {
            return;
        };
        let result = p.poll();
        if matches!(result, Ok(false)) {
            return;
        }
        self.speech.playback = None;
        self.speech.key = None;
        match result {
            Ok(_) => self.status = "朗读完成".into(),
            Err(e) => {
                self.error = true;
                self.status = e.to_string();
            }
        }
        ctx.notify();
    }
    pub(super) fn speech_icon(&self, id: String, text: String) -> widgets::El {
        let active = self.speech.key.as_deref() == Some(&id);
        self.work_action_icon(
            id.clone(),
            if active {
                "停止电脑朗读"
            } else {
                "在电脑上朗读"
            },
            if active {
                icons::UiIcon::Close
            } else {
                icons::UiIcon::Speaker
            },
            Action::Speak(id, text),
        )
    }
}
