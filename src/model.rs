/// Only explicit source results close a bet. Unknown statuses stay visible.
pub fn finished(status: &str) -> bool {
    matches!(status, "won" | "lost" | "draw" | "cancelled")
}

pub fn visible(status: &str, closed: Option<i64>, now: i64, days: i32) -> bool {
    if !finished(status) {
        return true;
    }
    days > 0 && closed.is_some_and(|at| now.saturating_sub(at).max(0) < i64::from(days) * 86_400)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retention_boundary_and_zero() {
        assert!(visible("won", Some(100), 86_499, 1));
        assert!(!visible("won", Some(100), 86_500, 1));
        assert!(!visible("lost", Some(100), 100, 0));
        assert!(!visible("won", None, 100, 3));
    }

    #[test]
    fn unresolved_and_unknown_statuses_are_not_results() {
        for status in ["open", "pending", "disputed", "new-status"] {
            assert!(visible(status, Some(0), i64::MAX, 0));
        }
        assert!(visible("draw", Some(100), 90, 3));
    }
}

/// Wrap navigation safely, including an empty or shortened feed.
pub fn navigate(index: usize, count: usize, forward: bool) -> usize {
    if count == 0 {
        return 0;
    }
    let index = index.min(count - 1);
    if forward {
        (index + 1) % count
    } else {
        (index + count - 1) % count
    }
}

#[derive(Default)]
pub struct Rotation {
    pub elapsed_ms: u32,
    pub paused: bool,
}

impl Rotation {
    pub fn tick(&mut self, delta: u32, period: u32, enabled: bool) -> bool {
        if self.paused || !enabled {
            return false;
        }
        self.elapsed_ms = self.elapsed_ms.saturating_add(delta);
        if self.elapsed_ms >= period {
            self.elapsed_ms = 0;
            true
        } else {
            false
        }
    }

    pub fn reset(&mut self) {
        self.elapsed_ms = 0;
    }
}

#[cfg(test)]
mod rotation_tests {
    use super::*;
    #[test]
    fn navigation_wraps_and_handles_feed_changes() {
        assert_eq!(navigate(0, 6, false), 5);
        assert_eq!(navigate(5, 6, true), 0);
        assert_eq!(navigate(99, 2, true), 0);
        assert_eq!(navigate(0, 0, false), 0);
        assert_eq!(navigate(0, 1, true), 0);
    }
    #[test]
    fn rotation_respects_pause_details_and_manual_reset() {
        let mut timer = Rotation::default();
        assert!(!timer.tick(14_999, 15_000, true));
        assert!(timer.tick(1, 15_000, true));
        timer.paused = true;
        assert!(!timer.tick(60_000, 15_000, true));
        timer.paused = false;
        assert!(!timer.tick(60_000, 15_000, false));
        assert!(!timer.tick(10_000, 15_000, true));
        timer.reset();
        assert!(!timer.tick(5_000, 15_000, true));
    }
}

pub fn avatar_url(path: &str) -> Option<String> {
    const BASE: &str = "https://www.aox.cz/kicomtracker/";
    let relative = path.strip_prefix(BASE).unwrap_or(path);
    let filename = relative.strip_prefix("assets/avatars/")?;
    if filename.is_empty()
        || !filename
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        return None;
    }
    Some(format!("{BASE}assets/avatars/{filename}"))
}

#[cfg(test)]
mod avatar_tests {
    use super::*;
    #[test]
    fn resolves_source_avatars_and_rejects_unrelated_paths() {
        let url = "https://www.aox.cz/kicomtracker/assets/avatars/kicom.jpg";
        assert_eq!(avatar_url("assets/avatars/kicom.jpg").as_deref(), Some(url));
        assert_eq!(avatar_url(url).as_deref(), Some(url));
        for invalid in [
            "",
            "assets/avatars/",
            "assets/avatars/../secret",
            "https://example.com/a.jpg",
        ] {
            assert!(avatar_url(invalid).is_none());
        }
    }
}

/// Explicit line/row bounds: canvas autofit does not clip overflowing paragraphs.
pub fn summary_lines(value: &str, columns: usize, rows: usize) -> Vec<String> {
    if columns == 0 || rows == 0 {
        return Vec::new();
    }
    let chars: Vec<char> = value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .collect();
    let mut lines = Vec::new();
    let mut start = 0;
    while start < chars.len() && lines.len() < rows {
        let mut end = (start + columns).min(chars.len());
        if end < chars.len()
            && lines.len() + 1 < rows
            && let Some(space) = chars[start..end].iter().rposition(|c| *c == ' ')
            && space > 0
        {
            end = start + space;
        }
        if lines.len() + 1 == rows && end < chars.len() {
            end = (start + columns.saturating_sub(1)).min(chars.len());
            lines.push(format!(
                "{}…",
                chars[start..end].iter().collect::<String>().trim_end()
            ));
            break;
        }
        lines.push(chars[start..end].iter().collect::<String>());
        start = end;
        while start < chars.len() && chars[start] == ' ' {
            start += 1;
        }
    }
    lines
}

#[cfg(test)]
mod summary_tests {
    use super::*;
    #[test]
    fn long_czech_text_stays_inside_line_and_row_limits() {
        for value in [
            "Podmínky sázky: Vzorkol překoná očekávání a Kicom vyhrává.",
            "https://example.com/verylongunbrokenurl",
            "",
            "a\n b   c",
        ] {
            for columns in [1, 8, 30] {
                let lines = summary_lines(value, columns, 2);
                assert!(lines.len() <= 2);
                assert!(lines.iter().all(|s| s.chars().count() <= columns));
            }
        }
        assert_eq!(summary_lines("Ahoj světe", 20, 2), vec!["Ahoj světe"]);
        assert_eq!(summary_lines("abcdef", 3, 1), vec!["ab…"]);
    }
}
