use adsum_chatbox::Chatbox;
use adsum_conversation::Conversation;
use adsum_dashboard::Dashboard;
use adsum_state::{AppState, SummonAction};
use gpui::{
    point, prelude::*, px, size, App, Bounds, Pixels, TitlebarOptions, WindowBackgroundAppearance,
    WindowBounds, WindowKind, WindowOptions,
};
use gpui_platform::application;
use std::sync::{Arc, Mutex};

fn show_hotkey_failure_notification(hotkey: &str) {
    let body = format!(
        "Adsum couldn't register the global hotkey {hotkey}. Check Accessibility permissions in System Settings.",
    );
    let osa = format!("display notification \"{body}\" with title \"Adsum\"");
    let _ = std::process::Command::new("osascript")
        .args(["-e", &osa])
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
                display_bounds.origin.x + (display_bounds.size.width - chatbox_size.width) / 2.0,
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

fn open_dashboard(cx: &mut App) -> gpui::WindowHandle<Dashboard> {
    let dashboard_size = size(px(1024.0), px(720.0));
    let bounds = match cx.primary_display() {
        Some(display) => {
            let display_bounds = display.bounds();
            let origin = point(
                display_bounds.origin.x + (display_bounds.size.width - dashboard_size.width) / 2.0,
                display_bounds.origin.y
                    + (display_bounds.size.height - dashboard_size.height) / 2.0,
            );
            Bounds::new(origin, dashboard_size)
        }
        None => Bounds::new(point(Pixels::ZERO, Pixels::ZERO), dashboard_size),
    };

    // Bring the app to the front so the dashboard window is summoned ON TOP of
    // whatever the user is currently looking at, rather than being buried.
    cx.activate(true);

    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("Adsum".into()),
                ..Default::default()
            }),
            is_resizable: true,
            kind: WindowKind::Normal,
            ..Default::default()
        },
        |window, cx| {
            // Activate the window so it grabs platform-level focus immediately.
            // Without this, the dashboard can open behind the active app on
            // some macOS setups when summoned via the global hotkey.
            window.activate_window();
            cx.new(|cx| Dashboard::new(window, cx))
        },
    )
    .unwrap()
}

fn run_example() {
    env_logger::init();

    // Both hotkeys share a single supervisor thread (and a single underlying
    // GlobalHotKeyManager — macOS only allows one per process). The supervisor
    // dispatches by index; index 0 = chatbox, index 1 = dashboard.
    let (chatbox_summon_tx, chatbox_summon_rx) = async_channel::unbounded::<()>();
    let (dashboard_summon_tx, dashboard_summon_rx) = async_channel::unbounded::<()>();
    let (exhausted_tx, exhausted_rx) = async_channel::bounded::<()>(1);

    std::thread::spawn(move || {
        let outcome = adsum_hotkey::supervisor::Supervisor::run(
            &["cmd+shift+space", "cmd+shift+d"],
            || Box::new(adsum_hotkey::RealBackend::new()),
            move |idx| match idx {
                0 => {
                    let _ = chatbox_summon_tx.send_blocking(());
                }
                1 => {
                    let _ = dashboard_summon_tx.send_blocking(());
                }
                other => eprintln!("adsum-app: unexpected hotkey index {other}"),
            },
        );
        eprintln!("hotkey supervisor exited: {outcome:?}");
        let _ = exhausted_tx.send_blocking(());
    });

    application().run(move |cx: &mut App| {
        cx.activate(true);

        // Shared app state + three window slots.
        let state = Arc::new(Mutex::new(AppState::default()));
        let chatbox_slot: Arc<Mutex<Option<gpui::WindowHandle<Chatbox>>>> =
            Arc::new(Mutex::new(None));
        let conversation_slot: Arc<Mutex<Option<gpui::WindowHandle<Conversation>>>> =
            Arc::new(Mutex::new(None));
        let dashboard_slot: Arc<Mutex<Option<gpui::WindowHandle<Dashboard>>>> =
            Arc::new(Mutex::new(None));

        // Global on_window_closed handler. Three branches: chatbox close
        // cascades to conversation close + saves session; conversation close
        // just clears its slot; dashboard close just clears its slot and
        // marks it not visible in AppState.
        let state_for_close = state.clone();
        let chatbox_slot_close = chatbox_slot.clone();
        let conversation_slot_close = conversation_slot.clone();
        let dashboard_slot_close = dashboard_slot.clone();
        cx.on_window_closed(move |cx, closed_window_id| {
            // Was it the chatbox? Save session, clear slot, mark hidden,
            // cascade-close conversation.
            let is_chatbox = {
                let slot = chatbox_slot_close.lock().unwrap();
                slot.as_ref()
                    .is_some_and(|h| h.window_id() == closed_window_id)
            }; // slot guard dropped here.
            if is_chatbox {
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
                slot.as_ref()
                    .is_some_and(|h| h.window_id() == closed_window_id)
            };
            if is_conversation {
                *conversation_slot_close.lock().unwrap() = None;
                return;
            }

            // Was it the dashboard? Clear its slot and mark hidden in state.
            let is_dashboard = {
                let slot = dashboard_slot_close.lock().unwrap();
                slot.as_ref()
                    .is_some_and(|h| h.window_id() == closed_window_id)
            };
            if is_dashboard {
                *dashboard_slot_close.lock().unwrap() = None;
                state_for_close.lock().unwrap().set_dashboard_visible(false);
            }
        })
        .detach();

        // Single hotkey-failure pump. Both hotkeys share the supervisor;
        // failure to register either is fatal for both.
        let exhausted_rx = exhausted_rx.clone();
        cx.spawn(async move |_| {
            if exhausted_rx.recv().await.is_ok() {
                show_hotkey_failure_notification("cmd+shift+space or cmd+shift+d");
                std::process::exit(1);
            }
        })
        .detach();

        // Chatbox summon pump.
        let chatbox_summon_rx = chatbox_summon_rx.clone();
        let state_for_chatbox = state.clone();
        let chatbox_slot_for_loop = chatbox_slot.clone();
        let conv_slot_for_chatbox = conversation_slot.clone();
        cx.spawn(async move |async_cx| {
            while let Ok(()) = chatbox_summon_rx.recv().await {
                let action = state_for_chatbox.lock().unwrap().handle_chatbox_summon();
                let state = state_for_chatbox.clone();
                let slot = chatbox_slot_for_loop.clone();
                let conv_slot = conv_slot_for_chatbox.clone();
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
                        // Clone (not take) the handle so the slot stays
                        // populated when on_window_closed fires synchronously
                        // inside handle.update — that's how the cascade
                        // identifies the chatbox as the closed window and
                        // cleans up state, slot, AND closes the conversation.
                        // Cloning still releases the slot lock at the `;` so
                        // we don't deadlock when on_window_closed re-locks.
                        let handle_opt = *slot.lock().unwrap();
                        if let Some(handle) = handle_opt {
                            let _ = handle.update(cx, |_view, window, _cx| {
                                window.remove_window();
                            });
                        }
                        // No state/slot updates here — on_window_closed does
                        // them when the chatbox window actually closes.
                    }
                });
            }
        })
        .detach();

        // Dashboard summon pump.
        let dashboard_summon_rx = dashboard_summon_rx.clone();
        let state_for_dashboard = state.clone();
        let dashboard_slot_for_loop = dashboard_slot.clone();
        cx.spawn(async move |async_cx| {
            while let Ok(()) = dashboard_summon_rx.recv().await {
                let action = state_for_dashboard
                    .lock()
                    .unwrap()
                    .handle_dashboard_summon();
                let state = state_for_dashboard.clone();
                let slot = dashboard_slot_for_loop.clone();
                async_cx.update(move |cx: &mut App| match action {
                    SummonAction::Open => {
                        let stale = slot.lock().unwrap().take();
                        if let Some(stale_handle) = stale {
                            let _ = stale_handle.update(cx, |_view, window, _cx| {
                                window.remove_window();
                            });
                        }
                        let handle = open_dashboard(cx);
                        *slot.lock().unwrap() = Some(handle);
                        state.lock().unwrap().set_dashboard_visible(true);
                    }
                    SummonAction::Dismiss => {
                        // Clone (not take) for the same reason as the chatbox
                        // path: on_window_closed needs the slot populated to
                        // identify the closed window as the dashboard.
                        let handle_opt = *slot.lock().unwrap();
                        if let Some(handle) = handle_opt {
                            let _ = handle.update(cx, |_view, window, _cx| {
                                window.remove_window();
                            });
                        }
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
