//! Public release rows, retained independently from per-frame element creation.
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    Version(&'static str),
    Note(&'static str),
    MoreNotes { version: usize, remaining: usize },
    Older,
}

pub fn rows(
    releases: &'static [(&'static str, &'static [&'static str])],
    release_limit: usize,
    note_limits: &BTreeMap<usize, usize>,
) -> Vec<Row> {
    let mut rows = Vec::new();
    for (index, (version, notes)) in releases.iter().take(release_limit).enumerate() {
        rows.push(Row::Version(version));
        let limit = note_limits.get(&index).copied().unwrap_or(8);
        rows.extend(notes.iter().take(limit).map(|note| Row::Note(note)));
        if limit < notes.len() {
            rows.push(Row::MoreNotes {
                version: index,
                remaining: notes.len() - limit,
            });
        }
    }
    if release_limit < releases.len() {
        rows.push(Row::Older);
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    static NOTES: &[&str] = &["1", "2", "3", "4", "5", "6", "7", "8", "9", "10"];
    static RELEASES: &[(&str, &[&str])] = &[("new", NOTES), ("old", &["previous"])];

    #[test]
    fn expansion_preserves_all_note_order_and_older_release_access() {
        let mut limits = BTreeMap::new();
        let first = rows(RELEASES, 1, &limits);
        assert_eq!(
            first
                .iter()
                .filter(|row| matches!(row, Row::Note(_)))
                .count(),
            8
        );
        assert_eq!(
            first[first.len() - 2],
            Row::MoreNotes {
                version: 0,
                remaining: 2
            }
        );
        assert_eq!(first.last(), Some(&Row::Older));
        limits.insert(0, 16);
        let expanded = rows(RELEASES, 2, &limits);
        let notes: Vec<_> = expanded
            .iter()
            .filter_map(|row| match row {
                Row::Note(note) => Some(*note),
                _ => None,
            })
            .collect();
        assert_eq!(
            notes,
            NOTES
                .iter()
                .copied()
                .chain(["previous"])
                .collect::<Vec<_>>()
        );
        assert!(
            !expanded
                .iter()
                .any(|row| matches!(row, Row::MoreNotes { .. } | Row::Older))
        );
    }
}
