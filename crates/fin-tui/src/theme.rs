use ratatui::style::{Color, Modifier, Style};

use crate::palette::{ACCENT_1, BORDER_SUBTLE, TEXT_MUTED, TEXT_PRIMARY};

#[derive(Debug, Clone, Copy)]
pub struct HeaderContract {
    pub project_name: &'static str,
    pub version: &'static str,
    pub glyph: &'static str,
}

impl HeaderContract {
    pub fn render(self) -> String {
        format!("{} {} v{}", self.glyph, self.project_name, self.version)
    }
}

pub const HEADER_CONTRACT: HeaderContract = HeaderContract {
    project_name: "fin",
    version: env!("CARGO_PKG_VERSION"),
    glyph: "■",
};

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub root: Style,
    pub border: Style,
    pub brand: Style,
    pub header_meta: Style,
    pub section_heading: Style,
    pub tabs: Style,
    pub tabs_active: Style,
    pub selected: Style,
    pub body: Style,
    pub footer_meta: Style,
    pub footer_status: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            root: Style::default().fg(TEXT_PRIMARY),
            border: Style::default().fg(BORDER_SUBTLE),
            brand: Style::default().fg(ACCENT_1).add_modifier(Modifier::BOLD),
            header_meta: Style::default().fg(TEXT_MUTED),
            section_heading: Style::default().fg(ACCENT_1).add_modifier(Modifier::BOLD),
            tabs: Style::default().fg(TEXT_PRIMARY),
            tabs_active: Style::default()
                .fg(Color::Rgb(255, 255, 255))
                .bg(ACCENT_1)
                .add_modifier(Modifier::BOLD),
            selected: Style::default()
                .fg(Color::Rgb(255, 255, 255))
                .bg(ACCENT_1)
                .add_modifier(Modifier::BOLD),
            body: Style::default().fg(TEXT_PRIMARY),
            footer_meta: Style::default().fg(TEXT_MUTED),
            footer_status: Style::default().fg(TEXT_MUTED),
        }
    }
}
