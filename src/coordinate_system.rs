// Single source of truth for ALL coordinate conversions
use crate::{App, AppMode, ActivePane};

pub struct CoordinateSystem<'a> {
    app: &'a App,
    term_width: u16,
    term_height: u16,
}

/// All the coordinate info we need for a click
pub struct ClickCoordinates {
    pub pane: Pane,
    pub pane_relative: (usize, usize),
    pub document: (usize, usize),
    pub grid: (usize, usize),
}

impl<'a> CoordinateSystem<'a> {
    pub fn new(app: &'a App, term_width: u16, term_height: u16) -> Self {
        Self { app, term_width, term_height }
    }

    /// THE function that handles all click coordinate conversion
    pub fn process_click(&self, screen_x: u16, screen_y: u16) -> Option<ClickCoordinates> {
        let pane = self.which_pane(screen_x)?;

        // Get pane-relative coordinates
        let pane_start_x = self.get_pane_start_x(pane)?;

        // Clamp to pane boundaries - don't allow negative or out-of-bounds
        let pane_x = if screen_x >= pane_start_x {
            (screen_x - pane_start_x) as usize
        } else {
            0 // Clamp to left edge of pane
        };
        let pane_y = screen_y as usize; // Already 0-based from kitty

        // Get viewport offset for this pane
        let (viewport_x, viewport_y) = self.get_viewport_offset(pane)?;

        // Calculate document position
        let doc_x = pane_x + viewport_x;
        let doc_y = pane_y + viewport_y;

        Some(ClickCoordinates {
            pane,
            pane_relative: (pane_x, pane_y),
            document: (doc_x, doc_y),
            grid: (doc_x, doc_y), // For grid-based cursor, these are the same
        })
    }

    /// Convert screen coordinates to document coordinates
    pub fn screen_to_document(&self, screen_x: u16, screen_y: u16) -> Option<(usize, usize)> {
        let pane = self.which_pane(screen_x)?;
        let pane_coords = self.screen_to_pane(screen_x, screen_y, pane)?;
        self.pane_to_document(pane_coords.0, pane_coords.1, pane)
    }

    /// Determine which pane a screen coordinate is in
    pub fn which_pane(&self, screen_x: u16) -> Option<Pane> {
        if self.app.app_mode == AppMode::NotesEditor {
            let notes_list_width = 4;
            let remaining = self.term_width.saturating_sub(notes_list_width);
            let notes_editor_width = remaining / 2;
            let extraction_start = notes_list_width + notes_editor_width;

            // Explicit boundary handling to prevent edge cases
            if screen_x < notes_list_width {
                Some(Pane::NotesList)
            } else if screen_x < extraction_start {
                Some(Pane::NotesEditor)
            } else {
                Some(Pane::Extraction)
            }
        } else {
            let split = self.app.split_position.unwrap_or(self.term_width / 2);
            // Divider is at split, PDF is before, extraction is after
            if screen_x < split {
                Some(Pane::Pdf)
            } else if screen_x == split {
                None // Click is on divider itself, not in a pane
            } else {
                Some(Pane::Extraction)
            }
        }
    }

    /// Convert screen to pane-relative coordinates
    pub fn screen_to_pane(&self, x: u16, y: u16, pane: Pane) -> Option<(usize, usize)> {
        let pane_start_x = self.get_pane_start_x(pane)?;
        // Clamp to pane boundaries
        let pane_x = if x >= pane_start_x {
            (x - pane_start_x) as usize
        } else {
            0
        };
        Some((
            pane_x,
            y as usize  // y is already 0-based from kitty
        ))
    }

    /// Convert pane coordinates to document coordinates (accounting for viewport)
    pub fn pane_to_document(&self, pane_x: usize, pane_y: usize, pane: Pane) -> Option<(usize, usize)> {
        let (viewport_x, viewport_y) = self.get_viewport_offset(pane)?;
        // Prevent any potential underflow - document coords should always be valid
        let doc_x = pane_x.saturating_add(viewport_x);
        let doc_y = pane_y.saturating_add(viewport_y);
        Some((doc_x, doc_y))
    }

    fn get_pane_start_x(&self, pane: Pane) -> Option<u16> {
        match pane {
            Pane::NotesList => Some(0),
            Pane::NotesEditor => Some(4),
            Pane::Extraction if self.app.app_mode == AppMode::NotesEditor => {
                let remaining = self.term_width.saturating_sub(4);
                Some(4 + remaining / 2)
            }
            Pane::Extraction => {
                // Extraction pane starts after the divider column in PDF mode
                let split = self.app.split_position.unwrap_or(self.term_width / 2);
                Some(split + 1)
            }
            Pane::Pdf => Some(0),
        }
    }

    fn get_viewport_offset(&self, pane: Pane) -> Option<(usize, usize)> {
        match pane {
            Pane::NotesEditor => {
                self.app.notes_display.as_ref()
                    .map(|r| (r.viewport_x, r.viewport_y))
            }
            Pane::Extraction => {
                self.app.edit_display.as_ref()
                    .map(|r| (r.viewport_x, r.viewport_y))
            }
            _ => Some((0, 0))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pane {
    NotesList,
    NotesEditor,
    Extraction,
    Pdf,
}