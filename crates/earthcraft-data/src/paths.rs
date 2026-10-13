use std::path::PathBuf;
pub(crate) fn cache_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("earthcraft_tiles")
}

pub(crate) fn user_home() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}
/// Default output folder: `~/<parent>/EarthCraft` (e.g. Pictures/EarthCraft).
pub(crate) fn default_out_dir(parent: &str) -> String {
    let home = user_home();
    let base = if home.join(parent).is_dir() {
        home.join(parent)
    } else {
        home
    };
    base.join("EarthCraft").to_string_lossy().into_owned()
}
