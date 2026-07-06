use tauri::{AppHandle, Emitter, Manager};

use super::events::{HOTKEY_ERROR, HOTKEY_TRIGGERED};
use super::types::{RecognitionClickEffect, RecognitionClickMode, RecognitionHotkeyEffectStep};
use super::{player, resolve_audio_effect_path, RecognitionState, ResolvedPlay};
use crate::input_simulation;

#[derive(Debug, Clone)]
pub(crate) enum TriggerContext {
    Hotkey,
    Region {
        center_x: i32,
        center_y: i32,
    },
    Color {
        matched_probes: Vec<ColorProbeMatch>,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct ColorProbeMatch {
    pub index: usize,
    pub center_x: i32,
    pub center_y: i32,
}

struct EffectPlan {
    playback_tx: std::sync::mpsc::Sender<player::AudioCommand>,
    audio: Option<ResolvedPlay>,
    hotkey_steps: Vec<RecognitionHotkeyEffectStep>,
    click_point: Option<(i32, i32)>,
}

pub(crate) async fn execute(
    app: AppHandle,
    card_id: String,
    context: TriggerContext,
) -> Result<(), String> {
    let plan = build_plan(&app, &card_id, &context)?;

    if let Some(audio) = plan.audio {
        let _ = plan.playback_tx.send(player::AudioCommand::Play {
            path: audio.path,
            volume: audio.volume,
            exclusive: !audio.allow_simultaneous,
        });
    }

    for step in plan.hotkey_steps {
        if step.delay_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(step.delay_ms as u64)).await;
        }
        input_simulation::press_hotkey_once(&step.hotkey, "识别触发按键效果").await?;
    }

    if let Some((x, y)) = plan.click_point {
        input_simulation::click_points(&[(x, y, 0)]).await?;
    }

    Ok(())
}

pub(crate) fn spawn_execute(app: AppHandle, card_id: String, context: TriggerContext) {
    tauri::async_runtime::spawn(async move {
        if let Err(error) = execute(app.clone(), card_id.clone(), context).await {
            let _ = app.emit_to("main", HOTKEY_ERROR, error);
        } else {
            let _ = app.emit(HOTKEY_TRIGGERED, card_id);
        }
    });
}

fn build_plan(
    app: &AppHandle,
    card_id: &str,
    context: &TriggerContext,
) -> Result<EffectPlan, String> {
    let state = app.state::<RecognitionState>();
    let inner = state.lock_inner()?;
    if !inner.settings.recognition_enabled {
        return Err("识别触发功能未启用".to_string());
    }
    let card = inner
        .settings
        .cards
        .iter()
        .find(|c| c.id == card_id)
        .ok_or_else(|| "卡片不存在".to_string())?
        .clone();
    if !card.enabled {
        return Err("卡片未启用".to_string());
    }

    let audio = card
        .effects
        .audio
        .as_ref()
        .filter(|effect| !effect.audio_files.is_empty())
        .map(|effect| resolve_audio_effect_path(&inner, card_id, effect, std::time::Instant::now()))
        .transpose()?;
    let hotkey_steps = card
        .effects
        .hotkey
        .as_ref()
        .map(|effect| effect.normalized_steps())
        .unwrap_or_default();
    let click_point = card
        .effects
        .click
        .as_ref()
        .and_then(|effect| click_point_for_effect(effect, context));
    if audio.is_none() && hotkey_steps.is_empty() && click_point.is_none() {
        return Err("卡片没有可执行的触发效果".to_string());
    }

    Ok(EffectPlan {
        playback_tx: inner.logic.playback_tx.clone(),
        audio,
        hotkey_steps,
        click_point,
    })
}

fn click_point_for_effect(
    effect: &RecognitionClickEffect,
    context: &TriggerContext,
) -> Option<(i32, i32)> {
    match effect.mode {
        RecognitionClickMode::CustomRegion => effect
            .custom_region
            .as_ref()
            .map(|rect| (rect.x + rect.width / 2, rect.y + rect.height / 2)),
        RecognitionClickMode::RecognitionRegion => match context {
            TriggerContext::Region { center_x, center_y } => Some((*center_x, *center_y)),
            TriggerContext::Color { matched_probes } => {
                let wanted = effect.color_probe_index?;
                matched_probes
                    .iter()
                    .find(|probe| probe.index == wanted)
                    .map(|probe| (probe.center_x, probe.center_y))
            }
            TriggerContext::Hotkey => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::morse::types::RegionRect;
    use crate::recognition::types::{RecognitionClickEffect, RecognitionClickMode};

    #[test]
    fn custom_click_uses_region_center() {
        let effect = RecognitionClickEffect {
            mode: RecognitionClickMode::CustomRegion,
            custom_region: Some(RegionRect {
                x: 10,
                y: 20,
                width: 30,
                height: 40,
            }),
            color_probe_index: None,
        };
        assert_eq!(
            click_point_for_effect(&effect, &TriggerContext::Hotkey),
            Some((25, 40))
        );
    }

    #[test]
    fn color_click_only_uses_selected_matched_probe() {
        let effect = RecognitionClickEffect {
            mode: RecognitionClickMode::RecognitionRegion,
            custom_region: None,
            color_probe_index: Some(1),
        };
        let context = TriggerContext::Color {
            matched_probes: vec![ColorProbeMatch {
                index: 0,
                center_x: 10,
                center_y: 20,
            }],
        };
        assert_eq!(click_point_for_effect(&effect, &context), None);

        let context = TriggerContext::Color {
            matched_probes: vec![ColorProbeMatch {
                index: 1,
                center_x: 30,
                center_y: 40,
            }],
        };
        assert_eq!(click_point_for_effect(&effect, &context), Some((30, 40)));
    }

    #[test]
    fn missing_custom_click_region_has_no_click_point() {
        let effect = RecognitionClickEffect {
            mode: RecognitionClickMode::CustomRegion,
            custom_region: None,
            color_probe_index: None,
        };
        assert_eq!(
            click_point_for_effect(&effect, &TriggerContext::Hotkey),
            None
        );
    }
}
