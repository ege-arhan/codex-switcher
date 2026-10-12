use std::{
    fs,
    io::Write,
    sync::{
        atomic::{AtomicBool, Ordering},
        LazyLock, Mutex,
    },
};
use tauri::Emitter;

static TURKISH: LazyLock<AtomicBool> = LazyLock::new(|| {
    let tr = crate::auth::get_config_dir()
        .ok()
        .and_then(|dir| fs::read_to_string(dir.join("ui-language")).ok())
        .map_or(true, |language| language.trim() != "en");
    AtomicBool::new(tr)
});
static SAVE_LOCK: Mutex<()> = Mutex::new(());

pub fn text<'a>(english: &'a str, turkish: &'a str) -> &'a str {
    if TURKISH.load(Ordering::Relaxed) {
        turkish
    } else {
        english
    }
}

fn valid_language(language: &str) -> Result<&str, &'static str> {
    match language {
        "tr" | "en" => Ok(language),
        _ => Err("Unsupported interface language"),
    }
}

#[tauri::command]
pub fn get_ui_language() -> &'static str {
    text("en", "tr")
}

#[tauri::command]
pub fn set_ui_language(app: tauri::AppHandle, language: String) -> Result<(), String> {
    valid_language(&language).map_err(str::to_owned)?;
    let _guard = SAVE_LOCK
        .lock()
        .map_err(|_| "Interface language settings unavailable")?;
    let dir = crate::auth::get_config_dir().map_err(|e| e.to_string())?;
    let temp = dir.join(format!("ui-language-{}.tmp", uuid::Uuid::new_v4()));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temp).map_err(|e| e.to_string())?;
    file.write_all(language.as_bytes())
        .map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    fs::rename(temp, dir.join("ui-language")).map_err(|e| e.to_string())?;
    TURKISH.store(language == "tr", Ordering::Relaxed);
    #[cfg(desktop)]
    {
        crate::app_menu::refresh(&app).map_err(|e| e.to_string())?;
        crate::tray::refresh_menu(&app);
    }
    app.emit("language-changed", language)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn language_setting_accepts_only_supported_languages() {
        assert_eq!(valid_language("tr").unwrap(), "tr");
        assert_eq!(valid_language("en").unwrap(), "en");
        assert!(valid_language("de").is_err());
        assert!(valid_language("tr/../../").is_err());
    }
}
