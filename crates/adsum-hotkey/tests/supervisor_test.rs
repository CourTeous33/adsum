use adsum_hotkey::backend::Backend;
use adsum_hotkey::supervisor::Supervisor;
use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use std::sync::Arc;

/// Mock backend: yields N successful events, then errors forever.
struct ScriptedBackend {
    successes_remaining: Arc<Mutex<u32>>,
    register_calls: Arc<Mutex<Vec<String>>>,
}

impl Backend for ScriptedBackend {
    fn register(&mut self, key_spec: &str) -> Result<()> {
        self.register_calls.lock().push(key_spec.to_string());
        Ok(())
    }

    fn next_event(&mut self) -> Result<()> {
        let mut n = self.successes_remaining.lock();
        if *n > 0 {
            *n -= 1;
            Ok(())
        } else {
            Err(anyhow!("backend died"))
        }
    }
}

#[test]
fn supervisor_restarts_once_then_exits() {
    let register_calls = Arc::new(Mutex::new(Vec::new()));
    let successes = Arc::new(Mutex::new(0u32)); // backend errors immediately

    let make_backend = {
        let register_calls = register_calls.clone();
        let successes = successes.clone();
        move || -> Box<dyn Backend> {
            Box::new(ScriptedBackend {
                successes_remaining: successes.clone(),
                register_calls: register_calls.clone(),
            })
        }
    };

    let fired = Arc::new(Mutex::new(0u32));
    let on_fire = {
        let fired = fired.clone();
        move || *fired.lock() += 1
    };

    let outcome = Supervisor::run("cmd+shift+space", make_backend, on_fire);

    // Two register calls: original + one restart. Then giving up.
    assert_eq!(register_calls.lock().len(), 2);
    assert!(matches!(outcome, adsum_hotkey::supervisor::Outcome::Exhausted));
    assert_eq!(*fired.lock(), 0);
}

#[test]
fn supervisor_passes_key_spec_to_register() {
    let register_calls = Arc::new(Mutex::new(Vec::new()));
    let successes = Arc::new(Mutex::new(0u32));

    let make_backend = {
        let register_calls = register_calls.clone();
        let successes = successes.clone();
        move || -> Box<dyn Backend> {
            Box::new(ScriptedBackend {
                successes_remaining: successes.clone(),
                register_calls: register_calls.clone(),
            })
        }
    };

    let _ = Supervisor::run("cmd+shift+space", make_backend, || {});

    let calls = register_calls.lock();
    assert!(calls.iter().all(|k| k == "cmd+shift+space"));
}

#[test]
fn supervisor_fires_callback_on_event() {
    let successes = Arc::new(Mutex::new(3u32)); // 3 events, then error
    let register_calls = Arc::new(Mutex::new(Vec::new()));

    let make_backend = {
        let register_calls = register_calls.clone();
        let successes = successes.clone();
        move || -> Box<dyn Backend> {
            Box::new(ScriptedBackend {
                successes_remaining: successes.clone(),
                register_calls: register_calls.clone(),
            })
        }
    };

    let fired = Arc::new(Mutex::new(0u32));
    let on_fire = {
        let fired = fired.clone();
        move || *fired.lock() += 1
    };

    let _ = Supervisor::run("cmd+shift+space", make_backend, on_fire);

    // 3 events fired before death; restart yields 0 more events; exhausts. Total = 3.
    assert_eq!(*fired.lock(), 3);
}

/// Mock backend whose `register` always errors.
struct UnregisterableBackend {
    register_attempts: Arc<Mutex<u32>>,
}

impl Backend for UnregisterableBackend {
    fn register(&mut self, _key_spec: &str) -> Result<()> {
        *self.register_attempts.lock() += 1;
        Err(anyhow!("registration always fails"))
    }

    fn next_event(&mut self) -> Result<()> {
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

    let outcome = Supervisor::run("cmd+shift+space", make_backend, || {});

    assert_eq!(*attempts.lock(), 2);
    assert!(matches!(outcome, adsum_hotkey::supervisor::Outcome::Exhausted));
}
