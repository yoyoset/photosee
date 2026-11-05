//! Login sessions (token -> user name) and the wrong-PIN lockout. Sessions
//! are written to `sessions.json` so a redeploy does not log the family
//! out: the user chose "logged in until Switch user".

use photosee_core::{Error, ErrorCode};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Wrong PINs in a row before a name is locked, and for how long. Four
/// digits are 10 000 guesses: at 5 per minute that is about 33 hours.
pub const MAX_FAILS: u32 = 5;
pub const LOCKOUT: Duration = Duration::from_secs(60);

struct Fails {
    count: u32,
    until: Option<Instant>,
}

pub struct Sessions {
    tokens: Mutex<HashMap<String, String>>,
    fails: Mutex<HashMap<String, Fails>>,
    file: PathBuf,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    // A poisoned map is still a valid map: the panicking writer either
    // inserted or did not.
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Sessions {
    /// `exists` drops sessions of users removed from users.json.
    pub fn new(file: PathBuf, exists: impl Fn(&str) -> bool) -> Self {
        // Unreadable or corrupt: everyone logs in again, nothing worse.
        let mut tokens: HashMap<String, String> = std::fs::read(&file)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        tokens.retain(|_, name| exists(name));
        Self {
            tokens: Mutex::new(tokens),
            fails: Mutex::new(HashMap::new()),
            file,
        }
    }

    pub fn user_of(&self, token: &str) -> Option<String> {
        lock(&self.tokens).get(token).cloned()
    }

    /// Checks `pin` against `real` (None: no such user) and opens a session.
    /// `now` is injected for tests.
    pub fn login(
        &self,
        name: &str,
        pin: &str,
        real: Option<&str>,
        now: Instant,
        token: String,
    ) -> Result<String, Error> {
        let wrong = || Error::new(ErrorCode::Unauthorized, "wrong name or PIN");
        let real = real.ok_or_else(wrong)?;
        let mut fails = lock(&self.fails);
        let f = fails.entry(name.to_owned()).or_insert(Fails {
            count: 0,
            until: None,
        });
        if let Some(t) = f.until.filter(|t| now < *t) {
            let secs = (t - now).as_secs() + 1;
            return Err(Error::new(
                ErrorCode::Unauthorized,
                format!("too many wrong PINs; try again in {secs} s"),
            ));
        }
        if !crate::guard::ct_eq(pin, real) {
            f.count += 1;
            if f.count >= MAX_FAILS {
                f.until = Some(now + LOCKOUT);
            }
            return Err(wrong());
        }
        fails.remove(name);
        drop(fails);
        self.open(name, token)
    }

    /// A session without a PIN check: first-run setup, right after the
    /// admin chose the PIN.
    pub fn open(&self, name: &str, token: String) -> Result<String, Error> {
        lock(&self.tokens).insert(token.clone(), name.to_owned());
        self.save();
        Ok(token)
    }

    pub fn logout(&self, token: &str) {
        if lock(&self.tokens).remove(token).is_some() {
            self.save();
        }
    }

    /// A deleted user, or a reset PIN: every device of theirs logs in again.
    pub fn drop_user(&self, name: &str) {
        lock(&self.tokens).retain(|_, n| n != name);
        self.save();
    }

    fn save(&self) {
        let bytes = serde_json::to_vec(&*lock(&self.tokens)).unwrap_or_default();
        // Losing this write only means a login is forgotten at the next
        // restart; the session itself works from memory.
        if let Err(e) = photosee_core::config::write_atomic(&self.file, &bytes) {
            eprintln!("photosee-server: cannot save sessions: {e}");
        }
    }
}
