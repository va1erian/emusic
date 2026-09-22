//! Where association data lives in the registry.
//!
//! All paths are relative to `HKEY_CURRENT_USER` (registration is per-user,
//! no admin rights required).

/// The three registry subtrees [`AssocManager`](super::AssocManager) reads
/// and writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssocRoots {
    /// Mirrors `Software\Classes`: ProgIDs, `.ext` keys and
    /// `Applications\<exe>` live here.
    pub classes: String,
    /// Mirrors `Software\<app name>`: holds the `Capabilities` subkey.
    pub app: String,
    /// Mirrors `Software\RegisteredApplications`.
    pub registered_apps: String,
}

impl AssocRoots {
    /// The real, production registry locations under `HKCU`.
    pub fn production(app_name: &str) -> Self {
        Self {
            classes: r"Software\Classes".to_string(),
            app: format!(r"Software\{app_name}"),
            registered_apps: r"Software\RegisteredApplications".to_string(),
        }
    }

    /// Roots confined to a single throwaway subtree
    /// (`Software\<namespace>\...`), so callers — namely tests — never
    /// touch the real, machine-wide associations.
    pub fn under_namespace(namespace: &str, app_name: &str) -> Self {
        Self {
            classes: format!(r"Software\{namespace}\Classes"),
            app: format!(r"Software\{namespace}\{app_name}"),
            registered_apps: format!(r"Software\{namespace}\RegisteredApplications"),
        }
    }
}
