use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::theme::Theme;

pub fn render_empty_state(frame: &mut Frame<'_>, area: Rect, message: &str, theme: Theme) {
    frame.render_widget(Paragraph::new(message).style(theme.footer_meta), area);
}

pub fn scaled_bar(value: i64, max: i64, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if max <= 0 || value <= 0 {
        return " ".repeat(width);
    }
    let filled = ((value as f64 / max as f64) * width as f64)
        .round()
        .clamp(1.0, width as f64) as usize;
    let mut output = String::new();
    output.push_str(&"█".repeat(filled));
    output.push_str(&" ".repeat(width.saturating_sub(filled)));
    output
}

pub fn sparkline_text(values: &[i64]) -> String {
    if values.is_empty() {
        return "-".to_owned();
    }
    const LEVELS: &[char; 8] = &['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let min = values.iter().copied().min().unwrap_or(0);
    let max = values.iter().copied().max().unwrap_or(0);
    if min == max {
        return "▅".repeat(values.len());
    }
    values
        .iter()
        .map(|value| {
            let ratio = (*value - min) as f64 / (max - min) as f64;
            let index = (ratio * (LEVELS.len() - 1) as f64).round() as usize;
            LEVELS[index.min(LEVELS.len() - 1)]
        })
        .collect::<String>()
}

pub fn format_minor_compact(value: i64) -> String {
    let sign = if value < 0 { "-" } else { "" };
    let abs = value.abs() as f64 / 100.0;
    if abs >= 1_000_000.0 {
        format!("{sign}{:.1}m", abs / 1_000_000.0)
    } else if abs >= 1_000.0 {
        format!("{sign}{:.1}k", abs / 1_000.0)
    } else {
        format!("{sign}{abs:.0}")
    }
}

pub fn truncate_text(value: &str, max_len: usize) -> String {
    if value.chars().count() <= max_len {
        return value.to_owned();
    }
    if max_len <= 3 {
        return value.chars().take(max_len).collect();
    }
    let mut output = value.chars().take(max_len - 3).collect::<String>();
    output.push_str("...");
    output
}

pub fn label_value_line(label: &str, value: String, theme: Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled(label.to_owned(), theme.section_heading),
        Span::styled(value, theme.body),
    ])
}

pub fn proportional_widths(values: &[i64], width: usize) -> Vec<usize> {
    if values.is_empty() {
        return Vec::new();
    }
    if width == 0 {
        return vec![0; values.len()];
    }

    let positives = values
        .iter()
        .map(|value| (*value).max(0) as f64)
        .collect::<Vec<_>>();
    let total = positives.iter().sum::<f64>();
    if total <= 0.0 {
        return vec![0; values.len()];
    }

    let mut widths = vec![0usize; values.len()];
    let mut remainders = Vec::new();
    let mut used = 0usize;

    for (index, value) in positives.iter().enumerate() {
        if *value <= 0.0 {
            continue;
        }
        let raw = (*value / total) * width as f64;
        let base = raw.floor() as usize;
        widths[index] = base;
        used += base;
        remainders.push((index, raw - base as f64, *value));
    }

    remainders.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                right
                    .2
                    .partial_cmp(&left.2)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| left.0.cmp(&right.0))
    });

    let mut remaining = width.saturating_sub(used);
    for (index, _, _) in remainders {
        if remaining == 0 {
            break;
        }
        widths[index] += 1;
        remaining -= 1;
    }

    widths
}

#[cfg(test)]
mod tests {
    use super::proportional_widths;

    #[test]
    fn proportional_widths_matches_requested_width() {
        let widths = proportional_widths(&[500, 300, 200], 20);
        assert_eq!(widths.iter().sum::<usize>(), 20);
        assert!(widths[0] >= widths[1]);
        assert!(widths[1] >= widths[2]);
    }

    #[test]
    fn proportional_widths_handles_zero_width_and_values() {
        assert_eq!(proportional_widths(&[1, 2, 3], 0), vec![0, 0, 0]);
        assert_eq!(proportional_widths(&[0, 0, 0], 10), vec![0, 0, 0]);
    }

    #[test]
    fn proportional_widths_assigns_remainder_to_larger_values() {
        let widths = proportional_widths(&[9, 1], 3);
        assert_eq!(widths, vec![3, 0]);
    }
}
