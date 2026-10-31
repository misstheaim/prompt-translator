//! Tauri commands of the `target` module: session targets (T38).

use std::time::Duration;

use tauri::{AppHandle, State};

use tauri_plugin_clipboard_manager::ClipboardExt;

use super::sessions::{self, Bindings};
use super::{DeliveryOutcome, SessionTarget, deliver};
use crate::error::{AppError, AppResult};
use crate::history::{self, AcceptedTarget};
use crate::translation::providers::claude_code::TranslatorPids;

/// Runs blocking Win32 / UI Automation work off the async runtime's threads.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| AppError::Io(format!("The session lookup failed: {e}")))?
}

/// The terminal windows (Windows Terminal tabs, classic consoles) a Translate tab can be bound
/// to, front to back. The app's own translator runs are never listed.
#[tauri::command]
pub async fn list_session_targets(
    translators: State<'_, TranslatorPids>,
) -> AppResult<Vec<SessionTarget>> {
    let translators = translators.inner().clone();
    blocking(move || {
        Ok(sessions::list(&translators)
            .into_iter()
            .map(SessionTarget::Terminal)
            .collect())
    })
    .await
}

/// Waits up to `timeoutMs` (at most 60 s) for the user to bring a terminal window to the front,
/// binds it (with its selected Windows Terminal tab) and returns it. Rejects with `Timeout` when
/// no terminal came to the front in time. No separate `bind_target` call is needed.
#[tauri::command]
pub async fn bind_terminal_by_focus(
    app: AppHandle,
    translators: State<'_, TranslatorPids>,
    bindings: State<'_, Bindings>,
    timeout_ms: u64,
) -> AppResult<SessionTarget> {
    // Fail at once instead of after the wait when no binding is left.
    bindings.ensure_room()?;
    let translators = translators.inner().clone();
    let target = blocking(move || {
        sessions::wait_for_focused_terminal(&translators, Duration::from_millis(timeout_ms))
    })
    .await?;
    bindings.bind(&app, target.clone())?;
    Ok(SessionTarget::Terminal(target))
}

/// Binds a target from `list_session_targets`: checks that it still exists (`SessionClosed`
/// otherwise) and returns it with its current title. At most 5 bindings (`InvalidInput`). Each
/// bind needs one `unbind_target`; while any binding exists, closed ones are reported as
/// `target://closed` and forgotten.
#[tauri::command]
pub async fn bind_target(
    app: AppHandle,
    translators: State<'_, TranslatorPids>,
    bindings: State<'_, Bindings>,
    target: SessionTarget,
) -> AppResult<SessionTarget> {
    let SessionTarget::Terminal(target) = target;
    let translators = translators.inner().clone();
    let target = blocking(move || sessions::check(&translators, &target)).await?;
    bindings.bind(&app, target.clone())?;
    Ok(SessionTarget::Terminal(target))
}

/// Releases one binding (when a bound Translate tab closes). Unknown targets are ignored.
#[tauri::command]
pub fn unbind_target(bindings: State<'_, Bindings>, target: SessionTarget) {
    let SessionTarget::Terminal(target) = target;
    bindings.unbind(&target);
}

/// Accept in a bound Translate tab: puts `text` into the target terminal's input without
/// sending it (the user presses Enter in Claude), and marks the history entry accepted with the
/// session as target.
///
/// - Text starting with `!`, `/` or `#` (after trimming) is refused with
///   `PrefixNeedsConfirmation` unless `confirmed` is true (D35).
/// - Call it right after the user's own action in the app, so Windows lets the app bring the
///   terminal to the front.
/// - When the terminal can't take the text (closed, focus refused, keys in use, several lines
///   for a classic console...), the text is copied to the clipboard instead and the outcome
///   is `deliveredToClipboard` with the reason; the history entry is still marked accepted with the session as target.
#[tauri::command]
pub async fn deliver_to_target(
    app: AppHandle,
    translators: State<'_, TranslatorPids>,
    target: SessionTarget,
    text: String,
    history_id: Option<i64>,
    confirmed: Option<bool>,
) -> AppResult<DeliveryOutcome> {
    if text.trim().is_empty() {
        return Err(AppError::InvalidInput("There is nothing to send".into()));
    }
    if deliver::needs_confirmation(&text) && confirmed != Some(true) {
        return Err(AppError::PrefixNeedsConfirmation);
    }
    let SessionTarget::Terminal(target) = target;
    let translators = translators.inner().clone();
    let pasted = {
        let (target, text) = (target.clone(), text.clone());
        blocking(move || Ok(deliver::paste(&translators, &target, &text))).await?
    };
    match pasted {
        Ok(()) => {
            if let Some(id) = history_id {
                // A history problem never fails the delivery.
                let session = AcceptedTarget::Session {
                    id: format!("terminal:{:#x}", target.hwnd),
                    name: target.title,
                };
                history::commands::mark_accepted(&app, id, session).await;
            }
            Ok(DeliveryOutcome::Delivered)
        }
        Err(reason) => {
            log::info!("Delivery to a terminal failed ({reason}); copying instead");
            app.clipboard()
                .write_text(text)
                .map_err(|e| AppError::Io(format!("Can't write to the clipboard: {e}")))?;
            if let Some(id) = history_id {
                let session = AcceptedTarget::Session {
                    id: format!("terminal:{:#x}", target.hwnd),
                    name: target.title,
                };
                history::commands::mark_accepted(&app, id, session).await;
            }
            Ok(DeliveryOutcome::DeliveredToClipboard {
                reason: format!("{reason}. The prompt was copied; paste it with Ctrl+V."),
            })
        }
    }
}
