//! Cursor auth is swapped in place so project settings and local chat history stay intact.
use anyhow::{bail, Context, Result};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

const AUTH_KEYS: &[&str] = &[
    "cursorAuth/accessToken",
    "cursorAuth/refreshToken",
    "cursorAuth/stripeMembershipType",
    "cursorAuth/stripeMembershipAuthId",
    "cursorAuth/stripeSubscriptionStatus",
    "cursorAuth/cachedEmail",
    "cursorAuth/cachedSignUpType",
    "cursorAuth/cachedScopedProfile",
    "cursorAuth/cachedTeam",
];
type Credentials = BTreeMap<String, String>;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CursorUsage {
    pub period: String,
    pub used_percent: Option<f64>,
    pub reset_at: Option<String>,
    pub checked_at: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CursorProfile {
    pub id: String,
    pub email: String,
    pub plan_name: Option<String>,
    pub is_merged: bool,
    pub usage: Option<CursorUsage>,
    pub error: Option<String>,
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct CursorProfiles {
    pub profiles: Vec<CursorProfile>,
    pub active_desktop_id: Option<String>,
}
#[derive(Clone, Default, Serialize)]
pub struct CursorLogin {
    pub flow_id: String,
    pub auth_url: Option<String>,
    pub status: String,
    pub error: Option<String>,
}
static LOGIN: LazyLock<Mutex<Option<CursorLogin>>> = LazyLock::new(|| Mutex::new(None));
#[derive(Default, Serialize, Deserialize)]
struct Probe {
    retry_at: i64,
    profile: Option<CursorProfile>,
}
fn root() -> Result<PathBuf> {
    Ok(crate::auth::get_config_dir()?.join("cursor"))
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
fn write_json(path: &Path, data: &impl Serialize) -> Result<()> {
    private_dir(path.parent().context("Missing parent directory")?)?;
    let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(&temp)?;
    f.write_all(&serde_json::to_vec(data)?)?;
    f.sync_all()?;
    fs::rename(temp, path)?;
    Ok(())
}
fn lock_store() -> Result<File> {
    private_dir(&root()?)?;
    let f = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(root()?.join("profiles.lock"))?;
    f.lock()?;
    Ok(f)
}
fn load() -> Result<CursorProfiles> {
    let p = root()?.join("profiles.json");
    if !p.exists() {
        return Ok(CursorProfiles::default());
    }
    Ok(serde_json::from_slice(&fs::read(p)?)?)
}
fn save(s: &CursorProfiles) -> Result<()> {
    write_json(&root()?.join("profiles.json"), s)
}
fn account_path(id: &str) -> Result<PathBuf> {
    uuid::Uuid::parse_str(id).context("Invalid Cursor profile ID")?;
    Ok(root()?.join(id))
}
fn credentials(id: &str) -> Result<Credentials> {
    Ok(serde_json::from_slice(&fs::read(
        account_path(id)?.join("credentials.json"),
    )?)?)
}
fn db_path() -> Result<PathBuf> {
    #[cfg(target_os = "macos")]
    let base = dirs::home_dir()
        .context("Home directory is unavailable")?
        .join("Library/Application Support");
    #[cfg(target_os = "windows")]
    let base = dirs::config_dir().context("Configuration directory is unavailable")?;
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let base = dirs::config_dir().context("Configuration directory is unavailable")?;
    Ok(base.join("Cursor/User/globalStorage/state.vscdb"))
}
fn read_auth(db: &Connection) -> Result<Credentials> {
    let mut out = Credentials::new();
    for key in AUTH_KEYS {
        let value = db.query_row("SELECT value FROM ItemTable WHERE key = ?1", [key], |r| {
            r.get::<_, String>(0)
        });
        match value {
            Ok(v) => {
                out.insert((*key).to_owned(), v);
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(out)
}
fn personal_scope(value: Option<&Value>) -> Result<()> {
    if value
        .is_some_and(|v| !v.is_null() && v.as_str().map(|s| !s.trim().is_empty()).unwrap_or(true))
    {
        bail!("Cursor team accounts are not supported; sign in to a personal account");
    }
    Ok(())
}
fn personal_auth(auth: &Credentials) -> Result<()> {
    if let Some(team) = auth
        .get("cursorAuth/cachedTeam")
        .filter(|s| !s.trim().is_empty())
    {
        personal_scope(Some(
            &serde_json::from_str::<Value>(team).unwrap_or_else(|_| Value::String(team.clone())),
        ))?;
    }
    Ok(())
}
fn outgoing_id(profiles: &[CursorProfile], auth: &Credentials) -> Result<Option<String>> {
    if token(auth).is_err() {
        return Ok(None);
    }
    native_active_id(profiles, auth)
        .map(Some)
        .context("Import the current Cursor account before switching")
}
fn cancel_pending(login: &mut CursorLogin) {
    if login.status == "pending" {
        login.status = "cancelled".into();
    }
}
fn native_auth() -> Result<Credentials> {
    let db = Connection::open_with_flags(db_path()?, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    read_auth(&db)
}
fn native_active_id(profiles: &[CursorProfile], auth: &Credentials) -> Option<String> {
    personal_auth(auth).ok()?;
    token(auth).ok()?;
    let cached = auth.get("cursorAuth/cachedEmail")?;
    let email = serde_json::from_str::<String>(cached).unwrap_or_else(|_| cached.trim().to_owned());
    profiles
        .iter()
        .find(|p| p.email.eq_ignore_ascii_case(email.trim()))
        .map(|p| p.id.clone())
}
fn reconciled_profiles() -> Result<CursorProfiles> {
    let mut s = load()?;
    s.active_desktop_id = native_auth()
        .ok()
        .and_then(|auth| native_active_id(&s.profiles, &auth));
    Ok(s)
}
fn replace_auth(db: &mut Connection, auth: &Credentials) -> Result<()> {
    let tx = db.transaction()?;
    for key in AUTH_KEYS {
        if let Some(value) = auth.get(*key) {
            tx.execute(
                "INSERT OR REPLACE INTO ItemTable (key,value) VALUES (?1,?2)",
                [*key, value],
            )?;
        } else {
            tx.execute("DELETE FROM ItemTable WHERE key = ?1", [key])?;
        }
    }
    tx.commit()?;
    Ok(())
}
fn token(auth: &Credentials) -> Result<&str> {
    auth.get("cursorAuth/accessToken")
        .map(String::as_str)
        .filter(|v| !v.is_empty())
        .context("Cursor login is required")
}
fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?)
}
async fn api(client: &reqwest::Client, auth: &Credentials, route: &str) -> Result<Value> {
    let url = format!("https://api2.cursor.sh/{route}");
    let request = client
        .post(url)
        .header("Content-Type", "application/json")
        .header("Connect-Protocol-Version", "1")
        .json(&json!({}));
    let response = request.bearer_auth(token(auth)?).send().await?;
    check_status(response.status().as_u16())?;
    Ok(response.json().await?)
}
fn check_status(code: u16) -> Result<()> {
    match code {
        401 => bail!("Cursor login is required"),
        429 => bail!("Cursor usage checks are temporarily rate limited (HTTP 429). Try refreshing again in five minutes."),
        code if !(200..300).contains(&code) => bail!("Cursor request returned HTTP {code}"),
        _ => Ok(()),
    }
}
fn login_url(challenge: &str, id: &str) -> String {
    let mut url = url::Url::parse("https://cursor.com/loginDeepControl").expect("Static URL");
    url.query_pairs_mut()
        .append_pair("challenge", challenge)
        .append_pair("uuid", id)
        .append_pair("mode", "login")
        .append_pair("supportsSelectedTeamLogin", "false");
    url.to_string()
}
fn quota(plan: &Value, usage: &Value) -> Result<CursorUsage> {
    let info = plan.get("planInfo").unwrap_or(plan);
    let used_percent = usage
        .pointer("/planUsage/totalPercentUsed")
        .and_then(Value::as_f64);
    if used_percent.is_some_and(|v| !v.is_finite() || !(0.0..=100.0).contains(&v)) {
        bail!("Cursor usage could not be verified");
    }
    let reset_at = info
        .get("includedUsageResetsAt")
        .and_then(|v| {
            v.as_str()
                .and_then(|s| s.parse::<i64>().ok())
                .or_else(|| v.as_i64())
        })
        .and_then(chrono::DateTime::from_timestamp_millis)
        .map(|d| d.to_rfc3339());
    Ok(CursorUsage {
        period: info
            .get("includedUsagePeriod")
            .and_then(Value::as_str)
            .unwrap_or("UNKNOWN")
            .to_owned(),
        used_percent,
        reset_at,
        checked_at: chrono::Utc::now().timestamp(),
    })
}
async fn verify(id: &str, auth: &Credentials, expected: Option<&str>) -> Result<CursorProfile> {
    personal_auth(auth)?;
    let c = client()?;
    let email = api(&c, auth, "aiserver.v1.AuthService/GetEmail")
        .await?
        .get("email")
        .and_then(Value::as_str)
        .context("Cursor account email could not be verified")?
        .to_owned();
    if expected.is_some_and(|e| !e.eq_ignore_ascii_case(&email)) {
        bail!("Cursor login email does not match the requested account");
    }
    let plan = api(&c, auth, "aiserver.v1.DashboardService/GetPlanInfo").await?;
    let usage = api(
        &c,
        auth,
        "aiserver.v1.DashboardService/GetCurrentPeriodUsage",
    )
    .await
    .and_then(|usage| quota(&plan, &usage));
    let (usage, error) = match usage {
        Ok(usage) => (Some(usage), None),
        Err(e) => (None, Some(e.to_string())),
    };
    let info = plan.get("planInfo").unwrap_or(&plan);
    Ok(CursorProfile {
        id: id.to_owned(),
        email,
        plan_name: Some(
            info.get("planName")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .context("Cursor plan could not be verified")?
                .to_owned(),
        ),
        is_merged: plan
            .get("isMerged")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        usage,
        error,
    })
}
fn store_profile(mut p: CursorProfile, auth: &Credentials, active: bool) -> Result<CursorProfiles> {
    let _lock = lock_store()?;
    let mut s = load()?;
    if let Some(old) = s
        .profiles
        .iter()
        .find(|v| v.email.eq_ignore_ascii_case(&p.email))
    {
        p.id = old.id.clone();
    }
    write_json(&account_path(&p.id)?.join("credentials.json"), auth)?;
    if active {
        s.active_desktop_id = Some(p.id.clone());
    }
    if let Some(old) = s.profiles.iter_mut().find(|v| v.id == p.id) {
        *old = p;
    } else {
        s.profiles.push(p);
    }
    save(&s)?;
    Ok(s)
}
#[tauri::command]
pub fn list_cursor_profiles() -> Result<CursorProfiles, String> {
    reconciled_profiles().map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn import_current_cursor_profile() -> Result<CursorProfiles, String> {
    async {
        let db = Connection::open_with_flags(db_path()?, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let auth = read_auth(&db)?;
        drop(db);
        let p = verify(&uuid::Uuid::new_v4().to_string(), &auth, None).await?;
        store_profile(p, &auth, true)
    }
    .await
    .map_err(|e: anyhow::Error| e.to_string())
}
#[tauri::command]
pub async fn refresh_cursor_profiles() -> Result<CursorProfiles, String> {
    async {
        let s = reconciled_profiles()?;
        let native = native_auth().ok();
        for p in s.profiles {
            let failed_id = p.id.clone();
            let result: Result<()> = async {
                let path = account_path(&p.id)?;
                private_dir(&path)?;
                let gate = OpenOptions::new()
                    .create(true)
                    .truncate(false)
                    .read(true)
                    .write(true)
                    .open(path.join("usage.lock"))?;
                if gate.try_lock().is_err() {
                    return Ok(());
                }
                let probe_path = path.join("usage.json");
                let mut probe: Probe = if probe_path.exists() {
                    serde_json::from_slice(&fs::read(&probe_path)?)?
                } else {
                    Probe::default()
                };
                let now = chrono::Utc::now().timestamp();
                if probe.retry_at > now
                    || probe
                        .profile
                        .as_ref()
                        .and_then(|p| p.usage.as_ref())
                        .is_some_and(|u| now - u.checked_at >= 0 && now - u.checked_at < 60)
                {
                    return Ok(());
                }
                let stored = credentials(&p.id)?;
                let active_native = native.as_ref().filter(|auth| {
                    native_active_id(std::slice::from_ref(&p), auth).as_deref()
                        == Some(p.id.as_str())
                });
                let auth = active_native.unwrap_or(&stored);
                let result = verify(&p.id, auth, Some(&p.email)).await;
                let native_verified = result.is_ok() && active_native.is_some();
                let updated = match result {
                    Ok(v) => {
                        probe.retry_at = 0;
                        v
                    }
                    Err(e) => {
                        let mut v = p;
                        let error = e.to_string();
                        if error.contains("HTTP 429") {
                            probe.retry_at = now + 300;
                        }
                        v.usage = None;
                        v.error = Some(error);
                        v
                    }
                };
                if updated
                    .error
                    .as_ref()
                    .is_some_and(|e| e.contains("HTTP 429"))
                {
                    probe.retry_at = now + 300;
                }
                probe.profile = Some(updated.clone());
                write_json(&probe_path, &probe)?;
                let _lock = lock_store()?;
                let mut latest = load()?;
                if native_verified && token(&credentials(&updated.id)?).ok() == token(&stored).ok()
                {
                    write_json(&account_path(&updated.id)?.join("credentials.json"), auth)?;
                }
                if let Some(p) = latest.profiles.iter_mut().find(|p| p.id == updated.id) {
                    *p = updated;
                }
                save(&latest)?;
                Ok(())
            }
            .await;
            if let Err(e) = result {
                let _lock = lock_store()?;
                let mut latest = load()?;
                if let Some(p) = latest.profiles.iter_mut().find(|p| p.id == failed_id) {
                    p.error = Some(e.to_string());
                    p.usage = None;
                }
                save(&latest)?;
            }
        }
        reconciled_profiles()
    }
    .await
    .map_err(|e: anyhow::Error| e.to_string())
}
#[tauri::command]
pub async fn start_cursor_login(email: String) -> Result<CursorLogin, String> {
    let email = email.trim().to_owned();
    if email.is_empty() || !email.contains('@') {
        return Err("Enter a valid email address".to_owned());
    }
    let pkce = crate::auth::generate_pkce();
    let id = uuid::Uuid::new_v4().to_string();
    let login = CursorLogin {
        flow_id: id.clone(),
        auth_url: Some(login_url(&pkce.code_challenge, &id)),
        status: "pending".into(),
        error: None,
    };
    {
        let mut state = LOGIN
            .lock()
            .map_err(|_| "Cursor login lock is unavailable")?;
        if state.as_ref().is_some_and(|v| v.status == "pending") {
            return Err("A Cursor login is already in progress".into());
        }
        *state = Some(login.clone());
    }
    tokio::spawn(async move {
        let result: Result<()> = async {
            let c = client()?;
            let deadline = Instant::now() + Duration::from_secs(300);
            while Instant::now() < deadline {
                if !LOGIN
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Cursor login lock is unavailable"))?
                    .as_ref()
                    .is_some_and(|v| v.flow_id == id && v.status == "pending")
                {
                    return Ok(());
                }
                let response = c
                    .get("https://api2.cursor.sh/auth/poll")
                    .query(&[
                        ("uuid", id.as_str()),
                        ("verifier", pkce.code_verifier.as_str()),
                    ])
                    .timeout(
                        deadline
                            .saturating_duration_since(Instant::now())
                            .min(Duration::from_secs(15)),
                    )
                    .send()
                    .await;
                let r = match response {
                    Ok(r) => r,
                    Err(_) => {
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        continue;
                    }
                };
                if r.status().is_server_error() || r.status().as_u16() == 429 {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    continue;
                }
                if r.status().as_u16() == 404 {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }
                if !r.status().is_success() {
                    bail!("Cursor login returned HTTP {}", r.status().as_u16());
                }
                let v: Value = r
                    .json()
                    .await
                    .context("Cursor login response could not be read")?;
                personal_scope(v.get("selectedTeamId"))?;
                let mut auth = Credentials::new();
                for (field, key) in [
                    ("accessToken", "cursorAuth/accessToken"),
                    ("refreshToken", "cursorAuth/refreshToken"),
                    ("authId", "cursorAuth/stripeMembershipAuthId"),
                ] {
                    auth.insert(
                        key.into(),
                        v.get(field)
                            .and_then(Value::as_str)
                            .filter(|s| !s.is_empty())
                            .context("Cursor login response is incomplete")?
                            .into(),
                    );
                }
                let p = verify(&uuid::Uuid::new_v4().to_string(), &auth, Some(&email)).await?;
                auth.insert("cursorAuth/cachedEmail".into(), p.email.clone());
                let mut state = LOGIN
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Cursor login lock is unavailable"))?;
                if let Some(login) = state
                    .as_mut()
                    .filter(|v| v.flow_id == id && v.status == "pending")
                {
                    store_profile(p, &auth, false)?;
                    login.status = "complete".into();
                }
                return Ok(());
            }
            bail!("Cursor login timed out; generate a new login link")
        }
        .await;
        if let Err(e) = result {
            if let Ok(mut state) = LOGIN.lock() {
                if let Some(v) = state
                    .as_mut()
                    .filter(|v| v.flow_id == id && v.status == "pending")
                {
                    v.status = "error".into();
                    v.error = Some(e.to_string());
                }
            }
        }
    });
    Ok(login)
}
#[tauri::command]
pub fn get_cursor_login() -> Result<Option<CursorLogin>, String> {
    LOGIN
        .lock()
        .map(|s| s.clone())
        .map_err(|_| "Cursor login lock is unavailable".into())
}
#[tauri::command]
pub fn cancel_cursor_login() -> Result<(), String> {
    let mut state = LOGIN
        .lock()
        .map_err(|_| "Cursor login lock is unavailable")?;
    if let Some(s) = state.as_mut() {
        cancel_pending(s);
    }
    Ok(())
}
fn cursor_closed() -> Result<()> {
    #[cfg(windows)]
    let output = Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq Cursor.exe", "/FO", "CSV", "/NH"])
        .output()?;
    #[cfg(not(windows))]
    let output = Command::new("ps").args(["-axo", "comm="]).output()?;
    if !output.status.success() {
        bail!("Cursor process status could not be verified");
    }
    let output = String::from_utf8_lossy(&output.stdout);
    if output.lines().any(|s| {
        let s = s.trim().to_lowercase();
        s.contains("cursor.app/")
            || s.contains("cursor helper")
            || s == "cursor"
            || s.ends_with("/cursor")
            || s.starts_with("\"cursor.exe\"")
    }) {
        bail!("Close Cursor and its helper processes before switching accounts");
    }
    Ok(())
}
fn launch_cursor() -> Result<()> {
    #[cfg(target_os = "macos")]
    let status = Command::new("open").args(["-a", "Cursor"]).status()?;
    #[cfg(target_os = "windows")]
    let status = Command::new("cmd")
        .args(["/C", "start", "", "cursor"])
        .status()?;
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Command::new("cursor").spawn()?;
        return Ok(());
    }
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        if !status.success() {
            bail!("Cursor could not be opened");
        }
        Ok(())
    }
}
#[tauri::command]
pub fn open_cursor_desktop() -> Result<(), String> {
    launch_cursor().map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn switch_cursor_desktop(profile_id: String) -> Result<CursorProfiles, String> {
    async {
        cursor_closed()?;
        let s = load()?;
        let p = s
            .profiles
            .iter()
            .find(|p| p.id == profile_id)
            .context("Cursor account not found")?;
        let target = credentials(&p.id)?;
        let verified = verify(&p.id, &target, Some(&p.email)).await?;
        let before = native_auth()?;
        personal_auth(&before)?;
        let outgoing_account = outgoing_id(&s.profiles, &before)?;
        if let Some(id) = &outgoing_account {
            let registered = s
                .profiles
                .iter()
                .find(|p| p.id == *id)
                .context("Cursor account not found")?;
            let email = api(&client()?, &before, "aiserver.v1.AuthService/GetEmail")
                .await?
                .get("email")
                .and_then(Value::as_str)
                .context("Cursor account email could not be verified")?
                .to_owned();
            if !email.eq_ignore_ascii_case(&registered.email) {
                bail!("Import the current Cursor account before switching");
            }
        }
        let _lock = lock_store()?;
        cursor_closed()?;
        let original = load()?;
        let mut latest = original.clone();
        let mut db = Connection::open_with_flags(db_path()?, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        let outgoing = read_auth(&db)?;
        if outgoing != before {
            bail!("Cursor authentication changed; refresh accounts and try again");
        }
        if let Some(id) = &outgoing_account {
            write_json(&account_path(id)?.join("credentials.json"), &outgoing)?;
        }
        latest.active_desktop_id = Some(p.id.clone());
        if let Some(p) = latest.profiles.iter_mut().find(|p| p.id == verified.id) {
            *p = verified;
        }
        replace_auth(&mut db, &target)?;
        if let Err(e) = save(&latest) {
            replace_auth(&mut db, &outgoing).context(
                "Cursor metadata failed and outgoing authentication could not be restored",
            )?;
            return Err(e);
        }
        if let Err(e) = launch_cursor() {
            replace_auth(&mut db, &outgoing).context(
                "Cursor launch failed and outgoing authentication could not be restored",
            )?;
            save(&original)
                .context("Cursor launch failed and account metadata could not be restored")?;
            return Err(e);
        }
        Ok(latest)
    }
    .await
    .map_err(|e: anyhow::Error| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reject_team_and_unknown_outgoing_preserve_complete_login() {
        assert!(personal_scope(Some(&json!("team-id"))).is_err());
        assert!(personal_auth(&Credentials::from([(
            "cursorAuth/cachedTeam".into(),
            "{\"id\":1}".into()
        )]))
        .is_err());
        assert!(personal_scope(Some(&Value::Null)).is_ok());
        assert!(personal_scope(Some(&json!(""))).is_ok());
        assert!(outgoing_id(
            &[],
            &Credentials::from([("cursorAuth/accessToken".into(), "token".into())])
        )
        .is_err());
        assert_eq!(outgoing_id(&[], &Credentials::new()).unwrap(), None);
        let mut login = CursorLogin {
            status: "complete".into(),
            ..Default::default()
        };
        cancel_pending(&mut login);
        assert_eq!(login.status, "complete");
        login.status = "pending".into();
        cancel_pending(&mut login);
        assert_eq!(login.status, "cancelled");
    }
    #[test]
    fn login_url_preserves_pkce() {
        let u = url::Url::parse(&login_url("a+b/=", "test")).unwrap();
        let q: BTreeMap<_, _> = u.query_pairs().into_owned().collect();
        assert_eq!(q["challenge"], "a+b/=");
        assert_eq!(q["supportsSelectedTeamLogin"], "false");
        assert_eq!(u.host_str(), Some("cursor.com"));
    }
    #[test]
    fn rate_limit_does_not_request_new_login() {
        assert!(check_status(200).is_ok());
        assert!(check_status(401)
            .unwrap_err()
            .to_string()
            .contains("login is required"));
        let limited = check_status(429).unwrap_err().to_string();
        assert!(limited.contains("five minutes"));
        assert!(!limited.contains("login"));
        assert!(check_status(500)
            .unwrap_err()
            .to_string()
            .contains("HTTP 500"));
    }
    #[test]
    fn unknown_quota_is_not_invented() {
        let p = json!({"planInfo":{"includedUsagePeriod":"INCLUDED_USAGE_PERIOD_WEEKLY","includedUsageResetsAt":"1792000000000"}});
        let u = quota(&p, &json!({})).unwrap();
        assert_eq!(u.used_percent, None);
        assert_eq!(u.period, "INCLUDED_USAGE_PERIOD_WEEKLY");
        assert!(u.reset_at.is_some());
        assert_eq!(
            quota(&p, &json!({"planUsage":{"totalPercentUsed":0}}))
                .unwrap()
                .used_percent,
            Some(0.0)
        );
        assert!(quota(&p, &json!({"planUsage":{"totalPercentUsed":101}})).is_err());
    }
    #[test]
    fn active_account_matches_native_email_only_with_a_token() {
        let profiles = vec![
            CursorProfile {
                id: "first".into(),
                email: "first@example.com".into(),
                plan_name: None,
                is_merged: false,
                usage: None,
                error: None,
            },
            CursorProfile {
                id: "second".into(),
                email: "second@example.com".into(),
                plan_name: None,
                is_merged: false,
                usage: None,
                error: None,
            },
        ];
        let mut auth = Credentials::from([
            ("cursorAuth/accessToken".into(), "token".into()),
            ("cursorAuth/cachedEmail".into(), "SECOND@example.com".into()),
        ]);
        assert_eq!(
            native_active_id(&profiles, &auth).as_deref(),
            Some("second")
        );
        auth.insert(
            "cursorAuth/cachedEmail".into(),
            "unknown@example.com".into(),
        );
        assert_eq!(native_active_id(&profiles, &auth), None);
        auth.insert("cursorAuth/cachedEmail".into(), "first@example.com".into());
        auth.remove("cursorAuth/accessToken");
        assert_eq!(native_active_id(&profiles, &auth), None);
    }
    #[test]
    fn sqlite_swap_preserves_history_and_removes_missing_auth() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute(
            "CREATE TABLE ItemTable (key TEXT PRIMARY KEY,value TEXT)",
            [],
        )
        .unwrap();
        for (k, v) in [
            ("chat", "keep"),
            ("telemetry.machineId", "keep"),
            ("settings", "keep"),
            ("cursorAuth/accessToken", "old"),
            ("cursorAuth/cachedTeam", "old-team"),
        ] {
            db.execute("INSERT INTO ItemTable VALUES (?1,?2)", [k, v])
                .unwrap();
        }
        let target = Credentials::from([("cursorAuth/accessToken".into(), "new".into())]);
        replace_auth(&mut db, &target).unwrap();
        assert_eq!(read_auth(&db).unwrap(), target);
        for key in ["chat", "telemetry.machineId", "settings"] {
            assert_eq!(
                db.query_row("SELECT value FROM ItemTable WHERE key=?1", [key], |r| {
                    r.get::<_, String>(0)
                })
                .unwrap(),
                "keep"
            );
        }
    }
}
