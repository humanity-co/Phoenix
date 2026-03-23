use anyhow::Result;
use std::fs;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct BugStats {
    pub total_fixed: usize,
    pub history: Vec<FixEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FixEntry {
    pub timestamp: String,
    pub file: String,
    pub error: String,
}

impl BugStats {
    pub fn load() -> Self {
        if let Ok(content) = fs::read_to_string("phoenix_stats.json") {
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save(&self) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        fs::write("phoenix_stats.json", content)?;
        
        // Also update the README counter
        let mut readme = fs::read_to_string("README.md").unwrap_or_default();
        let badge = format!("![Bugs Fixed](https://img.shields.io/badge/Bugs%20Fixed-{}-brightgreen)", self.total_fixed);
        
        if readme.contains("![Bugs Fixed]") {
            // Replace existing badge
            let start = readme.find("![Bugs Fixed]").unwrap();
            let end = readme[start..].find(')').unwrap() + start + 1;
            readme.replace_range(start..end, &badge);
        } else {
            readme.insert_str(0, &format!("{}\n\n", badge));
        }
        
        fs::write("README.md", readme)?;
        Ok(())
    }

    pub fn add_fix(&mut self, file: &str, error: &str) {
        self.total_fixed += 1;
        self.history.push(FixEntry {
            timestamp: chrono::Local::now().to_rfc3339(),
            file: file.to_string(),
            error: error.to_string(),
        });
    }
}
