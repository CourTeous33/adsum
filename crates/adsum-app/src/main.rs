use adsum_chatbox::Chatbox;
use adsum_conversation::Conversation;
use adsum_state::{AppState, SummonAction};
use gpui::{
    App, Bounds, Pixels, WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions,
    point, prelude::*, px, size,
};
use gpui_platform::application;
use std::sync::{Arc, Mutex};

fn show_hotkey_failure_notification() {
    let _ = std::process::Command::new("osascript")
        .args([
            "-e",
            "display notification \"Adsum couldn't register the global hotkey. Check Accessibility permissions in System Settings.\" with title \"Adsum\"",
        ])
        .status();
}

fn open_chatbox(
    state: Arc<Mutex<AppState>>,
    conversation_slot: Arc<Mutex<Option<gpui::WindowHandle<Conversation>>>>,
    cx: &mut App,
) -> gpui::WindowHandle<Chatbox> {
    let chatbox_size = size(px(720.0), px(80.0));
    let bounds = match cx.primary_display() {
        Some(display) => {
            let display_bounds = display.bounds();
            let origin = point(
                display_bounds.origin.x
                    + (display_bounds.size.width - chatbox_size.width) / 2.0,
                display_bounds.origin.y + display_bounds.size.height
                    - chatbox_size.height
                    - px(100.0),
            );
            Bounds::new(origin, chatbox_size)
        }
        None => Bounds::new(point(Pixels::ZERO, Pixels::ZERO), chatbox_size),
    };

    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            is_resizable: false,
            kind: WindowKind::PopUp,
            window_background: WindowBackgroundAppearance::Transparent,
            ..Default::default()
        },
        |window, cx| {
            let state = state.clone();
            let conv_slot = conversation_slot.clone();
            cx.new(|cx| Chatbox::new(state, conv_slot, window, cx))
        },
    )
    .unwrap()
}

fn run_example() {
    env_logger::init();

    let (summon_tx, summon_rx) = async_channel::unbounded::<()>();
    let (exhausted_tx, exhausted_rx) = async_channel::bounded::<()>(1);

    std::thread::spawn(move || {
        let outcome = adsum_hotkey::supervisor::Supervisor::run(
            "cmd+shift+space",
            || Box::new(adsum_hotkey::RealBackend::new()),
            || {
                let _ = summon_tx.send_blocking(());
            },
        );
        eprintln!("hotkey supervisor exited: {outcome:?}");
        let _ = exhausted_tx.send_blocking(());
    });

    application().run(move |cx: &mut App| {
        cx.activate(true);

        let state = Arc::new(Mutex::new(AppState::default()));
        let chatbox_slot: Arc<Mutex<Option<gpui::WindowHandle<Chatbox>>>> =
            Arc::new(Mutex::new(None));
        let conversation_slot: Arc<Mutex<Option<gpui::WindowHandle<Conversation>>>> =
            Arc::new(Mutex::new(None));

        // Register a single global on_window_closed handler. When the chatbox
        // closes by ANY means (Esc, blur, system close), clear the slot and
        // mark it not visible in AppState. Closing the chatbox cascades to
        // closing the conversation window (if any).
        //
        // When the conversation closes (either cascaded or by some other
        // means), just clear its slot — the chatbox is independent.
        let state_for_close = state.clone();
        let chatbox_slot_close = chatbox_slot.clone();
        let conversation_slot_close = conversation_slot.clone();
        cx.on_window_closed(move |cx, closed_window_id| {
            // Was it the chatbox?
            let is_chatbox = {
                let slot = chatbox_slot_close.lock().unwrap();
                slot.as_ref().is_some_and(|h| h.window_id() == closed_window_id)
            }; // slot guard dropped here.
            if is_chatbox {
                // No locks currently held.
                let session = state_for_close.lock().unwrap().take_session();
                if let Some(s) = session {
                    if !s.turns.is_empty() {
                        if let Err(err) = adsum_state::persistence::save_session(&s) {
                            eprintln!("adsum-app: failed to save session {}: {err:#}", s.id);
                        }
                    }
                }
                *chatbox_slot_close.lock().unwrap() = None;
                state_for_close.lock().unwrap().set_chatbox_visible(false);

                // Take the conversation handle in a standalone statement so the
                // MutexGuard drops before handle.update — remove_window() will
                // re-enter on_window_closed synchronously, which would
                // re-acquire conversation_slot_close.
                let conv_handle_opt = conversation_slot_close.lock().unwrap().take();
                if let Some(conv_handle) = conv_handle_opt {
                    let _ = conv_handle.update(cx, |_view, window, _cx| {
                        window.remove_window();
                    });
                }
                return;
            }

            // Was it the conversation? Just clear its slot — chatbox stays.
            let is_conversation = {
                let slot = conversation_slot_close.lock().unwrap();
                slot.as_ref().is_some_and(|h| h.window_id() == closed_window_id)
            };
            if is_conversation {
                *conversation_slot_close.lock().unwrap() = None;
            }
        })
        .detach();

        let exhausted_rx = exhausted_rx.clone();
        cx.spawn(async move |_async_cx| {
            if exhausted_rx.recv().await.is_ok() {
                show_hotkey_failure_notification();
                std::process::exit(1);
            }
        })
        .detach();

        let summon_rx = summon_rx.clone();
        let state_for_loop = state.clone();
        let slot_for_loop = chatbox_slot.clone();
        let conv_slot_for_loop = conversation_slot.clone();
        cx.spawn(async move |async_cx| {
            while let Ok(()) = summon_rx.recv().await {
                let action = state_for_loop.lock().unwrap().handle_chatbox_summon();
                let state = state_for_loop.clone();
                let slot = slot_for_loop.clone();
                let conv_slot = conv_slot_for_loop.clone();
                async_cx.update(move |cx: &mut App| match action {
                    SummonAction::Open => {
                        // Defensive: if a stale handle is in the slot (state
                        // says hidden but slot has a value), close the old
                        // window before opening a new one to avoid orphans.
                        let stale = slot.lock().unwrap().take();
                        if let Some(stale_handle) = stale {
                            let _ = stale_handle.update(cx, |_view, window, _cx| {
                                window.remove_window();
                            });
                        }
                        state.lock().unwrap().start_session();
                        let handle = open_chatbox(state.clone(), conv_slot.clone(), cx);
                        *slot.lock().unwrap() = Some(handle);
                        state.lock().unwrap().set_chatbox_visible(true);
                    }
                    SummonAction::Dismiss => {
                        // Take the handle in a standalone statement so the
                        // MutexGuard from `slot.lock()` is dropped at the `;`,
                        // before handle.update runs. remove_window() fires
                        // on_window_closed synchronously inside handle.update,
                        // and that callback re-locks `slot` — holding the
                        // guard across the call deadlocks std::sync::Mutex.
                        // The chatbox's on_window_closed handler also closes
                        // the conversation window, so we don't need to do it
                        // here.
                        let handle_opt = slot.lock().unwrap().take();
                        if let Some(handle) = handle_opt {
                            let _ = handle.update(cx, |_view, window, _cx| {
                                window.remove_window();
                            });
                        }
                        state.lock().unwrap().set_chatbox_visible(false);
                    }
                });
            }
        })
        .detach();
    });
}

fn main() {
    run_example();
}
