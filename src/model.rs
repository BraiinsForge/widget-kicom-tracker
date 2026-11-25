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
