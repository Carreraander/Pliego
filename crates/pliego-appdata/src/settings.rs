//! Ajustes: un objeto JSON cuyo esquema (con versión) define la interfaz. Aquí solo se guarda y se recupera con
//! seguridad: escritura atómica y, si el archivo está dañado, se aparta como `settings.corrupt.json` y se parte de cero.

use std::path::{Path, PathBuf};

use serde_json::Value;

pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(dir: &Path) -> Self {
        Self {
            path: dir.join("settings.json"),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Los ajustes guardados, o `None` si no hay (primer arranque) o el archivo no era un objeto JSON válido.
    pub fn load(&self) -> Option<Value> {
        let text = std::fs::read_to_string(&self.path).ok()?;
        match serde_json::from_str::<Value>(&text) {
            Ok(v) if v.is_object() => Some(v),
            _ => {
                let _ = std::fs::rename(&self.path, self.path.with_file_name("settings.corrupt.json"));
                None
            }
        }
    }

    /// Guarda el objeto de ajustes. Rechaza lo que no sea un objeto.
    pub fn save(&self, value: &Value) -> Result<(), String> {
        if !value.is_object() {
            return Err("Los ajustes deben ser un objeto JSON".into());
        }
        let text = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
        crate::write_atomic(&self.path, &text).map_err(|e| format!("No se pudieron guardar los ajustes: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn first_run_has_no_settings_then_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path());
        assert_eq!(store.load(), None);
        let v = json!({ "version": 1, "general": { "theme": "dark" } });
        store.save(&v).unwrap();
        assert_eq!(store.load(), Some(v));
        assert!(!dir.path().join("settings.json.tmp").exists());
    }

    #[test]
    fn corrupt_file_is_set_aside() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path());
        std::fs::write(store.path(), "{ no es json").unwrap();
        assert_eq!(store.load(), None);
        assert!(dir.path().join("settings.corrupt.json").exists());
        assert!(store.save(&json!([1])).is_err());
    }
}
