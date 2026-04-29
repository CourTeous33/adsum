use crate::backend::Backend;

#[derive(Debug)]
#[must_use = "ignoring Outcome::Exhausted hides hotkey supervisor failure"]
pub enum Outcome {
    /// Both attempts failed. Caller should notify the user and disable hotkey.
    Exhausted,
}

pub struct Supervisor;

impl Supervisor {
    /// Run the backend in a supervised loop. On the first failure, restart once.
    /// On the second failure, return `Outcome::Exhausted`.
    ///
    /// `make_backend` is called each time we (re)start the worker — it returns
    /// a fresh backend instance (the prior one may have died).
    /// `on_fire` is invoked synchronously each time the hotkey fires.
    pub fn run<F, G>(
        key_spec: &str,
        mut make_backend: F,
        mut on_fire: G,
    ) -> Outcome
    where
        F: FnMut() -> Box<dyn Backend>,
        G: FnMut(),
    {
        for attempt in 0..2 {
            let mut backend = make_backend();
            if let Err(err) = backend.register(key_spec) {
                eprintln!(
                    "adsum-hotkey: registration attempt {} failed: {err:#}",
                    attempt + 1
                );
                // Registration failed; treat as a death and try again, unless
                // this is already the second attempt.
                if attempt == 1 {
                    return Outcome::Exhausted;
                }
                continue;
            }

            // Drain events until backend errors out.
            loop {
                match backend.next_event() {
                    Ok(()) => on_fire(),
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
