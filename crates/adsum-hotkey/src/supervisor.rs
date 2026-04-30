use crate::backend::Backend;

#[derive(Debug)]
#[must_use = "ignoring Outcome::Exhausted hides hotkey supervisor failure"]
pub enum Outcome {
    /// Both attempts failed. Caller should notify the user and disable hotkeys.
    Exhausted,
}

pub struct Supervisor;

impl Supervisor {
    /// Run the backend in a supervised loop. On the first failure, restart once.
    /// On the second failure, return `Outcome::Exhausted`.
    ///
    /// `key_specs` is a list of hotkey specs registered against a single
    /// backend (single underlying GlobalHotKeyManager). `on_fire` is invoked
    /// synchronously each time any hotkey fires; the index argument indicates
    /// which spec in `key_specs` produced the event.
    ///
    /// `make_backend` is called each time we (re)start the worker — it returns
    /// a fresh backend instance (the prior one may have died).
    pub fn run<F, G>(
        key_specs: &[&str],
        mut make_backend: F,
        mut on_fire: G,
    ) -> Outcome
    where
        F: FnMut() -> Box<dyn Backend>,
        G: FnMut(usize),
    {
        for attempt in 0..2 {
            let mut backend = make_backend();
            if let Err(err) = backend.register_all(key_specs) {
                eprintln!(
                    "adsum-hotkey: registration attempt {} failed: {err:#}",
                    attempt + 1
                );
                continue;
            }

            // Drain events until backend errors out.
            loop {
                match backend.next_event() {
                    Ok(idx) => on_fire(idx),
                    Err(err) => {
                        eprintln!(
                            "adsum-hotkey: backend died on attempt {}: {err:#}",
                            attempt + 1
                        );
                        break;
                    }
                }
            }
        }

        Outcome::Exhausted
    }
}
