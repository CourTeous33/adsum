//! Reusable blinking caret for text-input affordance.
//!
//! Two pieces:
//! - [`Caret`] — owns visibility + the spawned blink task. Drop the field to
//!   cancel the loop.
//! - [`spawn_blink`] — generic helper that spawns a 500ms toggle loop on the
//!   parent entity. Caller wires the accessor (which `Caret` to toggle) and
//!   the predicate (when to stop).
//!
//! Renders the `▌` block with a constant layout slot — the "invisible" phase
//! draws in `bg_primary` so the row width never shifts as the caret blinks.

use gpui::{div, prelude::*, AnyElement, Context, Task};
use std::time::Duration;

#[derive(Default)]
pub struct Caret {
    pub visible: bool,
    task: Option<Task<()>>,
}

impl Caret {
    pub fn new() -> Self {
        Self {
            visible: true,
            task: None,
        }
    }

    /// Render the `▌` element.
    pub fn render(&self) -> AnyElement {
        let color = if self.visible {
            adsum_tokens::accent()
        } else {
            adsum_tokens::bg_primary()
        };
        div().text_color(color).child("▌").into_any_element()
    }

    /// Stop blinking — drops the task, snaps `visible` back to true so the
    /// caret re-shows immediately on the next start.
    pub fn stop(&mut self) {
        self.task = None;
        self.visible = true;
    }

    /// Replace the active blink task. Caller produces the task via
    /// [`spawn_blink`].
    pub fn set_task(&mut self, task: Task<()>) {
        self.task = Some(task);
    }
}

/// Spawn a blink loop on the parent entity. The returned `Task<()>` should be
/// stored on a [`Caret`] (via [`Caret::set_task`]) so dropping the caret
/// cancels the loop.
///
/// `accessor` returns the `Caret` to toggle. `should_continue` is the predicate
/// checked each tick; the loop self-terminates when it returns `false`.
///
/// The 500ms interval matches typical text-input caret cadence.
pub fn spawn_blink<E, F, P>(cx: &mut Context<E>, accessor: F, should_continue: P) -> Task<()>
where
    E: 'static,
    F: Fn(&mut E) -> &mut Caret + Copy + 'static,
    P: Fn(&E) -> bool + Copy + 'static,
{
    cx.spawn(async move |this, cx| loop {
        cx.background_executor()
            .timer(Duration::from_millis(500))
            .await;
        let keep_going = this
            .update(cx, |entity, cx| {
                if !should_continue(entity) {
                    return false;
                }
                let caret = accessor(entity);
                caret.visible = !caret.visible;
                cx.notify();
                true
            })
            .unwrap_or(false);
        if !keep_going {
            break;
        }
    })
}
