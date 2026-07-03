//! Resolve content-hashed asset URLs from Vite's build manifest, so templates
//! reference the current hashed filenames (automatic cache-busting).

use std::collections::HashMap;

use serde::Deserialize;

const MANIFEST: &str = "web/dist/.vite/manifest.json";
const ENTRY: &str = "src/offline.ts";

#[derive(Clone)]
pub struct Assets {
    pub offline_js: String,
    pub offline_css: String,
}

#[derive(Deserialize)]
struct ManifestEntry {
    file: String,
    #[serde(default)]
    css: Vec<String>,
}

impl Assets {
    /// Resolve once at startup (fails with a clear message if the frontend hasn't
    /// been built). Used as the cached value and the fallback for `current`.
    pub fn load() -> anyhow::Result<Self> {
        Self::resolve()
    }

    /// The current hashed assets. In DEBUG builds this re-reads the manifest on
    /// every call, so a `pnpm build` is picked up without restarting the server
    /// (the dev-loop wrinkle); in RELEASE it returns the values cached at startup.
    /// Falls back to the cached values if a re-read momentarily fails.
    pub fn current(&self) -> Self {
        #[cfg(debug_assertions)]
        if let Ok(fresh) = Self::resolve() {
            return fresh;
        }
        self.clone()
    }

    fn resolve() -> anyhow::Result<Self> {
        let raw = std::fs::read_to_string(MANIFEST).map_err(|e| {
            anyhow::anyhow!("{MANIFEST} not found ({e}) -- build the frontend: `cd web && pnpm build`")
        })?;
        let map: HashMap<String, ManifestEntry> = serde_json::from_str(&raw)?;
        let entry = map
            .get(ENTRY)
            .ok_or_else(|| anyhow::anyhow!("`{ENTRY}` missing from Vite manifest"))?;
        Ok(Assets {
            offline_js: format!("/{}", entry.file),
            offline_css: entry
                .css
                .first()
                .map(|c| format!("/{c}"))
                .unwrap_or_default(),
        })
    }
}
