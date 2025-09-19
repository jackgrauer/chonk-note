// macOS Services Integration
// Enables system-wide text services in chonker7
use anyhow::Result;
use std::process::Command;
use std::fs;
use std::path::PathBuf;

pub struct ServicesIntegration {
    service_name: String,
    bundle_id: String,
    temp_dir: PathBuf,
}

impl ServicesIntegration {
    pub fn new() -> Self {
        Self {
            service_name: "Chonker7".to_string(),
            bundle_id: "com.chonker7.app".to_string(),
            temp_dir: PathBuf::from("/tmp/chonker7_services"),
        }
    }

    // Register service handlers with macOS
    pub fn register_services(&self) -> Result<()> {
        // Create temp directory for service exchange
        fs::create_dir_all(&self.temp_dir)?;

        // Register text manipulation services
        self.register_text_service("Transform Selection", "transform")?;
        self.register_text_service("Search in Chonker7", "search")?;
        self.register_text_service("Copy as Markdown", "markdown")?;

        Ok(())
    }

    // Register a specific text service
    fn register_text_service(&self, display_name: &str, action: &str) -> Result<()> {
        // Services are typically registered via Info.plist in an app bundle
        // For terminal apps, we can use NSUserDefaults to register dynamically
        let script = format!(
            r#"
            tell application "System Events"
                -- Register service for text handling
                -- This would normally be done via Info.plist
                log "Registering service: {}"
            end tell
            "#,
            display_name
        );

        Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()?;

        Ok(())
    }

    // Handle incoming service requests
    pub fn handle_service_request(&self, action: &str, text: &str) -> Result<String> {
        match action {
            "transform" => self.transform_text(text),
            "search" => self.search_text(text),
            "markdown" => self.convert_to_markdown(text),
            _ => Ok(text.to_string()),
        }
    }

    // Transform selected text (uppercase, lowercase, capitalize)
    fn transform_text(&self, text: &str) -> Result<String> {
        // Could show a menu for transformation options
        // For now, just cycle through transformations
        if text == text.to_uppercase() {
            Ok(text.to_lowercase())
        } else if text == text.to_lowercase() {
            Ok(self.capitalize_words(text))
        } else {
            Ok(text.to_uppercase())
        }
    }

    fn capitalize_words(&self, text: &str) -> String {
        text.split_whitespace()
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    // Search for text in current document
    fn search_text(&self, query: &str) -> Result<String> {
        // This would integrate with the app's search functionality
        Ok(format!("Searching for: {}", query))
    }

    // Convert selection to markdown
    fn convert_to_markdown(&self, text: &str) -> Result<String> {
        // Basic markdown conversion
        let lines: Vec<&str> = text.lines().collect();
        let mut result = String::new();

        for line in lines {
            if line.starts_with("# ") {
                result.push_str(line);
            } else if !line.is_empty() {
                result.push_str(&format!("**{}**", line));
            }
            result.push('\n');
        }

        Ok(result)
    }

}

// Integration with App
impl crate::App {
    pub fn enable_macos_services(&mut self) -> Result<()> {
        let services = ServicesIntegration::new();
        services.register_services()?;

        // Store services handler if needed
        // self.services_handler = Some(services);

        Ok(())
    }

    pub fn handle_service_action(&mut self, action: &str) -> Result<()> {
        let services = ServicesIntegration::new();

        // Get selected text
        let selected_text = self.get_selected_text();

        // Process through service
        let result = services.handle_service_request(action, &selected_text)?;

        // Replace selection with result
        if result != selected_text {
            self.replace_selection(&result)?;
        }

        Ok(())
    }

    pub fn get_selected_text(&self) -> String {
        let range = self.selection.primary();
        let text = self.rope.slice(range.from()..range.to()).to_string();
        text
    }

    pub fn replace_selection(&mut self, new_text: &str) -> Result<()> {
        use helix_core::{Transaction, history::State};

        let range = self.selection.primary();
        let transaction = Transaction::change(
            &self.rope,
            vec![(range.from(), range.to(), Some(new_text.into()))].into_iter()
        );

        // Save state for undo
        let state = State {
            doc: self.rope.clone(),
            selection: self.selection.clone(),
        };

        if transaction.apply(&mut self.rope) {
            self.history.commit_revision(&transaction, &state);
            self.selection = self.selection.clone().map(transaction.changes());
            self.needs_redraw = true;

            if let Some(renderer) = &mut self.edit_display {
                renderer.update_from_rope(&self.rope);
            }
        }

        Ok(())
    }
}