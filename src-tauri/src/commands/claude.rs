//! Claude uses its own login and credentials. Only profile metadata is stored here.
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClaudeUsage {
    pub five_hour_used: f64,
    pub seven_day_used: f64,
    pub five_hour_resets_at: Option<String>,
    pub seven_day_resets_at: Option<String>,
    pub checked_at: i64,
    #[serde(default)]
    pub model_used: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClaudeProfile {
    pub id: String,
    pub email: String,
    pub is_default: bool,
    #[serde(default)]
    pub account_uuid: Option<String>,
    #[serde(default)]
    pub desktop_status: String,
    #[serde(default)]
    pub logged_in: bool,
    #[serde(default)]
    pub plan_type: Option<String>,
    #[serde(default)]
    pub last_warmup_at: Option<i64>,
    #[serde(default)]
    pub usage: Option<ClaudeUsage>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct ClaudeProfiles {
    pub profiles: Vec<ClaudeProfile>,
    pub active_code_id: Option<String>,
    pub active_desktop_id: Option<String>,
    #[serde(default)]
    pub auto_switch_enabled: bool,
    #[serde(default)]
    pub auto_warmup_enabled: bool,
    #[serde(default, skip_deserializing)]
    pub waiting_for_quota: bool,
}

#[derive(Clone, Default, Serialize)]
pub struct ClaudeLogin {
    pub flow_id: String,
    pub profile_id: String,
    pub auth_url: Option<String>,
    pub status: String,
    pub error: Option<String>,
}

static LOGIN: LazyLock<Mutex<Option<ClaudeLogin>>> = LazyLock::new(|| Mutex::new(None));
static LOGIN_INPUT: LazyLock<Mutex<Option<(String, tokio::sync::mpsc::Sender<String>)>>> =
    LazyLock::new(|| Mutex::new(None));
static WARMUP_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn root() -> Result<PathBuf> {
    crate::auth::get_config_dir().map(|p| p.join("claude"))
}
fn home() -> Result<PathBuf> {
    dirs::home_dir().context("Home directory is unavailable")
}

fn private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    private_dir(path.parent().context("Missing parent directory")?)?;
    let tmp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&tmp)?;
    file.write_all(&serde_json::to_vec_pretty(value)?)?;
    file.sync_all()?;
    fs::rename(&tmp, path)?;
    Ok(())
}

fn lock_store() -> Result<File> {
    private_dir(&root()?)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(root()?.join("profiles.lock"))?;
    file.lock()?;
    Ok(file)
}

fn load() -> Result<ClaudeProfiles> {
    let path = root()?.join("profiles.json");
    if !path.exists() {
        return Ok(ClaudeProfiles::default());
    }
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn save(store: &ClaudeProfiles) -> Result<()> {
    write_json(&root()?.join("profiles.json"), store)
}

fn profile<'a>(store: &'a ClaudeProfiles, id: &str) -> Result<&'a ClaudeProfile> {
    store
        .profiles
        .iter()
        .find(|p| p.id == id)
        .context("Claude account not found")
}

fn code_dir(p: &ClaudeProfile) -> Result<PathBuf> {
    uuid::Uuid::parse_str(&p.id).context("Invalid Claude profile ID")?;
    if p.is_default {
        Ok(home()?.join(".claude"))
    } else {
        Ok(root()?.join(&p.id).join("code"))
    }
}

fn claude_binary() -> Result<PathBuf> {
    let local = home()?.join(".local/bin/claude");
    if local.is_file() {
        return Ok(local);
    }
    for dir in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        let bin = dir.join(if cfg!(windows) {
            "claude.exe"
        } else {
            "claude"
        });
        if bin.is_file() {
            return Ok(bin);
        }
    }
    bail!("Install Claude Code before adding a Claude account")
}

fn code_command(p: &ClaudeProfile) -> Result<Command> {
    let mut cmd = Command::new(claude_binary()?);
    for key in [
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
        "ANTHROPIC_BASE_URL",
        "ANTHROPIC_PROFILE",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "CLAUDE_SECURESTORAGE_CONFIG_DIR",
        "CLAUDE_CODE_OAUTH_REFRESH_TOKEN",
        "CLAUDE_CODE_OAUTH_SCOPES",
        "CLAUDE_CODE_OAUTH_CLIENT_ID",
        "CLAUDE_CODE_USE_BEDROCK",
        "CLAUDE_CODE_USE_VERTEX",
        "CLAUDE_CODE_USE_FOUNDRY",
    ] {
        cmd.env_remove(key);
    }
    if p.is_default {
        cmd.env_remove("CLAUDE_CONFIG_DIR");
    } else {
        cmd.env("CLAUDE_CONFIG_DIR", code_dir(p)?);
    }
    Ok(cmd)
}

fn prepare_code_dir(p: &ClaudeProfile) -> Result<()> {
    let dir = code_dir(p)?;
    private_dir(&dir)?;
    if p.is_default {
        return Ok(());
    }
    #[cfg(unix)]
    {
        // Share local context, never the login or account-specific .claude.json.
        let base = home()?.join(".claude");
        for item in [
            "projects",
            "settings.json",
            "CLAUDE.md",
            "skills",
            "agents",
            "plugins",
        ] {
            let source = base.join(item);
            let target = dir.join(item);
            if source.exists() && fs::symlink_metadata(&target).is_err() {
                std::os::unix::fs::symlink(source, target)?;
            }
        }
    }
    Ok(())
}

fn login_status(p: &ClaudeProfile) -> Result<Value> {
    let mut child = code_command(p)?
        .args(["auth", "status"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let start = Instant::now();
    while child.try_wait()?.is_none() {
        if start.elapsed() > Duration::from_secs(10) {
            let _ = child.kill();
            let _ = child.wait();
            bail!("Claude login status timed out");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let output = child.wait_with_output()?;
    serde_json::from_slice(&output.stdout).context("Claude did not return a login status")
}

fn read_credentials(p: &ClaudeProfile) -> Result<Value> {
    #[cfg(target_os = "macos")]
    {
        let suffix = if p.is_default {
            String::new()
        } else {
            format!(
                "-{}",
                &format!(
                    "{:x}",
                    Sha256::digest(code_dir(p)?.to_string_lossy().as_bytes())
                )[..8]
            )
        };
        let output = Command::new("security")
            .args([
                "find-generic-password",
                "-s",
                &format!("Claude Code-credentials{suffix}"),
                "-w",
            ])
            .output()?;
        if output.status.success() {
            let credentials: Value = serde_json::from_slice(&output.stdout)?;
            if credentials
                .pointer("/claudeAiOauth/accessToken")
                .and_then(Value::as_str)
                .is_some()
            {
                return Ok(credentials);
            }
        }
    }
    let credentials: Value = serde_json::from_slice(
        &fs::read(code_dir(p)?.join(".credentials.json"))
            .context("Claude subscription login is unavailable; sign in again")?,
    )?;
    if credentials
        .pointer("/claudeAiOauth/accessToken")
        .and_then(Value::as_str)
        .is_none()
    {
        bail!("Claude subscription login is unavailable");
    }
    Ok(credentials)
}

fn parse_usage(value: &Value) -> Result<ClaudeUsage> {
    let percent = |key: &str| -> Result<f64> {
        value
            .pointer(&format!("/{key}/utilization"))
            .and_then(Value::as_f64)
            .filter(|v| v.is_finite() && (0.0..=100.0).contains(v))
            .context("Claude usage window is unavailable")
    };
    Ok(ClaudeUsage {
        five_hour_used: percent("five_hour")?,
        seven_day_used: percent("seven_day")?,
        five_hour_resets_at: value
            .pointer("/five_hour/resets_at")
            .and_then(Value::as_str)
            .map(str::to_owned),
        seven_day_resets_at: value
            .pointer("/seven_day/resets_at")
            .and_then(Value::as_str)
            .map(str::to_owned),
        checked_at: chrono::Utc::now().timestamp(),
        model_used: ["seven_day_opus", "seven_day_sonnet"]
            .into_iter()
            .filter(|key| value.get(*key).is_some_and(|v| !v.is_null()))
            .map(percent)
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .max_by(f64::total_cmp),
    })
}

fn login_needs_refresh(credentials: &Value, now: i64) -> bool {
    credentials
        .pointer("/claudeAiOauth/expiresAt")
        .and_then(Value::as_i64)
        .is_some_and(|expires| expires <= now + 60_000)
}

async fn native_access_token(p: &ClaudeProfile, force: bool) -> Result<String> {
    let copy = p.clone();
    let mut credentials = tokio::task::spawn_blocking(move || read_credentials(&copy)).await??;
    if force || login_needs_refresh(&credentials, chrono::Utc::now().timestamp_millis()) {
        let dir = root()?.join(&p.id);
        private_dir(&dir)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(dir.join("refresh.lock"))?;
        lock.try_lock()
            .context("This Claude login is already being refreshed")?;
        let copy = p.clone();
        credentials = tokio::task::spawn_blocking(move || read_credentials(&copy)).await??;
        let oauth = credentials
            .get("claudeAiOauth")
            .context("Claude subscription login unavailable")?;
        let token = oauth
            .get("refreshToken")
            .and_then(Value::as_str)
            .context("Sign in again to renew this Claude login")?;
        let scopes = oauth
            .get("scopes")
            .and_then(Value::as_array)
            .context("Claude login scopes unavailable; sign in again")?
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(" ");
        // Native auth login supports refresh-token login and owns all credential writes.
        // Secrets are passed via the child environment, never command arguments or logs.
        let mut command = tokio::process::Command::from(code_command(p)?);
        command
            .env("CLAUDE_CODE_OAUTH_REFRESH_TOKEN", token)
            .env("CLAUDE_CODE_OAUTH_SCOPES", scopes);
        if let Some(client) = oauth.get("clientId").and_then(Value::as_str) {
            command.env("CLAUDE_CODE_OAUTH_CLIENT_ID", client);
        }
        let mut child = command
            .args(["auth", "login", "--claudeai"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()?;
        let status = tokio::time::timeout(Duration::from_secs(35), child.wait())
            .await
            .context("Native Claude login refresh timed out")??;
        if !status.success() {
            bail!("Native Claude login refresh failed; sign in again");
        }
        let copy = p.clone();
        credentials = tokio::task::spawn_blocking(move || read_credentials(&copy)).await??;
    }
    credentials
        .pointer("/claudeAiOauth/accessToken")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .context("Claude subscription login unavailable")
}

const USAGE_RATE_LIMITED: &str = "Claude usage checks are temporarily rate limited (HTTP 429). Automatic retry is pending; signing in again is not required.";

#[derive(Default, Serialize, Deserialize)]
struct UsageProbe {
    #[serde(default)]
    retry_at: i64,
    #[serde(default)]
    usage: Option<ClaudeUsage>,
}

impl UsageProbe {
    fn cached(&self, now: i64, after_warmup: bool) -> Result<Option<ClaudeUsage>> {
        if self.retry_at > now {
            bail!(USAGE_RATE_LIMITED);
        }
        if after_warmup {
            return Ok(None);
        }
        Ok(self
            .usage
            .clone()
            .filter(|u| (0..45).contains(&(now - u.checked_at))))
    }
}

fn usage_retry_delay(value: Option<&str>, now: i64) -> i64 {
    value
        .and_then(|v| {
            v.parse::<i64>().ok().or_else(|| {
                chrono::DateTime::parse_from_rfc2822(v)
                    .ok()
                    .map(|date| date.timestamp() - now)
            })
        })
        .unwrap_or(300)
        .clamp(1, 86400)
}

async fn fetch_usage(p: &ClaudeProfile, after_warmup: bool) -> Result<ClaudeUsage> {
    let directory = root()?.join(&p.id);
    private_dir(&directory)?;
    let probe_path = directory.join("usage-probe.json");
    // Share throttling across the UI, warm-up and managed terminal processes.
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(directory.join("usage-probe.lock"))?;
    lock.try_lock()
        .map_err(|_| anyhow::anyhow!("Claude usage is already being checked"))?;
    let probe: UsageProbe = match fs::read(&probe_path) {
        Ok(bytes) => serde_json::from_slice(&bytes)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => UsageProbe::default(),
        Err(error) => return Err(error.into()),
    };
    if let Some(usage) = probe.cached(chrono::Utc::now().timestamp(), after_warmup)? {
        return Ok(usage);
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    for force in [false, true] {
        let token = native_access_token(p, force).await?;
        // This read-only endpoint is used by the native CLI's /usage command.
        let response = client
            .get("https://api.anthropic.com/api/oauth/usage")
            .bearer_auth(token)
            .header("anthropic-beta", "oauth-2025-04-20")
            .send()
            .await?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED && !force {
            continue;
        }
        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let now = chrono::Utc::now().timestamp();
            let delay = usage_retry_delay(
                response
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|value| value.to_str().ok()),
                now,
            );
            write_json(
                &probe_path,
                &UsageProbe {
                    retry_at: now + delay,
                    usage: None,
                },
            )?;
            bail!(USAGE_RATE_LIMITED);
        }
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            bail!("Claude subscription login is unavailable; sign in again");
        }
        if !response.status().is_success() {
            bail!(
                "Claude usage check failed (HTTP {})",
                response.status().as_u16()
            );
        }
        let usage = parse_usage(&response.json::<Value>().await?)?;
        write_json(
            &probe_path,
            &UsageProbe {
                retry_at: 0,
                usage: Some(usage.clone()),
            },
        )?;
        return Ok(usage);
    }
    bail!("Claude usage could not be verified")
}

fn verified_plan(status: &Value) -> Option<String> {
    status
        .get("subscriptionType")
        .and_then(Value::as_str)
        .map(|s| s.to_ascii_lowercase())
}

fn paid_plan(plan: Option<&str>) -> bool {
    matches!(plan, Some("pro" | "max" | "team" | "enterprise"))
}

fn eligible(p: &ClaudeProfile, tried: &HashSet<String>, now: i64) -> bool {
    p.logged_in
        && paid_plan(p.plan_type.as_deref())
        && p.error.is_none()
        && !tried.contains(&p.id)
        && p.usage.as_ref().is_some_and(|u| {
            (0..=60).contains(&(now - u.checked_at))
                && u.five_hour_used < 100.0
                && u.seven_day_used < 100.0
                && u.model_used
                    .is_none_or(|used| used.is_finite() && used < 100.0)
        })
}

#[tauri::command]
pub async fn list_claude_profiles() -> Result<ClaudeProfiles, String> {
    tokio::task::spawn_blocking(|| {
        let mut store = load()?;
        store.waiting_for_quota = waiting_for_quota()?;
        let Ok((live, backups)) = desktop_paths() else {
            for p in &mut store.profiles {
                p.desktop_status = "unavailable".into();
            }
            return Ok::<_, anyhow::Error>(store);
        };
        for p in &mut store.profiles {
            if uuid::Uuid::parse_str(&p.id).is_err() {
                p.desktop_status = "unavailable".into();
                continue;
            }
            p.account_uuid = code_account_uuid(p).or_else(|| p.account_uuid.clone());
            let path = if store.active_desktop_id.as_deref() == Some(&p.id) {
                live.clone()
            } else {
                backups.join(&p.id)
            };
            p.desktop_status = match account_uuid(&path.join("config.json")) {
                None => "login_required",
                Some(identity) if Some(identity.as_str()) == p.account_uuid.as_deref() => "saved",
                _ => "different_account",
            }
            .into();
        }
        Ok::<_, anyhow::Error>(store)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

fn account_uuid(path: &Path) -> Option<String> {
    let value: Value = serde_json::from_slice(&fs::read(path).ok()?).ok()?;
    value
        .pointer("/oauthAccount/accountUuid")
        .or_else(|| value.get("lastKnownAccountUuid"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn code_account_uuid(p: &ClaudeProfile) -> Option<String> {
    let path = if p.is_default {
        home().ok()?.join(".claude.json")
    } else {
        code_dir(p).ok()?.join(".claude.json")
    };
    account_uuid(&path)
}

fn identify_live_desktop(store: &mut ClaudeProfiles, live: &Path) -> Result<()> {
    for p in &mut store.profiles {
        p.account_uuid = code_account_uuid(p).or_else(|| p.account_uuid.clone());
    }
    if let Some(identity) = account_uuid(&live.join("config.json")) {
        if let Some(selected) = store.active_desktop_id.as_deref() {
            let expected = profile(store, selected)?;
            if expected.account_uuid.as_deref() != Some(identity.as_str()) {
                bail!("Another account is open in this Desktop profile. Sign out of Claude and sign in with {}; saved profiles were preserved", expected.email);
            }
        } else {
            let matched = store.profiles.iter().find(|p| p.account_uuid.as_deref() == Some(&identity))
                .context("Add the account currently signed in to Claude Desktop before switching its profile")?;
            store.active_desktop_id = Some(matched.id.clone());
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn import_current_claude_profile() -> Result<ClaudeProfiles, String> {
    tokio::task::spawn_blocking(|| -> Result<ClaudeProfiles> {
        let mut p = ClaudeProfile {
            id: uuid::Uuid::new_v4().to_string(),
            email: String::new(),
            is_default: true,
            account_uuid: None,
            desktop_status: String::new(),
            logged_in: false,
            plan_type: None,
            last_warmup_at: None,
            usage: None,
            error: None,
        };
        let status = login_status(&p)?;
        if status.get("loggedIn").and_then(Value::as_bool) != Some(true)
            || status.get("authMethod").and_then(Value::as_str) != Some("claude.ai")
        {
            bail!("Sign in to Claude Code with your subscription first");
        }
        p.email = status
            .get("email")
            .and_then(Value::as_str)
            .context("Claude login has no email")?
            .to_owned();
        p.logged_in = true;
        p.plan_type = verified_plan(&status);
        let _lock = lock_store()?;
        let mut store = load()?;
        if store.profiles.iter().any(|p| p.is_default) {
            return Ok(store);
        }
        store.active_code_id = Some(p.id.clone());
        p.account_uuid = code_account_uuid(&p);
        store.profiles.push(p);
        let live = home()?.join("Library/Application Support/Claude");
        let _ = identify_live_desktop(&mut store, &live);
        save(&store)?;
        Ok(store)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn refresh_claude_profiles() -> Result<ClaudeProfiles, String> {
    let profiles = load().map_err(|e| e.to_string())?.profiles;
    let mut updates = Vec::new();
    for mut p in profiles {
        let probe = p.clone();
        let status = tokio::task::spawn_blocking(move || login_status(&probe))
            .await
            .map_err(|e| e.to_string())?;
        p.logged_in = status.as_ref().is_ok_and(|s| {
            s.get("loggedIn").and_then(Value::as_bool) == Some(true)
                && s.get("authMethod").and_then(Value::as_str) == Some("claude.ai")
                && s.get("email")
                    .and_then(Value::as_str)
                    .is_some_and(|email| email.eq_ignore_ascii_case(&p.email))
        });
        p.plan_type = if p.logged_in {
            status.as_ref().ok().and_then(verified_plan)
        } else {
            None
        };
        p.usage = None;
        p.error = None;
        if p.logged_in {
            match fetch_usage(&p, false).await {
                Ok(usage) => p.usage = Some(usage),
                Err(e) => p.error = Some(e.to_string()),
            }
        } else {
            p.error = Some("Sign in with this account to verify its remaining usage".into());
        }
        updates.push(p);
    }
    let _lock = lock_store().map_err(|e| e.to_string())?;
    let mut store = load().map_err(|e| e.to_string())?;
    for updated in updates {
        if let Some(p) = store.profiles.iter_mut().find(|p| p.id == updated.id) {
            p.logged_in = updated.logged_in;
            p.plan_type = updated.plan_type;
            p.usage = updated.usage;
            p.error = updated.error;
        }
    }
    save(&store).map_err(|e| e.to_string())?;
    Ok(store)
}

fn extract_auth_url(line: &str) -> Option<String> {
    line.split_whitespace().find_map(|word| {
        let start = word.find("https://")?;
        let url = word[start..].split('\u{1b}').next()?;
        let parsed = url::Url::parse(url).ok()?;
        let approved = matches!(
            (parsed.host_str(), parsed.path()),
            (Some("claude.ai"), "/oauth/authorize") | (Some("claude.com"), "/cai/oauth/authorize")
        );
        (approved && parsed.username().is_empty() && parsed.password().is_none())
            .then(|| url.to_owned())
    })
}

#[tauri::command]
pub async fn start_claude_login(email: String) -> Result<ClaudeLogin, String> {
    let email = email.trim().to_lowercase();
    if !email.contains('@') || email.len() > 254 || email.chars().any(char::is_whitespace) {
        return Err("Enter a valid email address".into());
    }
    {
        let mut login = LOGIN.lock().map_err(|_| "Claude login state unavailable")?;
        if login
            .as_ref()
            .is_some_and(|l| l.status == "pending" || l.status == "cancelling")
        {
            return Err("A Claude login is already pending; finish or cancel it first".into());
        }
        *login = Some(ClaudeLogin {
            status: "pending".into(),
            ..Default::default()
        });
    }
    let result = start_login_process(email).await;
    if let Err(e) = &result {
        *LOGIN.lock().map_err(|_| "Claude login state unavailable")? = Some(ClaudeLogin {
            status: "failed".into(),
            error: Some(e.to_string()),
            ..Default::default()
        });
    }
    result.map_err(|e| e.to_string())
}

async fn start_login_process(email: String) -> Result<ClaudeLogin> {
    let p = {
        let _lock = lock_store()?;
        let mut store = load()?;
        let p = if let Some(p) = store
            .profiles
            .iter()
            .find(|p| p.email.eq_ignore_ascii_case(&email))
        {
            p.clone()
        } else {
            let p = ClaudeProfile {
                id: uuid::Uuid::new_v4().to_string(),
                email,
                is_default: false,
                account_uuid: None,
                desktop_status: String::new(),
                logged_in: false,
                plan_type: None,
                last_warmup_at: None,
                usage: None,
                error: None,
            };
            store.profiles.push(p.clone());
            save(&store)?;
            p
        };
        if p.is_default {
            bail!("The existing login is already imported; add a different account");
        }
        p
    };
    prepare_code_dir(&p)?;
    let mut cmd = tokio::process::Command::from(code_command(&p)?);
    let mut child = cmd
        .args(["auth", "login", "--claudeai", "--email", &p.email])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let mut stdin = child.stdin.take().context("No login input")?;
    let (input_tx, mut input_rx) = tokio::sync::mpsc::channel::<String>(1);
    let mut stdout = BufReader::new(child.stdout.take().context("No login output")?).lines();
    let mut stderr = BufReader::new(child.stderr.take().context("No login error output")?).lines();
    let pending = ClaudeLogin {
        flow_id: uuid::Uuid::new_v4().to_string(),
        profile_id: p.id.clone(),
        status: "pending".into(),
        ..Default::default()
    };
    let flow_id = pending.flow_id.clone();
    *LOGIN_INPUT
        .lock()
        .map_err(|_| anyhow::anyhow!("Claude login input unavailable"))? =
        Some((flow_id.clone(), input_tx));
    *LOGIN
        .lock()
        .map_err(|_| anyhow::anyhow!("Claude login state unavailable"))? = Some(pending.clone());
    tokio::spawn(async move {
        let mut out_open = true;
        let mut err_open = true;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(600);
        let result: Result<()> = async {
            loop {
                if LOGIN.lock().ok().and_then(|s| s.clone()).is_none_or(|l| l.status != "pending" || l.flow_id != flow_id) { bail!("Claude login cancelled"); }
                tokio::select! {
                    code = input_rx.recv() => {
                        if let Some(code) = code {
                            stdin.write_all(format!("{code}\n").as_bytes()).await?;
                            stdin.flush().await?;
                        }
                    }
                    line = stdout.next_line(), if out_open => { match line? { Some(line) => update_login_url(&flow_id, &line), None => out_open = false } }
                    line = stderr.next_line(), if err_open => { match line? { Some(line) => update_login_url(&flow_id, &line), None => err_open = false } }
                    status = child.wait() => { if !status?.success() { bail!("Claude login failed; try the native login again"); } break; }
                    _ = tokio::time::sleep_until(deadline) => bail!("Claude login timed out"),
                    _ = tokio::time::sleep(Duration::from_millis(250)) => {}
                }
            }
            let verify = p.clone();
            let status = tokio::task::spawn_blocking(move || login_status(&verify)).await??;
            if status.get("loggedIn").and_then(Value::as_bool) != Some(true)
                || status.get("authMethod").and_then(Value::as_str) != Some("claude.ai")
                || !status.get("email").and_then(Value::as_str).is_some_and(|e| e.eq_ignore_ascii_case(&p.email)) {
                bail!("Signed-in account does not match the requested email");
            }
            let _lock = lock_store()?;
            let mut store = load()?;
            if let Some(p) = store.profiles.iter_mut().find(|a| a.id == p.id) { p.logged_in = true; p.plan_type = verified_plan(&status); p.error = None; p.account_uuid = code_account_uuid(p); }
            save(&store)?;
            Ok(())
        }.await;
        let _ = child.kill().await;
        if let Ok(mut input) = LOGIN_INPUT.lock() {
            if input.as_ref().is_some_and(|(id, _)| id == &flow_id) {
                *input = None;
            }
        }
        if let Ok(mut state) = LOGIN.lock() {
            if let Some(state) = state.as_mut().filter(|s| s.flow_id == flow_id) {
                state.status = if result.is_ok() { "complete" } else { "failed" }.into();
                state.error = result.err().map(|e| e.to_string());
                state.auth_url = None;
            }
        }
    });
    Ok(pending)
}

fn parse_login_return(input: &str, auth_url: &str) -> Result<String> {
    let input = input.trim();
    if input.is_empty() || input.len() > 8192 || input.chars().any(char::is_whitespace) {
        bail!("Paste the final callback URL or the complete code#state");
    }
    let auth = url::Url::parse(auth_url)?;
    let expected_state = auth
        .query_pairs()
        .find(|(key, _)| key == "state")
        .map(|(_, value)| value.into_owned())
        .context("This Claude login is no longer pending; generate a new link")?;
    let (code, state) = if input.starts_with("http://") || input.starts_with("https://") {
        let callback = url::Url::parse(input)
            .context("Paste the final callback URL or the complete code#state")?;
        let redirect = auth
            .query_pairs()
            .find(|(key, _)| key == "redirect_uri")
            .map(|(_, value)| value.into_owned())
            .context("The return link does not belong to this login")?;
        let redirect = url::Url::parse(&redirect)?;
        if callback.origin() != redirect.origin()
            || callback.path() != redirect.path()
            || callback.username() != ""
            || callback.password().is_some()
        {
            bail!("The return link does not belong to this login");
        }
        let mut codes = callback
            .query_pairs()
            .filter(|(key, _)| key == "code")
            .map(|(_, value)| value.into_owned());
        let code = codes
            .next()
            .context("Paste the final callback URL or the complete code#state")?;
        if codes.next().is_some() {
            bail!("The return link does not belong to this login");
        }
        let mut states = callback
            .query_pairs()
            .filter(|(key, _)| key == "state")
            .map(|(_, value)| value.into_owned());
        let state = states
            .next()
            .context("Paste the final callback URL or the complete code#state")?;
        if states.next().is_some() {
            bail!("The return link does not belong to this login");
        }
        (code, state)
    } else {
        let (code, state) = input
            .split_once('#')
            .context("Paste the final callback URL or the complete code#state")?;
        (code.to_owned(), state.to_owned())
    };
    if state != expected_state {
        bail!("The authorization state does not match this login");
    }
    if code.is_empty()
        || code.contains('#')
        || code.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        bail!("Paste the final callback URL or the complete code#state");
    }
    Ok(format!("{code}#{state}"))
}

#[tauri::command]
pub async fn complete_claude_login(flow_id: String, input: String) -> Result<(), String> {
    let login = get_claude_login()?
        .filter(|l| l.status == "pending" && l.flow_id == flow_id)
        .ok_or("This Claude login is no longer pending; generate a new link")?;
    let code = parse_login_return(
        &input,
        login
            .auth_url
            .as_deref()
            .ok_or("Claude login is unavailable")?,
    )
    .map_err(|e| e.to_string())?;
    let sender = LOGIN_INPUT
        .lock()
        .map_err(|_| "Claude login input unavailable")?
        .as_ref()
        .filter(|(id, _)| id == &flow_id)
        .map(|(_, sender)| sender.clone())
        .ok_or("This Claude login is no longer pending; generate a new link")?;
    sender
        .try_send(code)
        .map_err(|_| "Claude could not receive the authorization code; generate a new link".into())
}

fn update_login_url(flow_id: &str, line: &str) {
    if let Some(url) = extract_auth_url(line) {
        if let Ok(mut login) = LOGIN.lock() {
            if let Some(login) = login
                .as_mut()
                .filter(|l| l.flow_id == flow_id && l.status == "pending")
            {
                login.auth_url = Some(url);
            }
        }
    }
}

#[tauri::command]
pub fn get_claude_login() -> Result<Option<ClaudeLogin>, String> {
    LOGIN
        .lock()
        .map(|l| l.clone())
        .map_err(|_| "Claude login state unavailable".into())
}

#[tauri::command]
pub fn cancel_claude_login() -> Result<(), String> {
    if let Some(login) = LOGIN
        .lock()
        .map_err(|_| "Claude login state unavailable")?
        .as_mut()
        .filter(|s| s.status == "pending")
    {
        login.status = "cancelling".into();
        login.auth_url = None;
    }
    Ok(())
}

#[tauri::command]
pub async fn set_claude_auto_switch(enabled: bool) -> Result<ClaudeProfiles, String> {
    let _lock = lock_store().map_err(|e| e.to_string())?;
    let mut store = load().map_err(|e| e.to_string())?;
    store.auto_switch_enabled = enabled;
    save(&store).map_err(|e| e.to_string())?;
    Ok(store)
}

fn warmup_due(p: &ClaudeProfile, now: i64) -> bool {
    if !eligible(p, &HashSet::new(), now) {
        return false;
    }
    let usage = p.usage.as_ref().unwrap();
    usage.five_hour_used == 0.0
        && usage.five_hour_resets_at.as_deref().map_or(true, |reset| {
            chrono::DateTime::parse_from_rfc3339(reset).is_ok_and(|date| date.timestamp() <= now)
        })
        && p.last_warmup_at.is_none_or(|last| now >= last + 5 * 3600)
}

fn warmup_command(p: &ClaudeProfile) -> Result<Command> {
    uuid::Uuid::parse_str(&p.id).context("Invalid Claude profile ID")?;
    let dir = root()?.join(&p.id).join("warmup");
    private_dir(&dir)?;
    let mut command = code_command(p)?;
    command.current_dir(dir).args([
        "-p",
        "Reply with exactly OK.",
        "--model",
        "haiku",
        "--tools",
        "",
        "--strict-mcp-config",
        "--mcp-config",
        "{\"mcpServers\":{}}",
        "--setting-sources",
        "",
        "--settings",
        "{\"disableAllHooks\":true}",
        "--disable-slash-commands",
        "--no-session-persistence",
        "--max-turns",
        "1",
        "--output-format",
        "json",
    ]);
    Ok(command)
}

async fn warmup_native(p: &ClaudeProfile) -> Result<()> {
    let probe = p.clone();
    let status = tokio::task::spawn_blocking(move || login_status(&probe)).await??;
    if status.get("loggedIn").and_then(Value::as_bool) != Some(true)
        || !status
            .get("email")
            .and_then(Value::as_str)
            .is_some_and(|e| e.eq_ignore_ascii_case(&p.email))
    {
        bail!("Sign in to this Claude account first");
    }
    let usage = fetch_usage(p, false).await?;
    let mut verified = p.clone();
    verified.logged_in = true;
    verified.plan_type = verified_plan(&status);
    verified.error = None;
    verified.usage = Some(usage);
    if !eligible(&verified, &HashSet::new(), chrono::Utc::now().timestamp()) {
        bail!("No verified Claude quota is available for warm-up");
    }
    let mut command = tokio::process::Command::from(warmup_command(p)?);
    let child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let output = tokio::time::timeout(Duration::from_secs(90), child.wait_with_output())
        .await
        .context("Claude warm-up timed out")??;
    if !output.status.success() {
        bail!("Claude warm-up failed");
    }
    let response: Value =
        serde_json::from_slice(&output.stdout).context("Claude warm-up was not confirmed")?;
    if response.get("is_error").and_then(Value::as_bool) != Some(false)
        || response
            .get("result")
            .and_then(Value::as_str)
            .is_none_or(|result| result.trim() != "OK")
    {
        bail!("Claude warm-up was not confirmed");
    }
    let fresh = fetch_usage(p, true).await;
    let error = fresh.as_ref().err().map(ToString::to_string);
    let _lock = lock_store()?;
    let mut store = load()?;
    if let Some(saved) = store.profiles.iter_mut().find(|saved| saved.id == p.id) {
        saved.last_warmup_at = Some(chrono::Utc::now().timestamp());
        saved.usage = fresh.ok();
        saved.error = error;
    }
    save(&store)?;
    Ok(())
}

#[tauri::command]
pub async fn warmup_claude_account(profile_id: String) -> Result<ClaudeProfiles, String> {
    let _guard = WARMUP_LOCK
        .try_lock()
        .map_err(|_| "Claude warm-up is already running")?;
    let store = load().map_err(|e| e.to_string())?;
    let p = profile(&store, &profile_id)
        .map_err(|e| e.to_string())?
        .clone();
    warmup_native(&p).await.map_err(|e| e.to_string())?;
    list_claude_profiles().await
}

#[tauri::command]
pub async fn set_claude_auto_warmup(enabled: bool) -> Result<ClaudeProfiles, String> {
    let _lock = lock_store().map_err(|e| e.to_string())?;
    let mut store = load().map_err(|e| e.to_string())?;
    store.auto_warmup_enabled = enabled;
    save(&store).map_err(|e| e.to_string())?;
    Ok(store)
}

pub fn spawn_auto_warmup(app: tauri::AppHandle) {
    use tauri::Emitter;
    tauri::async_runtime::spawn(async move {
        let mut timer = tokio::time::interval(Duration::from_secs(60));
        let mut retry_after: HashMap<String, i64> = HashMap::new();
        loop {
            timer.tick().await;
            if !load().is_ok_and(|store| store.auto_warmup_enabled) {
                continue;
            }
            let Ok(_guard) = WARMUP_LOCK.try_lock() else {
                continue;
            };
            let Ok(store) = refresh_claude_profiles().await else {
                continue;
            };
            for p in store.profiles {
                if !load().is_ok_and(|store| store.auto_warmup_enabled) {
                    break;
                }
                let now = chrono::Utc::now().timestamp();
                if retry_after.get(&p.id).is_some_and(|after| now < *after) || !warmup_due(&p, now)
                {
                    continue;
                }
                let result = warmup_native(&p).await;
                if result.is_err() {
                    retry_after.insert(p.id.clone(), now + 300);
                }
                let _ = app.emit("claude-warmup-event", json!({"profileId":p.id,"ok":result.is_ok(),"error":result.err().map(|e| e.to_string())}));
            }
        }
    });
}

fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[tauri::command]
pub async fn open_claude_code(
    profile_id: String,
    cwd: Option<String>,
) -> Result<ClaudeProfiles, String> {
    tokio::task::spawn_blocking(move || -> Result<ClaudeProfiles> {
        let cwd = cwd
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or(home()?);
        if !cwd.is_dir() {
            bail!("Choose an existing project directory");
        }
        let _lock = lock_store()?;
        let mut store = load()?;
        let p = profile(&store, &profile_id)?;
        if !p.logged_in {
            bail!("Sign in to this Claude account first");
        }
        let command = format!(
            "{} --claude-run {} {}",
            quote(&std::env::current_exe()?.to_string_lossy()),
            quote(&profile_id),
            quote(&cwd.to_string_lossy())
        );
        #[cfg(target_os = "macos")]
        {
            let script = format!(
                "tell application \"Terminal\"\nactivate\ndo script {}\nend tell",
                serde_json::to_string(&command)?
            );
            if !Command::new("osascript")
                .args(["-e", &script])
                .status()?
                .success()
            {
                bail!("Could not open Claude Code terminal");
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = command;
            bail!("Claude terminal launching is currently supported on macOS");
        }
        store.active_code_id = Some(profile_id);
        save(&store)?;
        Ok(store)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

/// The helper is a separate terminal process; it never starts the Switcher UI.
pub fn run_cli_helper(args: &[String]) -> Option<Result<()>> {
    match args.first().map(String::as_str) {
        Some("--claude-hook") => Some(record_failure(args)),
        Some("--claude-run") => Some(run_session(args)),
        Some("--claude-import-current") => Some(
            tokio::runtime::Runtime::new()
                .context("Runtime unavailable")
                .and_then(|rt| {
                    rt.block_on(import_current_claude_profile())
                        .map(|_| ())
                        .map_err(anyhow::Error::msg)
                }),
        ),
        Some("--claude-login") => Some((|| -> Result<()> {
            let email = args.get(1).context("Missing Claude account email")?.clone();
            tokio::runtime::Runtime::new()?.block_on(async {
                start_claude_login(email)
                    .await
                    .map_err(anyhow::Error::msg)?;
                let mut printed = false;
                loop {
                    let login = get_claude_login()
                        .map_err(anyhow::Error::msg)?
                        .context("Claude login is unavailable")?;
                    if let Some(url) = login.auth_url.filter(|_| !printed) {
                        println!("{url}");
                        printed = true;
                    }
                    match login.status.as_str() {
                        "complete" => return Ok(()),
                        "failed" | "cancelled" => bail!(
                            "{}",
                            login
                                .error
                                .unwrap_or_else(|| "Claude login cancelled".into())
                        ),
                        _ => tokio::time::sleep(Duration::from_millis(250)).await,
                    }
                }
            })
        })()),
        _ => None,
    }
}

fn event_path(run_id: &str) -> Result<PathBuf> {
    uuid::Uuid::parse_str(run_id).context("Invalid Claude runner ID")?;
    Ok(root()?.join("events").join(format!("{run_id}.json")))
}

// Runner processes cannot emit Tauri events; short-lived status files expose their wait state.
fn waiting_for_quota() -> Result<bool> {
    let path = root()?.join("waiting");
    if !path.exists() {
        return Ok(false);
    }
    let now = chrono::Utc::now().timestamp();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if let Ok(bytes) = fs::read(entry.path()) {
            if let Ok(timestamp) = serde_json::from_slice::<i64>(&bytes) {
                if (0..=90).contains(&(now - timestamp)) {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

fn runner_hook_marker(event: &Value) -> Result<Option<Value>> {
    match event.get("hook_event_name").and_then(Value::as_str) {
        Some("UserPromptSubmit") => Ok(Some(json!({"kind":"prompt"}))),
        Some("StopFailure") if event.get("error").and_then(Value::as_str) == Some("rate_limit") => {
            let session = event.get("session_id").and_then(Value::as_str).context("Missing session ID")?;
            uuid::Uuid::parse_str(session)?;
            Ok(Some(json!({"kind":"rate_limit","session_id":session})))
        }
        _ => Ok(None),
    }
}

fn record_failure(args: &[String]) -> Result<()> {
    let path = event_path(args.get(1).context("Missing runner ID")?)?;
    let mut input = String::new();
    std::io::stdin()
        .take(64 * 1024)
        .read_to_string(&mut input)?;
    let event: Value = serde_json::from_str(&input)?;
    if let Some(marker) = runner_hook_marker(&event)? {
        write_json(&path, &marker)?;
    }
    Ok(())
}

fn run_session(args: &[String]) -> Result<()> {
    let mut id = args.get(1).context("Missing account ID")?.clone();
    let cwd = PathBuf::from(args.get(2).context("Missing project directory")?);
    if !cwd.is_dir() {
        bail!("Project directory is unavailable");
    }
    let run_id = uuid::Uuid::new_v4().to_string();
    let event = event_path(&run_id)?;
    let waiting_path = root()?.join("waiting").join(format!("{run_id}.json"));
    let mut pending_failure: Option<Value> = None;
    let mut next_probe = Instant::now();
    let hook = format!(
        "{} --claude-hook {}",
        quote(&std::env::current_exe()?.to_string_lossy()),
        quote(&run_id)
    );
    let settings = json!({"hooks":{"StopFailure":[{"matcher":"rate_limit","hooks":[{"type":"command","command":hook,"timeout":5}]}],"UserPromptSubmit":[{"hooks":[{"type":"command","command":hook,"timeout":5}]}]}}).to_string();
    let rt = tokio::runtime::Runtime::new()?;
    let mut resume: Option<String> = None;
    let mut tried = HashMap::new();
    loop {
        let store = load()?;
        let p = profile(&store, &id)?;
        prepare_code_dir(p)?;
        tried.insert(id.clone(), Instant::now());
        let mut command = code_command(p)?;
        command.current_dir(&cwd).args(["--settings", &settings]);
        if let Some(session) = &resume {
            command.args([
                "--resume",
                session,
                &crate::auth::load_app_settings()?.continue_phrase,
            ]);
        }
        let mut child = command.spawn()?;
        loop {
            if let Some(status) = child.try_wait()? {
                let _ = fs::remove_file(&event);
                let _ = fs::remove_file(&waiting_path);
                if !status.success() {
                    bail!("Claude exited with {status}");
                }
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(500));
            if event.exists() {
                let marker: Value = serde_json::from_slice(&fs::read(&event)?)?;
                fs::remove_file(&event)?;
                if marker.get("kind").and_then(Value::as_str) == Some("prompt") {
                    pending_failure = None;
                    let _ = fs::remove_file(&waiting_path);
                    continue;
                }
                pending_failure = Some(marker);
                next_probe = Instant::now();
                tried.insert(id.clone(), Instant::now());
            }
            if !load()?.auto_switch_enabled {
                pending_failure = None;
                let _ = fs::remove_file(&waiting_path);
                continue;
            }
            let Some(failure) = pending_failure.as_ref() else {
                continue;
            };
            if Instant::now() < next_probe {
                continue;
            }
            next_probe = Instant::now() + Duration::from_secs(30);
            let refreshed = match rt.block_on(refresh_claude_profiles()) {
                Ok(store) => store,
                Err(error) => {
                    let _ = write_json(&waiting_path, &chrono::Utc::now().timestamp());
                    eprintln!("\n[Switcher] Usage could not be verified: {error}. Current account retained; retrying in 30 seconds.");
                    continue;
                }
            };
            tried.retain(|_, time| time.elapsed() < Duration::from_secs(60));
            let excluded = tried.keys().cloned().collect();
            let target = refreshed
                .profiles
                .iter()
                .filter(|p| eligible(p, &excluded, chrono::Utc::now().timestamp()))
                .max_by(|a, b| {
                    let remaining = |p: &ClaudeProfile| {
                        p.usage
                            .as_ref()
                            .map(|u| (100.0 - u.five_hour_used).min(100.0 - u.seven_day_used))
                            .unwrap_or(0.0)
                    };
                    remaining(a).total_cmp(&remaining(b))
                });
            let Some(target) = target else {
                let _ = write_json(&waiting_path, &chrono::Utc::now().timestamp());
                eprintln!("\n[Switcher] No verified Pro/Max/Team/Enterprise account with remaining usage. Waiting for a verified quota reset; retrying in 30 seconds.");
                continue;
            };
            // A user may submit a new prompt while native quota probes are in flight.
            if event.exists() { continue; }
            let session = failure
                .get("session_id")
                .and_then(Value::as_str)
                .context("Missing Claude session ID")?;
            uuid::Uuid::parse_str(session)?;
            #[cfg(unix)]
            {
                Command::new("kill")
                    .args(["-TERM", &child.id().to_string()])
                    .status()?;
                let until = Instant::now() + Duration::from_secs(10);
                while child.try_wait()?.is_none() {
                    if Instant::now() >= until {
                        bail!("Claude did not stop cleanly; account was not switched");
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
            #[cfg(not(unix))]
            {
                bail!("Automatic Claude continuation currently requires macOS or Linux");
            }
            eprintln!(
                "\n[Switcher] Continuing the same local session with {}",
                target.email
            );
            let _ = fs::remove_file(&waiting_path);
            id = target.id.clone();
            resume = Some(session.to_owned());
            pending_failure = None;
            let _lock = lock_store()?;
            let mut store = load()?;
            store.active_code_id = Some(id.clone());
            save(&store)?;
            break;
        }
    }
}

#[derive(Serialize, Deserialize)]
struct DesktopTransaction {
    from: String,
    to: String,
}

fn desktop_paths() -> Result<(PathBuf, PathBuf)> {
    #[cfg(target_os = "macos")]
    {
        let parent = home()?.join("Library/Application Support");
        Ok((
            parent.join("Claude"),
            parent.join("Claude Switcher Profiles"),
        ))
    }
    #[cfg(not(target_os = "macos"))]
    {
        bail!("Claude Desktop profile switching currently supports macOS")
    }
}

fn desktop_running() -> Result<bool> {
    let native = Command::new("osascript")
        .args([
            "-e",
            "application id \"com.anthropic.claudefordesktop\" is running",
        ])
        .output()?;
    if !native.status.success() {
        bail!("Could not inspect Claude Desktop; install the native app first");
    }
    if String::from_utf8_lossy(&native.stdout).trim() == "true" {
        return Ok(true);
    }
    // `comm` is the executable path without arguments, so command lines that merely
    // mention Claude.app cannot match.
    let output = Command::new("ps").args(["-axo", "comm="]).output()?;
    if !output.status.success() {
        bail!("Could not verify that Claude Desktop has stopped");
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .any(|line| writes_desktop_profile(line.trim())))
}

/// The GUI and its Electron helpers (renderer, GPU, utility) write the profile.
/// Bundled tools such as chrome-native-host (owned by a browser), crashpad and
/// the embedded Claude Code CLI outlive the app and never hold the profile.
fn writes_desktop_profile(executable: &str) -> bool {
    let Some((bundle, rest)) = executable.rsplit_once("/Claude.app/Contents/") else {
        return false;
    };
    if !bundle.starts_with('/') && !bundle.is_empty() {
        return false;
    }
    if rest == "MacOS/Claude" {
        return true;
    }
    rest.strip_prefix("Frameworks/")
        .and_then(|helper| helper.split_once(".app/Contents/MacOS/"))
        .is_some_and(|(app, binary)| app == binary && binary.starts_with("Claude Helper"))
}

fn launch_desktop() -> Result<()> {
    if !Command::new("open")
        .args(["-a", "Claude"])
        .status()?
        .success()
    {
        bail!("Could not open Claude Desktop");
    }
    Ok(())
}

/// After Claude was quit, a failed switch reopens it only when the live profile is
/// intact and no transaction journal is pending; the original error always wins.
fn reopen_after_failure<T>(
    result: Result<T>,
    quit: bool,
    live: &Path,
    journal: &Path,
    launch: impl FnOnce() -> Result<()>,
) -> Result<T> {
    if result.is_err() && quit && live.is_dir() && !journal.exists() {
        let _ = launch();
    }
    result
}

fn recover_desktop_transaction(
    store: &mut ClaudeProfiles,
    live: &Path,
    backups: &Path,
    journal: &Path,
) -> Result<()> {
    if !journal.exists() {
        return Ok(());
    }
    let tx: DesktopTransaction = serde_json::from_slice(&fs::read(journal)?)?;
    uuid::Uuid::parse_str(&tx.from)?;
    uuid::Uuid::parse_str(&tx.to)?;
    profile(store, &tx.from)?;
    profile(store, &tx.to)?;
    let from = backups.join(&tx.from);
    let to = backups.join(&tx.to);
    match (live.is_dir(), from.is_dir(), to.is_dir()) {
        (true, true, false) => store.active_desktop_id = Some(tx.to),
        (false, true, true) => {
            fs::rename(from, live)?;
            store.active_desktop_id = Some(tx.from);
        }
        (true, false, true) => store.active_desktop_id = Some(tx.from),
        _ => bail!(
            "Claude desktop recovery is ambiguous; all profiles were preserved for inspection"
        ),
    }
    save(store)?;
    fs::remove_file(journal)?;
    Ok(())
}

fn swap_directories(live: &Path, from: &Path, to: &Path) -> Result<()> {
    if from.exists() {
        bail!("The outgoing Claude backup already exists; no data was overwritten");
    }
    if !to.is_dir() {
        bail!("The target Claude desktop profile is unavailable");
    }
    fs::rename(live, from).context("Could not preserve the current Claude desktop profile")?;
    if let Err(error) = fs::rename(to, live) {
        fs::rename(from, live)
            .context("Claude desktop rollback failed; the preserved profile must be restored")?;
        return Err(error.into());
    }
    Ok(())
}

#[tauri::command]
pub async fn switch_claude_desktop(profile_id: String) -> Result<ClaudeProfiles, String> {
    tokio::task::spawn_blocking(move || -> Result<ClaudeProfiles> {
        let _lock = lock_store()?;
        let mut store = load()?;
        profile(&store, &profile_id)?;
        uuid::Uuid::parse_str(&profile_id)?;
        let (live, backups) = desktop_paths()?;
        let journal = root()?.join("desktop-transaction.json");
        if store.active_desktop_id.as_deref() == Some(&profile_id) && !journal.exists() {
            identify_live_desktop(&mut store, &live)?;
            launch_desktop()?;
            return Ok(store);
        }
        if !journal.exists() {
            identify_live_desktop(&mut store, &live)?;
        }
        // Quit through the app so it can flush Cookies/LevelDB; never force-kill it.
        let quit = desktop_running()?;
        if quit
            && !Command::new("osascript")
                .args(["-e", "tell application \"Claude\" to quit"])
                .status()?
                .success()
        {
            bail!("Claude Desktop could not close; no account data was changed");
        }
        let result = switch_closed_desktop(&mut store, &profile_id, &live, &backups, &journal);
        reopen_after_failure(result, quit, &live, &journal, launch_desktop)?;
        launch_desktop()
            .context("The desktop profile switched successfully; open Claude manually")?;
        Ok(store)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

fn switch_closed_desktop(
    store: &mut ClaudeProfiles,
    profile_id: &str,
    live: &Path,
    backups: &Path,
    journal: &Path,
) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(15);
    while desktop_running()? {
        if Instant::now() >= deadline {
            bail!("Close Claude Desktop before switching accounts");
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    private_dir(backups)?;
    recover_desktop_transaction(store, live, backups, journal)?;
    identify_live_desktop(store, live)?;
    let from_id = store
        .active_desktop_id
        .clone()
        .context("Import the current Claude account before switching Desktop")?;
    uuid::Uuid::parse_str(&from_id)?;
    if from_id == profile_id {
        return Ok(());
    }
    if !live.is_dir() {
        bail!("Open and sign in to Claude Desktop once before switching its profile");
    }
    let to = backups.join(profile_id);
    if !to.exists() {
        private_dir(&to)?;
        let config = live.join("claude_desktop_config.json");
        if config.is_file() {
            fs::copy(config, to.join("claude_desktop_config.json"))?;
        }
    }
    write_json(
        journal,
        &DesktopTransaction {
            from: from_id.clone(),
            to: profile_id.to_string(),
        },
    )?;
    if let Err(e) = swap_directories(live, &backups.join(&from_id), &to) {
        if live.exists() {
            let _ = fs::remove_file(journal);
        }
        return Err(e);
    }
    store.active_desktop_id = Some(profile_id.to_string());
    if let Err(e) = save(store) {
        swap_directories(live, &to, &backups.join(&from_id))?;
        fs::remove_file(journal)?;
        return Err(e);
    }
    fs::remove_file(journal)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_probe_honors_shared_cooldown_and_freshness() {
        let usage =
            parse_usage(&json!({"five_hour":{"utilization":10},"seven_day":{"utilization":20}}))
                .unwrap();
        let now = usage.checked_at;
        let probe = UsageProbe {
            retry_at: now + 300,
            usage: Some(usage.clone()),
        };
        let saved: UsageProbe =
            serde_json::from_slice(&serde_json::to_vec(&probe).unwrap()).unwrap();
        assert!(saved
            .cached(now, false)
            .unwrap_err()
            .to_string()
            .contains("HTTP 429"));
        assert!(saved.cached(now, true).is_err());
        assert!(saved.cached(now + 300, false).unwrap().is_none());
        let probe = UsageProbe {
            retry_at: 0,
            usage: Some(usage),
        };
        assert!(probe.cached(now, true).unwrap().is_none());
        assert!(probe.cached(now + 44, false).unwrap().is_some());
        assert!(probe.cached(now + 45, false).unwrap().is_none());
        assert!(probe.cached(now - 1, false).unwrap().is_none());
        assert_eq!(usage_retry_delay(None, now), 300);
        assert_eq!(usage_retry_delay(Some("120"), now), 120);
        assert_eq!(usage_retry_delay(Some("invalid"), now), 300);
        assert_eq!(usage_retry_delay(Some("-1"), now), 1);
        assert_eq!(usage_retry_delay(Some("999999"), now), 86400);
        assert_eq!(
            usage_retry_delay(Some("Wed, 21 Oct 2015 07:28:00 GMT"), 1445412420),
            60
        );
    }

    #[test]
    fn pasted_login_return_is_bound_to_the_current_flow() {
        let auth = "https://claude.ai/oauth/authorize?state=current&redirect_uri=http%3A%2F%2Flocalhost%3A4321%2Fcallback";
        assert_eq!(
            parse_login_return("abc#current", auth).unwrap(),
            "abc#current"
        );
        assert_eq!(
            parse_login_return(
                "http://localhost:4321/callback?code=abc&state=current",
                auth
            )
            .unwrap(),
            "abc#current"
        );
        for value in [
            "abc#old",
            "abc\n#current",
            "abc\0#current",
            "http://evil.example/callback?code=abc&state=current",
            "http://localhost:9999/callback?code=abc&state=current",
            "http://localhost:4321/callback?code=abc&code=def&state=current",
        ] {
            assert!(parse_login_return(value, auth).is_err());
        }
    }

    #[test]
    fn automatic_warmup_skips_active_stale_exhausted_or_recently_warmed_accounts() {
        let now = chrono::Utc::now().timestamp();
        let mut p = ClaudeProfile {
            id: uuid::Uuid::new_v4().to_string(),
            email: "test@example.com".into(),
            is_default: false,
            account_uuid: None,
            desktop_status: String::new(),
            logged_in: true,
            plan_type: Some("pro".into()),
            last_warmup_at: None,
            error: None,
            usage: Some(
                parse_usage(&json!({"five_hour":{"utilization":0},"seven_day":{"utilization":5}}))
                    .unwrap(),
            ),
        };
        p.usage.as_mut().unwrap().checked_at = now;
        assert!(warmup_due(&p, now));
        assert!(!warmup_due(&p, now + 61));
        p.last_warmup_at = Some(now);
        assert!(!warmup_due(&p, now));
        p.last_warmup_at = None;
        p.usage.as_mut().unwrap().five_hour_resets_at = Some(
            chrono::DateTime::from_timestamp(now + 3600, 0)
                .unwrap()
                .to_rfc3339(),
        );
        assert!(!warmup_due(&p, now));
        p.usage.as_mut().unwrap().five_hour_resets_at = None;
        p.usage.as_mut().unwrap().seven_day_used = 100.;
        assert!(!warmup_due(&p, now));
    }

    #[test]
    fn runner_hook_markers_distinguish_new_prompts_without_storing_prompt_text() {
        let marker = runner_hook_marker(&json!({"hook_event_name":"UserPromptSubmit","prompt":"private user text"})).unwrap().unwrap();
        assert_eq!(marker, json!({"kind":"prompt"}));
        assert!(runner_hook_marker(&json!({"hook_event_name":"StopFailure","error":"other"})).unwrap().is_none());
        let session = uuid::Uuid::new_v4().to_string();
        assert_eq!(runner_hook_marker(&json!({"hook_event_name":"StopFailure","error":"rate_limit","session_id":session})).unwrap(), Some(json!({"kind":"rate_limit","session_id":session})));
    }

    #[test]
    fn unknown_native_plan_and_saved_wait_state_fail_closed() {
        assert_eq!(
            verified_plan(&json!({"subscriptionType":"Pro"})).as_deref(),
            Some("pro")
        );
        assert_eq!(verified_plan(&json!({"loggedIn":true})), None);
        assert!(!paid_plan(
            verified_plan(&json!({"subscriptionType":"free"})).as_deref()
        ));
        let saved: ClaudeProfiles =
            serde_json::from_value(json!({"profiles":[],"waiting_for_quota":true})).unwrap();
        assert!(!saved.waiting_for_quota);
    }

    #[test]
    fn only_fresh_verified_accounts_with_quota_are_eligible() {
        let mut p = ClaudeProfile {
            id: "b".into(),
            email: "b@example.com".into(),
            is_default: false,
            account_uuid: None,
            desktop_status: String::new(),
            logged_in: true,
            plan_type: Some("pro".into()),
            last_warmup_at: None,
            error: None,
            usage: Some(
                parse_usage(
                    &json!({"five_hour":{"utilization":10},"seven_day":{"utilization":25}}),
                )
                .unwrap(),
            ),
        };
        let now = chrono::Utc::now().timestamp();
        let tried = HashSet::new();
        assert!(eligible(&p, &tried, now));
        for plan in [None, Some("free"), Some("unknown")] {
            p.plan_type = plan.map(str::to_owned);
            assert!(!eligible(&p, &tried, now));
        }
        for plan in ["pro", "max", "team", "enterprise"] {
            p.plan_type = Some(plan.into());
            assert!(eligible(&p, &tried, now));
        }
        assert!(!eligible(&p, &tried, now + 61));
        p.usage.as_mut().unwrap().seven_day_used = 100.0;
        assert!(!eligible(&p, &tried, now));
        p.usage = None;
        assert!(!eligible(&p, &tried, now));
        assert!(parse_usage(
            &json!({"five_hour":{"utilization":-1},"seven_day":{"utilization":20}})
        )
        .is_err());
        assert_eq!(
            extract_auth_url("Open https://claude.ai/oauth/authorize?state=example"),
            Some("https://claude.ai/oauth/authorize?state=example".into())
        );
        assert!(extract_auth_url("https://evil.example/oauth/authorize").is_none());
        assert_eq!(
            extract_auth_url("Open https://claude.com/cai/oauth/authorize?state=example"),
            Some("https://claude.com/cai/oauth/authorize?state=example".into())
        );
        assert!(extract_auth_url("https://claude.com.evil.example/cai/oauth/authorize").is_none());
    }

    #[test]
    fn credentials_refresh_before_expiry_and_model_usage_is_validated() {
        assert!(login_needs_refresh(
            &json!({"claudeAiOauth":{"expiresAt":120000}}),
            60000
        ));
        assert!(!login_needs_refresh(
            &json!({"claudeAiOauth":{"expiresAt":120001}}),
            60000
        ));
        assert!(parse_usage(
            &json!({"five_hour":{"utilization":1},"seven_day":{"utilization":2},
            "seven_day_opus":{"utilization":-1}})
        )
        .is_err());
    }

    #[test]
    fn desktop_identity_matches_an_imported_account_and_rejects_unknown_logins() {
        let dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(&dir).unwrap();
        let p = ClaudeProfile {
            id: uuid::Uuid::new_v4().to_string(),
            email: "test@example.com".into(),
            is_default: false,
            account_uuid: Some("account-b".into()),
            desktop_status: String::new(),
            logged_in: true,
            plan_type: Some("pro".into()),
            last_warmup_at: None,
            usage: None,
            error: None,
        };
        let mut store = ClaudeProfiles {
            profiles: vec![p.clone()],
            ..Default::default()
        };
        fs::write(
            dir.join("config.json"),
            r#"{"lastKnownAccountUuid":"account-b"}"#,
        )
        .unwrap();
        identify_live_desktop(&mut store, &dir).unwrap();
        assert_eq!(store.active_desktop_id.as_deref(), Some(p.id.as_str()));
        fs::write(
            dir.join("config.json"),
            r#"{"lastKnownAccountUuid":"unknown"}"#,
        )
        .unwrap();
        assert!(identify_live_desktop(&mut store, &dir).is_err());
        assert_eq!(store.active_desktop_id.as_deref(), Some(p.id.as_str()));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn desktop_switch_preserves_both_profiles_and_refuses_overwrite() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let live = root.join("live");
        let a = root.join("a");
        let b = root.join("b");
        fs::create_dir_all(&live).unwrap();
        fs::create_dir_all(&b).unwrap();
        fs::write(live.join("session"), "account-a").unwrap();
        fs::write(b.join("session"), "account-b").unwrap();
        swap_directories(&live, &a, &b).unwrap();
        assert_eq!(
            fs::read_to_string(live.join("session")).unwrap(),
            "account-b"
        );
        assert_eq!(fs::read_to_string(a.join("session")).unwrap(), "account-a");
        assert!(swap_directories(&live, &a, &b).is_err());
        swap_directories(&live, &b, &a).unwrap();
        assert_eq!(
            fs::read_to_string(live.join("session")).unwrap(),
            "account-a"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn desktop_stop_wait_ignores_tools_that_outlive_the_app() {
        for (path, blocks) in [
            ("/Applications/Claude.app/Contents/MacOS/Claude", true),
            ("/Applications/Claude.app/Contents/Frameworks/Claude Helper.app/Contents/MacOS/Claude Helper", true),
            ("/Applications/Claude.app/Contents/Frameworks/Claude Helper (Renderer).app/Contents/MacOS/Claude Helper (Renderer)", true),
            ("/Applications/Claude.app/Contents/Frameworks/Claude Helper (GPU).app/Contents/MacOS/Claude Helper (GPU)", true),
            ("/Users/me/Applications/Claude.app/Contents/MacOS/Claude", true),
            ("/Applications/Claude.app/Contents/Helpers/chrome-native-host", false),
            ("/Applications/Claude.app/Contents/Frameworks/Electron Framework.framework/Helpers/chrome_crashpad_handler", false),
            ("/Applications/Claude.app/Contents/Resources/claude-code/claude", false),
            ("/Applications/Cursor.app/Contents/MacOS/Cursor", false),
            ("/Applications/NotClaude.app/Contents/MacOS/Claude", false),
            ("/Applications/Claude.app/Contents/MacOS/Claude Helper", false),
            ("/Applications/Claude.app/Contents/Frameworks/Claude Helper.app/Contents/MacOS/other", false),
            ("/Applications/Claude.app/Contents/Frameworks/Claude Helper.app/Contents/MacOS/Claude Helper/x", false),
            ("/tmp/x /Applications/Claude.app/Contents/MacOS/Claude", true),
            ("node /Applications/Claude.app/Contents/MacOS/Claude", false),
            ("", false),
        ] {
            assert_eq!(writes_desktop_profile(path), blocks, "{path}");
        }
    }

    #[test]
    fn failed_desktop_switch_reopens_only_an_intact_profile() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let live = root.join("live");
        let journal = root.join("desktop-transaction.json");
        fs::create_dir_all(&live).unwrap();
        let run = |result: Result<()>, quit: bool| {
            let mut opened = false;
            let error = reopen_after_failure(result, quit, &live, &journal, || {
                opened = true;
                bail!("launch failed")
            })
            .err()
            .map(|e| e.to_string());
            (opened, error)
        };
        assert_eq!(
            run(Err(anyhow::anyhow!("original")), true),
            (true, Some("original".into()))
        );
        assert_eq!(run(Ok(()), true), (false, None));
        assert_eq!(
            run(Err(anyhow::anyhow!("original")), false),
            (false, Some("original".into()))
        );
        fs::write(&journal, "{}").unwrap();
        assert_eq!(
            run(Err(anyhow::anyhow!("original")), true),
            (false, Some("original".into()))
        );
        fs::remove_file(&journal).unwrap();
        fs::remove_dir(&live).unwrap();
        assert_eq!(
            run(Err(anyhow::anyhow!("original")), true),
            (false, Some("original".into()))
        );
        fs::remove_dir_all(root).unwrap();
    }
}
