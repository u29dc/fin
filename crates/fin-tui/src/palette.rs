use ratatui::style::Color;

use crate::routes::Route;

pub const ACCENT_1: Color = Color::Cyan;
pub const ACCENT_2: Color = Color::Rgb(140, 214, 232);
pub const ACCENT_3: Color = Color::Rgb(92, 176, 214);
pub const ACCENT_4: Color = Color::Rgb(68, 138, 190);
pub const TEXT_PRIMARY: Color = Color::Gray;
pub const TEXT_MUTED: Color = Color::DarkGray;
pub const BORDER_SUBTLE: Color = Color::DarkGray;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteSection {
    Context,
    Navigate,
    Global,
}

impl PaletteSection {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Context => "Context",
            Self::Navigate => "Navigate",
            Self::Global => "Global",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteActionKind {
    Navigate(Route),
    Refresh,
    SetCashflowGroup(String),
    SetOverviewScopeAll,
    SetOverviewScopeGroup(String),
    Quit,
}

#[derive(Debug, Clone)]
pub struct PaletteAction {
    pub title: String,
    pub context: String,
    pub section: PaletteSection,
    pub kind: PaletteActionKind,
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct PaletteState {
    pub open: bool,
    pub query: String,
    pub selected: usize,
}

#[derive(Debug, Clone)]
pub enum PaletteRow {
    Section(PaletteSection),
    Separator,
    Action(usize),
}

pub fn filtered_action_indices(actions: &[PaletteAction], query: &str) -> Vec<usize> {
    let needle = query.trim().to_ascii_lowercase();
    if needle.is_empty() {
        return (0..actions.len()).collect();
    }

    let mut matches = Vec::new();
    for (index, action) in actions.iter().enumerate() {
        let title_match = action.title.to_ascii_lowercase().contains(&needle);
        let context_match = action.context.to_ascii_lowercase().contains(&needle);
        let keyword_match = action
            .keywords
            .iter()
            .any(|keyword| keyword.to_ascii_lowercase().contains(&needle));
        if title_match || context_match || keyword_match {
            matches.push(index);
        }
    }
    matches
}

pub fn build_rows(actions: &[PaletteAction], filtered_indices: &[usize]) -> Vec<PaletteRow> {
    let mut context = Vec::new();
    let mut navigate = Vec::new();
    let mut global = Vec::new();

    for &index in filtered_indices {
        match actions[index].section {
            PaletteSection::Context => context.push(index),
            PaletteSection::Navigate => navigate.push(index),
            PaletteSection::Global => global.push(index),
        }
    }

    let mut rows = Vec::new();
    append_section_rows(&mut rows, PaletteSection::Context, &context);
    append_section_rows(&mut rows, PaletteSection::Navigate, &navigate);
    append_section_rows(&mut rows, PaletteSection::Global, &global);
    rows
}

fn append_section_rows(rows: &mut Vec<PaletteRow>, section: PaletteSection, entries: &[usize]) {
    if entries.is_empty() {
        return;
    }

    if !rows.is_empty() {
        rows.push(PaletteRow::Separator);
    }

    rows.push(PaletteRow::Section(section));
    rows.extend(entries.iter().map(|index| PaletteRow::Action(*index)));
}
