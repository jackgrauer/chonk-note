// Coordinate validation that catches issues IMMEDIATELY
use crate::App;

#[derive(Debug, Clone)]
pub struct CoordinateIssue {
    pub issue_type: IssueType,
    pub expected: (usize, usize),
    pub actual: (usize, usize),
    pub context: String,
}

#[derive(Debug, Clone)]
pub enum IssueType {
    CursorMismatch,
    ViewportDrift,
    ClickPositionWrong,
    SelectionCorrupted,
}

pub struct CoordinateValidator {
    last_click: Option<(u16, u16)>,
    last_cursor: Option<(usize, usize)>,
    issues: Vec<CoordinateIssue>,
}

impl CoordinateValidator {
    pub fn new() -> Self {
        Self {
            last_click: None,
            last_cursor: None,
            issues: Vec::new(),
        }
    }

    /// Validate that cursor moved to where we clicked
    pub fn validate_click_to_cursor(&mut self,
        click_x: u16,
        click_y: u16,
        cursor_row: usize,
        cursor_col: usize,
        viewport_x: usize,
        viewport_y: usize,
    ) -> bool {
        // Calculate where cursor SHOULD be based on click
        let expected_row = click_y as usize + viewport_y;
        let expected_col = click_x as usize + viewport_x;

        if cursor_row != expected_row || cursor_col != expected_col {
            self.issues.push(CoordinateIssue {
                issue_type: IssueType::CursorMismatch,
                expected: (expected_row, expected_col),
                actual: (cursor_row, cursor_col),
                context: format!("Click({},{}) + Viewport({},{}) should put cursor at ({},{}), but it's at ({},{})",
                    click_x, click_y, viewport_x, viewport_y,
                    expected_row, expected_col,
                    cursor_row, cursor_col),
            });

            // Print immediately to stderr so we see it
            eprintln!("❌ COORDINATE BUG: {}", self.issues.last().unwrap().context);
            false
        } else {
            true
        }
    }

    /// Validate viewport hasn't drifted
    pub fn validate_viewport_consistency(&mut self,
        old_viewport: (usize, usize),
        new_viewport: (usize, usize),
        scroll_amount: (i32, i32),
    ) -> bool {
        let expected_x = (old_viewport.0 as i32 + scroll_amount.0).max(0) as usize;
        let expected_y = (old_viewport.1 as i32 + scroll_amount.1).max(0) as usize;

        if new_viewport != (expected_x, expected_y) {
            self.issues.push(CoordinateIssue {
                issue_type: IssueType::ViewportDrift,
                expected: (expected_x, expected_y),
                actual: new_viewport,
                context: format!("Viewport drift: scrolled ({},{}) from ({},{}) but got ({},{}) instead of ({},{})",
                    scroll_amount.0, scroll_amount.1,
                    old_viewport.0, old_viewport.1,
                    new_viewport.0, new_viewport.1,
                    expected_x, expected_y),
            });

            eprintln!("❌ VIEWPORT BUG: {}", self.issues.last().unwrap().context);
            false
        } else {
            true
        }
    }

    pub fn report(&self) {
        if self.issues.is_empty() {
            eprintln!("✅ All coordinate validations passed!");
        } else {
            eprintln!("❌ Found {} coordinate issues:", self.issues.len());
            for issue in &self.issues {
                eprintln!("  - {:?}: {}", issue.issue_type, issue.context);
            }
        }
    }
}

/// Add this to App to run validation automatically
pub fn auto_validate(app: &mut App, validator: &mut CoordinateValidator) {
    // This runs after every mouse event
    if let Some((x, y)) = app.debug_info.as_ref().map(|d| d.mouse_screen) {
        let (cursor_row, cursor_col) = match app.active_pane {
            crate::ActivePane::Left => (app.notes_cursor.row, app.notes_cursor.col),
            crate::ActivePane::Right => (app.extraction_cursor.row, app.extraction_cursor.col),
        };

        let (viewport_x, viewport_y) = if app.active_pane == crate::ActivePane::Left {
            app.notes_display.as_ref()
                .map(|r| (r.viewport_x, r.viewport_y))
                .unwrap_or((0, 0))
        } else {
            app.edit_display.as_ref()
                .map(|r| (r.viewport_x, r.viewport_y))
                .unwrap_or((0, 0))
        };

        // Validate after click
        if validator.last_click != Some((x, y)) {
            validator.validate_click_to_cursor(
                x, y, cursor_row, cursor_col, viewport_x, viewport_y
            );
            validator.last_click = Some((x, y));
        }
    }
}