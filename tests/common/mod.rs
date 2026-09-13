//! Shared sandbox helper for integration tests, adapted from git-helper's
//! `tests/cli.rs` pattern: each test gets its own throwaway git repository
//! and an isolated git configuration that never touches the developer's
//! real global/system config.
//!
//! Library-level tests (`diff`/`patch`) call gatomic's functions in-process,
//! which shell out to `git` relying on the process's current directory and
//! environment. Since those are process-global, tests using [`Sandbox::enter`]
//! must run serialized — acquire [`SANDBOX_LOCK`] for the duration of the test.

use std::path::PathBuf;
use std::process::Output;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A panic inside one test while holding this lock must not poison it for
/// every other test in the suite — recover the guard instead of unwrapping.
pub fn sandbox_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub struct Sandbox {
    pub dir: PathBuf,
    global_config: PathBuf,
    previous_dir: PathBuf,
}

impl Sandbox {
    /// Creates a new sandbox repo and makes it the process's current
    /// directory, with git env vars pointed at an isolated config. Callers
    /// must hold [`sandbox_lock`] for as long as the returned guard lives.
    pub fn enter() -> Self {
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let base = std::env::temp_dir().join(format!("gatomic-test-{}-{id}", std::process::id()));
        let dir = base.join("repo");
        std::fs::create_dir_all(&dir).unwrap();
        let global_config = base.join("gitconfig");
        std::fs::write(&global_config, "").unwrap();

        let previous_dir = std::env::current_dir().unwrap();

        // SAFETY: callers hold `sandbox_lock` for the sandbox's lifetime, so
        // no other thread observes these process-wide env var changes.
        unsafe {
            std::env::set_var("GIT_CONFIG_GLOBAL", &global_config);
            std::env::set_var("GIT_CONFIG_SYSTEM", "/dev/null");
            std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
        }
        std::env::set_current_dir(&dir).unwrap();

        let sandbox = Sandbox {
            dir,
            global_config,
            previous_dir,
        };
        sandbox.git(&["init", "-q", "-b", "main"]);
        sandbox.git(&["config", "user.email", "test@example.com"]);
        sandbox.git(&["config", "user.name", "tester"]);
        sandbox
    }

    pub fn git(&self, args: &[&str]) -> Output {
        let out = std::process::Command::new("git")
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        out
    }

    pub fn write(&self, name: &str, contents: &str) {
        let path = self.dir.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    pub fn commit(&self, file: &str, contents: &str, message: &str) {
        self.write(file, contents);
        self.git(&["add", file]);
        self.git(&["commit", "-q", "-m", message]);
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.previous_dir);
        let _ = std::fs::remove_dir_all(self.dir.parent().unwrap());
        let _ = &self.global_config;
    }
}
