pub(crate) fn transfer_progress(downloaded: u64, total: u64) -> String {
    if total == 0 {
        return format!("{} MiB saved", downloaded / 1_048_576);
    }
    let percent = (u128::from(downloaded) * 100 / u128::from(total)).min(100);
    format!(
        "{percent}% · {} of {} MiB",
        downloaded / 1_048_576,
        total / 1_048_576
    )
}

#[cfg(test)]
mod tests {
    use super::transfer_progress;

    #[test]
    fn unknown_total_does_not_invent_a_percentage() {
        assert_eq!(transfer_progress(3 * 1_048_576, 0), "3 MiB saved");
    }

    #[test]
    fn percentage_cannot_overflow_or_exceed_completion() {
        assert!(transfer_progress(u64::MAX, u64::MAX).starts_with("100%"));
        assert!(transfer_progress(u64::MAX, 1).starts_with("100%"));
        assert!(transfer_progress(0, u64::MAX).starts_with("0%"));
        assert_eq!(transfer_progress(1_048_576, 2_097_152), "50% · 1 of 2 MiB");
    }
}

/// Reject invalid or completed resume records, including non-finite bridge values.
pub(crate) fn playback_progress(position: f64, duration: f64) -> Option<String> {
    if !position.is_finite()
        || !duration.is_finite()
        || position < 5.0
        || duration <= 0.0
        || position >= duration - 5.0
    {
        return None;
    }
    let percent = (position / duration * 100.0).clamp(0.0, 100.0) as u32;
    Some(format!("Continue watching · {percent}%"))
}

#[cfg(test)]
mod playback_tests {
    use super::playback_progress;
    #[test]
    fn resume_progress_rejects_invalid_and_completed_records() {
        for (position, duration) in [
            (f64::NAN, 100.0),
            (10.0, f64::INFINITY),
            (-1.0, 100.0),
            (4.0, 100.0),
            (96.0, 100.0),
            (10.0, 0.0),
        ] {
            assert!(playback_progress(position, duration).is_none());
        }
        assert_eq!(
            playback_progress(25.0, 100.0).as_deref(),
            Some("Continue watching · 25%")
        );
    }
}

pub fn player_shortcut(
    key: &str,
    modified: bool,
    held: bool,
    focused: bool,
) -> Option<(&'static str, f64)> {
    if modified || focused {
        return None;
    }
    match key {
        "space" if !held => Some(("toggle", 0.)),
        "left" => Some(("seek-by", -10.)),
        "right" => Some(("seek-by", 10.)),
        "n" if !held => Some(("next", 0.)),
        _ => None,
    }
}

#[cfg(test)]
mod keyboard_tests {
    use super::player_shortcut;
    #[test]
    fn player_keys_keep_modified_and_unrelated_shortcuts_available() {
        assert_eq!(
            player_shortcut("space", false, false, false),
            Some(("toggle", 0.))
        );
        assert_eq!(
            player_shortcut("left", false, true, false),
            Some(("seek-by", -10.))
        );
        assert_eq!(
            player_shortcut("right", false, false, false),
            Some(("seek-by", 10.))
        );
        assert_eq!(
            player_shortcut("n", false, false, false),
            Some(("next", 0.))
        );
        for key in ["space", "left", "right", "n"] {
            assert_eq!(player_shortcut(key, true, false, false), None);
        }
        assert_eq!(player_shortcut("space", false, true, false), None);
        assert_eq!(player_shortcut("n", false, true, false), None);
        assert_eq!(player_shortcut("enter", false, false, false), None);
        for key in ["space", "left", "right", "n"] {
            assert_eq!(player_shortcut(key, false, false, true), None);
        }
    }
}
