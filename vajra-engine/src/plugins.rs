use std::collections::HashMap;

use extism::{Manifest, Plugin, Wasm};

pub struct PluginManager {
    plugins: HashMap<String, Plugin>,
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    pub fn load_plugin(&mut self, name: &str, wasm_bytes: &[u8]) -> Result<(), extism::Error> {
        let wasm = Wasm::data(wasm_bytes);
        let manifest = Manifest::new([wasm]);
        let plugin = Plugin::new(&manifest, [], true)?;
        self.plugins.insert(name.to_string(), plugin);
        Ok(())
    }

    pub fn extract_links(
        &mut self,
        plugin_name: &str,
        url: &str,
    ) -> Result<Vec<String>, extism::Error> {
        if let Some(plugin) = self.plugins.get_mut(plugin_name) {
            let res = plugin.call::<&str, &str>("extract", url)?;
            // Assume the plugin returns a JSON array of URLs
            let urls: Vec<String> = serde_json::from_str(res).unwrap_or_default();
            Ok(urls)
        } else {
            Err(extism::Error::msg("Plugin not found"))
        }
    }
}
