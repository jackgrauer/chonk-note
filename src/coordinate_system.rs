// Single source of truth for ALL coordinate conversions
use crate::{App, AppMode, ActivePane};

pub struct CoordinateSystem<'a> {
    app: &'a App,
    term_width: u16,
    term_height: u16,
}

impl<'a> CoordinateSystem<'a> {
    pub fn new(app: &'a App, term_width: u16, term_height: u16) -> Self {
        Self { app, term_width, term_height }
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

            if screen_x <= notes_list_width {
                Some(Pane::NotesList)
            } else if screen_x < extraction_start {
                Some(Pane::NotesEditor)
            } else {
                Some(Pane::Extraction)
            }
        } else {
            let split = self.app.split_position.unwrap_or(self.term_width / 2);
            if screen_x <= split {
                Some(Pane::Pdf)
            } else {
                Some(Pane::Extraction)
            }
        }
    }

    /// Convert screen to pane-relative coordinates
    pub fn screen_to_pane(&self, x: u16, y: u16, pane: Pane) -> Option<(usize, usize)> {
        let pane_start_x = self.get_pane_start_x(pane)?;
        Some((
            x.saturating_sub(pane_start_x) as usize,
            y as usize  // y is already 0-based from kitty
        ))
    }

    /// Convert pane coordinates to document coordinates (accounting for viewport)
    pub fn pane_to_document(&self, pane_x: usize, pane_y: usize, pane: Pane) -> Option<(usize, usize)> {
        let (viewport_x, viewport_y) = self.get_viewport_offset(pane)?;
        Some((
            pane_x + viewport_x,
            pane_y + viewport_y
        ))
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
                Some(self.app.split_position.unwrap_or(self.term_width / 2))
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