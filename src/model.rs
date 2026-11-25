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

/// A path starting with `/` is relative to Nexus; a full URL is used as it is.
pub fn avatar_url(avatar: &str) -> Option<String> {
    const NEXUS: &str = "https://nexus.braiinsforge.com";
    let url = if avatar.starts_with('/') {
        format!("{NEXUS}{avatar}")
    } else if avatar.starts_with("https://") || avatar.starts_with("http://") {
        avatar.to_owned()
    } else {
        return None;
    };
    // The file name becomes the photo's cache key.
    let file = url
        .rsplit('/')
        .next()
        .expect("BUG: rsplit yields at least one piece");
    if file.is_empty()
        || !file
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        return None;
    }
    Some(url)
}

#[cfg(test)]
mod avatar_tests {
    use super::*;
    #[test]
    fn resolves_paths_against_nexus_and_keeps_full_urls() {
        let url = "https://nexus.braiinsforge.com/api/v1/data/kicomtracker/avatar/Kicom21-jpg.jpg";
        assert_eq!(
            avatar_url("/api/v1/data/kicomtracker/avatar/Kicom21-jpg.jpg").as_deref(),
            Some(url)
        );
        assert_eq!(
            avatar_url(url).as_deref(),
            Some(url),
            "a full URL to the same photo resolves alike, so the cached photo stays"
        );
        for full in [
            "https://cdn.example.com/a/b.jpg",
            "http://example.com/b.jpg",
        ] {
            assert_eq!(avatar_url(full).as_deref(), Some(full));
        }
    }

    #[test]
    fn rejects_avatars_without_a_usable_file_name() {
        for invalid in [
            "",
            "Kicom21-jpg.jpg",
            "api/v1/data/kicomtracker/avatar/Kicom21-jpg.jpg",
            "ftp://example.com/b.jpg",
            "/api/v1/data/kicomtracker/avatar/",
            "https://example.com/b.jpg?size=480",
        ] {
            assert!(avatar_url(invalid).is_none(), "{invalid} must not resolve");
        }
    }
}
