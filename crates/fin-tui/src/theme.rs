use ratatui::style::{Modifier, Style};

use crate::palette::{CYAN_100, CYAN_300, CYAN_500, CYAN_700, SLATE_200, SLATE_950};

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
    project_name: env!("CARGO_PKG_NAME"),
    version: env!("CARGO_PKG_VERSION"),
    glyph: "■",
};

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub root: Style,
    pub border: Style,
    pub header: Style,
    pub tabs: Style,
    pub tabs_active: Style,
    pub body: Style,
    pub footer: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            root: Style::default().bg(SLATE_950).fg(SLATE_200),
            border: Style::default().fg(CYAN_700),
            header: Style::default().fg(CYAN_100).add_modifier(Modifier::BOLD),
            tabs: Style::default().fg(CYAN_300),
            tabs_active: Style::default()
                .fg(CYAN_100)
                .bg(CYAN_500)
                .add_modifier(Modifier::BOLD),
            body: Style::default().fg(SLATE_200),
            footer: Style::default().fg(CYAN_300),
        }
    }
}
