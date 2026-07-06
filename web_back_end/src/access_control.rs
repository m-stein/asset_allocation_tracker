use crate::error::WebBackEndError;
use axum::http::HeaderMap;
use core_lib::AccessGrant;
use rand::RngCore;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const ACCESS_TOKEN_HEADER: &str = "x-tallytail-access-token";
const ACCESS_STATE_FILE_NAME: &str = "access_control_state.ron";
const ACCESS_GRANT_TTL: Duration = Duration::from_secs(12 * 60 * 60);
const ACCESS_TOKEN_BYTE_LENGTH: usize = 32;
const MIN_UNLOCK_PATTERN_POINTS: usize = 6;
const MAX_FAILED_UNLOCK_ATTEMPTS: u8 = 3;
const UNLOCK_BLOCK_DURATION: Duration = Duration::from_secs(3 * 60 * 60);

#[derive(Clone)]
pub struct AccessControl {
    state: Arc<AccessControlState>,
}

struct AccessControlState {
    unlock_pattern: String,
    sessions: Mutex<HashMap<String, Instant>>,
    unlock_guards: Mutex<HashMap<String, UnlockGuard>>,
    state_file_path: PathBuf,
}

#[derive(Default)]
struct UnlockGuard {
    failed_attempts: u8,
    blocked_until: Option<SystemTime>,
}

#[derive(Default, serde::Deserialize, serde::Serialize)]
struct PersistedUnlockGuard {
    failed_attempts: u8,
    blocked_until_epoch_seconds: Option<u64>,
}

#[derive(Default, serde::Deserialize, serde::Serialize)]
struct PersistedAccessState {
    unlock_guards_by_client_id: HashMap<String, PersistedUnlockGuard>,
}

pub enum UnlockAttemptResult {
    Unlocked(AccessGrant),
    PatternNotAccepted,
    TooManyAttempts,
}

impl AccessControl {
    pub fn new(unlock_pattern: String, data_dir: PathBuf) -> eyre::Result<Self> {
        let unlock_pattern = normalize_pattern(&unlock_pattern)?;
        fs::create_dir_all(&data_dir)?;
        let access_state_path = data_dir.join(ACCESS_STATE_FILE_NAME);
        let unlock_guards = load_unlock_guards(&access_state_path)?;

        Ok(Self {
            state: Arc::new(AccessControlState {
                unlock_pattern,
                sessions: Mutex::new(HashMap::new()),
                unlock_guards: Mutex::new(unlock_guards),
                state_file_path: access_state_path,
            }),
        })
    }

    pub fn unlock(&self, headers: &HeaderMap, pattern: &str) -> eyre::Result<UnlockAttemptResult> {
        let client_id = determine_client_id(headers);
        let pattern_matches = normalize_pattern(pattern)
            .map(|pattern| pattern == self.state.unlock_pattern)
            .unwrap_or(false);

        {
            let mut unlock_guards = self
                .state
                .unlock_guards
                .lock()
                .map_err(|_| eyre::eyre!("Access unlock guards are poisoned"))?;
            *unlock_guards = load_unlock_guards(&self.state.state_file_path)?;

            let failed_result = {
                let unlock_guard = unlock_guards.entry(client_id).or_default();

                if unlock_guard.is_blocked() {
                    return Ok(UnlockAttemptResult::TooManyAttempts);
                }

                unlock_guard.clear_expired_block();

                if pattern_matches {
                    unlock_guard.reset();
                    None
                } else {
                    Some(unlock_guard.record_failed_attempt())
                }
            };

            if let Some(result) = failed_result {
                save_unlock_guards(&self.state.state_file_path, &unlock_guards)?;
                return Ok(result);
            } else {
                unlock_guards.retain(|_, guard| !guard.is_empty());
                save_unlock_guards(&self.state.state_file_path, &unlock_guards)?;
            }
        }

        let access_token = generate_access_token();
        let expires_at = Instant::now() + ACCESS_GRANT_TTL;
        self.state
            .sessions
            .lock()
            .map_err(|_| eyre::eyre!("Access session store is poisoned"))?
            .insert(access_token.clone(), expires_at);

        Ok(UnlockAttemptResult::Unlocked(AccessGrant {
            access_token,
            expires_in_seconds: ACCESS_GRANT_TTL.as_secs(),
        }))
    }

    fn has_access(&self, headers: &HeaderMap) -> eyre::Result<bool> {
        let Some(access_token) = headers
            .get(ACCESS_TOKEN_HEADER)
            .and_then(|header| header.to_str().ok())
        else {
            return Ok(false);
        };

        let now = Instant::now();
        let mut sessions = self
            .state
            .sessions
            .lock()
            .map_err(|_| eyre::eyre!("Access session store is poisoned"))?;

        sessions.retain(|_, expires_at| *expires_at > now);
        Ok(sessions
            .get(access_token)
            .is_some_and(|expires_at| *expires_at > now))
    }
}

fn load_unlock_guards(path: &Path) -> eyre::Result<HashMap<String, UnlockGuard>> {
    if !path.exists() {
        return Ok(HashMap::new());
    }

    let state = fs::read_to_string(path)?;
    let persisted: PersistedAccessState = ron::from_str(&state)?;
    Ok(persisted
        .unlock_guards_by_client_id
        .into_iter()
        .map(|(client_id, guard)| {
            let guard = UnlockGuard {
                failed_attempts: guard.failed_attempts,
                blocked_until: guard
                    .blocked_until_epoch_seconds
                    .map(|seconds| UNIX_EPOCH + Duration::from_secs(seconds)),
            };
            (client_id, guard)
        })
        .collect())
}

fn save_unlock_guards(
    path: &Path,
    unlock_guards: &HashMap<String, UnlockGuard>,
) -> eyre::Result<()> {
    let persisted = PersistedAccessState {
        unlock_guards_by_client_id: unlock_guards
            .iter()
            .map(|(client_id, guard)| {
                Ok((
                    client_id.clone(),
                    PersistedUnlockGuard {
                        failed_attempts: guard.failed_attempts,
                        blocked_until_epoch_seconds: guard
                            .blocked_until
                            .map(|blocked_until| blocked_until.duration_since(UNIX_EPOCH))
                            .transpose()?
                            .map(|duration| duration.as_secs()),
                    },
                ))
            })
            .collect::<eyre::Result<HashMap<_, _>>>()?,
    };
    fs::write(path, ron::to_string(&persisted)?)?;
    Ok(())
}

impl UnlockGuard {
    fn is_empty(&self) -> bool {
        self.failed_attempts == 0 && self.blocked_until.is_none()
    }

    fn is_blocked(&self) -> bool {
        self.blocked_until
            .is_some_and(|blocked_until| blocked_until > SystemTime::now())
    }

    fn clear_expired_block(&mut self) -> bool {
        if self
            .blocked_until
            .is_some_and(|blocked_until| blocked_until <= SystemTime::now())
        {
            self.blocked_until = None;
            true
        } else {
            false
        }
    }

    fn record_failed_attempt(&mut self) -> UnlockAttemptResult {
        self.failed_attempts = self.failed_attempts.saturating_add(1);
        if self.failed_attempts >= MAX_FAILED_UNLOCK_ATTEMPTS {
            self.failed_attempts = 0;
            self.blocked_until = Some(SystemTime::now() + UNLOCK_BLOCK_DURATION);
            UnlockAttemptResult::TooManyAttempts
        } else {
            UnlockAttemptResult::PatternNotAccepted
        }
    }

    fn reset(&mut self) {
        self.failed_attempts = 0;
        self.blocked_until = None;
    }
}

fn determine_client_id(headers: &HeaderMap) -> String {
    headers
        .get("fly-client-ip")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string())
        .or_else(|| {
            headers
                .get("x-forwarded-for")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.split(',').next())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "unknown-client".to_string())
}

fn normalize_pattern(pattern: &str) -> eyre::Result<String> {
    let digits: Vec<char> = pattern.chars().filter(|ch| ch.is_ascii_digit()).collect();
    if digits.len() < MIN_UNLOCK_PATTERN_POINTS {
        return Err(eyre::eyre!(
            "Unlock pattern must contain at least {MIN_UNLOCK_PATTERN_POINTS} points"
        ));
    }

    let mut seen = [false; 10];
    for digit in &digits {
        let value = digit
            .to_digit(10)
            .ok_or_else(|| eyre::eyre!("Unlock pattern contains an invalid point"))?
            as usize;
        if value == 0 || value > 9 {
            return Err(eyre::eyre!("Unlock pattern points must be between 1 and 9"));
        }
        if seen[value] {
            return Err(eyre::eyre!(
                "Unlock pattern must not contain repeated points"
            ));
        }
        seen[value] = true;
    }

    Ok(digits.into_iter().collect())
}

fn generate_access_token() -> String {
    let mut bytes = [0_u8; ACCESS_TOKEN_BYTE_LENGTH];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn ensure_has_access(
    access_control: &AccessControl,
    headers: &HeaderMap,
) -> Result<(), WebBackEndError> {
    match access_control.has_access(headers) {
        Ok(true) => Ok(()),
        Ok(false) => Err(WebBackEndError::unauthorized("Unlock required")),
        Err(err) => Err(WebBackEndError::from(err)),
    }
}
