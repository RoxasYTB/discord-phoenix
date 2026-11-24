use std::sync::Mutex;

#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

pub trait TokenStore: Send + Sync + 'static {
    fn load(&self) -> Result<Option<Secret>, String>;
    fn save(&self, token: &Secret) -> Result<(), String>;
    fn clear(&self) -> Result<(), String>;
}

pub const SERVICE: &str = "discord-phoenix";
pub const ACCOUNT: &str = "token";

#[derive(Debug, Default)]
pub struct KeyringStore;

impl KeyringStore {
    fn entry() -> Result<keyring::Entry, String> {
        keyring::Entry::new(SERVICE, ACCOUNT).map_err(|e| e.to_string())
    }
}

impl TokenStore for KeyringStore {
    fn load(&self) -> Result<Option<Secret>, String> {
        match Self::entry()?.get_password() {
            Ok(t) if !t.is_empty() => Ok(Some(Secret(t))),
            Ok(_) | Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }
    fn save(&self, token: &Secret) -> Result<(), String> {
        Self::entry()?.set_password(token.expose()).map_err(|e| e.to_string())
    }
    fn clear(&self) -> Result<(), String> {
        match Self::entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

#[derive(Debug, Default)]
pub struct MemoryStore(pub Mutex<Option<Secret>>);

impl TokenStore for MemoryStore {
    fn load(&self) -> Result<Option<Secret>, String> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn save(&self, token: &Secret) -> Result<(), String> {
        *self.0.lock().unwrap() = Some(token.clone());
        Ok(())
    }
    fn clear(&self) -> Result<(), String> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_debug_is_redacted() {
        let s = Secret::new("mfa.very-secret-token");
        assert!(!format!("{s:?}").contains("very-secret"));
    }

    #[test]
    fn memory_store_roundtrip() {
        let st = MemoryStore::default();
        assert!(st.load().unwrap().is_none());
        st.save(&Secret::new("t")).unwrap();
        assert_eq!(st.load().unwrap().unwrap().expose(), "t");
        st.clear().unwrap();
        assert!(st.load().unwrap().is_none());
    }
}
