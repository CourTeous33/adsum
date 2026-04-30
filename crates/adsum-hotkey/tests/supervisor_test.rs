use adsum_hotkey::backend::Backend;
use adsum_hotkey::supervisor::Supervisor;
use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use std::sync::Arc;

/// Mock backend: yields a scripted sequence of (index, success?) tuples.
/// Once the script is exhausted, every subsequent `next_event` errors.
struct ScriptedBackend {
    /// Pop from the front each call to `next_event`. None → error.
    events: Arc<Mutex<Vec<usize>>>,
    register_all_calls: Arc<Mutex<Vec<Vec<String>>>>,
}

impl Backend for ScriptedBackend {
    fn register_all(&mut self, key_specs: &[&str]) -> Result<()> {
        self.register_all_calls
            .lock()
            .push(key_specs.iter().map(|s| s.to_string()).collect());
        Ok(())
    }

    fn next_event(&mut self) -> Result<usize> {
        let mut events = self.events.lock();
        if events.is_empty() {
            return Err(anyhow!("backend died"));
        }
        Ok(events.remove(0))
    }
}

#[test]
fn supervisor_restarts_once_then_exits() {
    let register_all_calls = Arc::new(Mutex::new(Vec::new()));
    let events = Arc::new(Mutex::new(Vec::<usize>::new())); // backend errors immediately

    let make_backend = {
        let register_all_calls = register_all_calls.clone();
        let events = events.clone();
        move || -> Box<dyn Backend> {
            Box::new(ScriptedBackend {
                events: events.clone(),
                register_all_calls: register_all_calls.clone(),
            })
        }
    };

    let fired = Arc::new(Mutex::new(Vec::<usize>::new()));
    let on_fire = {
        let fired = fired.clone();
        move |idx: usize| fired.lock().push(idx)
    };

    let outcome = Supervisor::run(&["cmd+shift+space"], make_backend, on_fire);

    // Two register calls: original + one restart. Then giving up.
    assert_eq!(register_all_calls.lock().len(), 2);
    assert!(matches!(outcome, adsum_hotkey::supervisor::Outcome::Exhausted));
    assert_eq!(fired.lock().len(), 0);
}

#[test]
fn supervisor_passes_key_specs_to_register_all() {
    let register_all_calls = Arc::new(Mutex::new(Vec::new()));
    let events = Arc::new(Mutex::new(Vec::<usize>::new()));

    let make_backend = {
        let register_all_calls = register_all_calls.clone();
        let events = events.clone();
        move || -> Box<dyn Backend> {
            Box::new(ScriptedBackend {
                events: events.clone(),
                register_all_calls: register_all_calls.clone(),
            })
        }
    };

    let _ = Supervisor::run(
        &["cmd+shift+space", "cmd+shift+d"],
        make_backend,
        |_| {},
    );

    let calls = register_all_calls.lock();
    assert!(calls
        .iter()
        .all(|specs| specs == &["cmd+shift+space".to_string(), "cmd+shift+d".to_string()]));
}

#[test]
fn supervisor_fires_callback_on_event_with_index() {
    // Script: events fire in order [chatbox, dashboard, chatbox], then error.
    let events = Arc::new(Mutex::new(vec![0usize, 1, 0]));
    let register_all_calls = Arc::new(Mutex::new(Vec::new()));

    let make_backend = {
        let register_all_calls = register_all_calls.clone();
        let events = events.clone();
        move || -> Box<dyn Backend> {
            Box::new(ScriptedBackend {
                events: events.clone(),
                register_all_calls: register_all_calls.clone(),
            })
        }
    };

    let fired = Arc::new(Mutex::new(Vec::<usize>::new()));
    let on_fire = {
        let fired = fired.clone();
        move |idx: usize| fired.lock().push(idx)
    };

    let _ = Supervisor::run(
        &["cmd+shift+space", "cmd+shift+d"],
        make_backend,
        on_fire,
    );

    // 3 events fired before death; restart drains 0 more events; exhausts.
    assert_eq!(*fired.lock(), vec![0, 1, 0]);
}

/// Mock backend whose `register_all` always errors.
struct UnregisterableBackend {
    register_attempts: Arc<Mutex<u32>>,
}

impl Backend for UnregisterableBackend {
    fn register_all(&mut self, _key_specs: &[&str]) -> Result<()> {
        *self.register_attempts.lock() += 1;
        Err(anyhow!("registration always fails"))
    }

    fn next_event(&mut self) -> Result<usize> {
        unreachable!("registration always fails before next_event")
    }
}

#[test]
fn supervisor_exhausts_when_registration_always_fails() {
    let attempts = Arc::new(Mutex::new(0u32));

    let make_backend = {
        let attempts = attempts.clone();
        move || -> Box<dyn Backend> {
            Box::new(UnregisterableBackend {
                register_attempts: attempts.clone(),
            })
        }
    };

    let outcome = Supervisor::run(&["cmd+shift+space"], make_backend, |_| {});

    assert_eq!(*attempts.lock(), 2);
    assert!(matches!(outcome, adsum_hotkey::supervisor::Outcome::Exhausted));
}
