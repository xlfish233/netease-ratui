use crate::app::App;
use crate::app::AppSnapshot;
use crate::audio_worker::AudioCommand;
use crate::error::MessageError;
use crate::messages::app::AppEvent;
use crate::messages::source::SourceCommand;
use crate::netease::actor::NeteaseCommand;
use tokio::sync::mpsc;

#[derive(Default)]
pub struct CoreEffects {
    pub(super) actions: Vec<CoreEffect>,
}

#[derive(Debug)]
pub enum CoreEffect {
    EmitState(Box<AppSnapshot>),
    EmitToast(String),
    EmitError(MessageError),
    SendSource {
        cmd: SourceCommand,
        warn: Option<&'static str>,
    },
    SendNeteaseHi {
        cmd: NeteaseCommand,
        warn: Option<&'static str>,
    },
    SendNeteaseLo {
        cmd: NeteaseCommand,
        warn: Option<&'static str>,
    },
    SendAudio {
        cmd: AudioCommand,
        warn: Option<&'static str>,
    },
}

impl CoreEffects {
    pub fn emit_state(&mut self, app: &App) {
        self.actions
            .push(CoreEffect::EmitState(Box::new(AppSnapshot::from_app(app))));
    }

    pub fn send_source_warn(&mut self, cmd: SourceCommand, warn: &'static str) {
        self.actions.push(CoreEffect::SendSource {
            cmd,
            warn: Some(warn),
        });
    }

    pub fn send_netease_hi(&mut self, cmd: NeteaseCommand) {
        self.actions
            .push(CoreEffect::SendNeteaseHi { cmd, warn: None });
    }

    pub fn send_netease_hi_warn(&mut self, cmd: NeteaseCommand, warn: &'static str) {
        self.actions.push(CoreEffect::SendNeteaseHi {
            cmd,
            warn: Some(warn),
        });
    }

    pub fn send_netease_lo(&mut self, cmd: NeteaseCommand) {
        self.actions
            .push(CoreEffect::SendNeteaseLo { cmd, warn: None });
    }

    #[allow(dead_code)]
    pub(super) fn send_netease_lo_warn(&mut self, cmd: NeteaseCommand, warn: &'static str) {
        self.actions.push(CoreEffect::SendNeteaseLo {
            cmd,
            warn: Some(warn),
        });
    }

    pub fn send_audio(&mut self, cmd: AudioCommand) {
        self.actions.push(CoreEffect::SendAudio { cmd, warn: None });
    }

    pub fn send_audio_warn(&mut self, cmd: AudioCommand, warn: &'static str) {
        self.actions.push(CoreEffect::SendAudio {
            cmd,
            warn: Some(warn),
        });
    }

    pub fn toast(&mut self, message: impl Into<String>) {
        self.actions.push(CoreEffect::EmitToast(message.into()));
    }

    pub fn error(&mut self, err: MessageError) {
        self.actions.push(CoreEffect::EmitError(err));
    }
}

pub struct CoreDispatch<'a> {
    pub(super) tx_source: &'a mpsc::Sender<SourceCommand>,
    pub(super) tx_netease_hi: &'a mpsc::Sender<NeteaseCommand>,
    pub(super) tx_netease_lo: &'a mpsc::Sender<NeteaseCommand>,
    pub(super) tx_audio: &'a mpsc::Sender<AudioCommand>,
    pub(super) tx_evt: &'a mpsc::Sender<AppEvent>,
}

pub async fn run_effects(effects: CoreEffects, dispatch: &CoreDispatch<'_>) {
    for effect in effects.actions {
        match effect {
            CoreEffect::EmitState(app) => {
                let _ = dispatch.tx_evt.send(AppEvent::State(app)).await;
            }
            CoreEffect::EmitToast(msg) => {
                let _ = dispatch.tx_evt.send(AppEvent::Toast(msg)).await;
            }
            CoreEffect::EmitError(err) => {
                let _ = dispatch.tx_evt.send(AppEvent::Error(err)).await;
            }
            CoreEffect::SendSource { cmd, warn } => {
                if let Err(e) = dispatch.tx_source.send(cmd).await
                    && let Some(ctx) = warn
                {
                    tracing::warn!(err = %e, "{ctx}");
                }
            }
            CoreEffect::SendNeteaseHi { cmd, warn } => {
                if let Err(e) = dispatch.tx_netease_hi.send(cmd).await
                    && let Some(ctx) = warn
                {
                    tracing::warn!(err = %e, "{ctx}");
                }
            }
            CoreEffect::SendNeteaseLo { cmd, warn } => {
                if let Err(e) = dispatch.tx_netease_lo.send(cmd).await
                    && let Some(ctx) = warn
                {
                    tracing::warn!(err = %e, "{ctx}");
                }
            }
            CoreEffect::SendAudio { cmd, warn } => {
                if let Err(e) = dispatch.tx_audio.send(cmd).await
                    && let Some(ctx) = warn
                {
                    tracing::warn!(err = %e, "{ctx}");
                }
            }
        }
    }
}
