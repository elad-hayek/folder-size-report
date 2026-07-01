use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::config::EntryKind;

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub struct TopEntry {
    pub size_bytes: u64,
    pub kind: EntryKind,
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct TopN {
    limit: usize,
    heap: BinaryHeap<Reverse<TopEntry>>,
}

impl TopN {
    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            heap: BinaryHeap::new(),
        }
    }

    pub fn push(&mut self, entry: TopEntry) {
        if self.limit == 0 {
            return;
        }
        self.heap.push(Reverse(entry));
        if self.heap.len() > self.limit {
            self.heap.pop();
        }
    }

    pub fn sorted_desc(&self) -> Vec<TopEntry> {
        let mut entries: Vec<_> = self.heap.iter().map(|entry| entry.0.clone()).collect();
        entries.sort_by(|a, b| {
            b.size_bytes
                .cmp(&a.size_bytes)
                .then_with(|| a.path.cmp(&b.path))
                .then_with(|| a.kind.cmp(&b.kind))
        });
        entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_largest_entries() {
        let mut top = TopN::new(2);
        top.push(TopEntry {
            size_bytes: 10,
            kind: EntryKind::File,
            path: "a".into(),
        });
        top.push(TopEntry {
            size_bytes: 30,
            kind: EntryKind::File,
            path: "b".into(),
        });
        top.push(TopEntry {
            size_bytes: 20,
            kind: EntryKind::Directory,
            path: "c".into(),
        });
        let values = top.sorted_desc();
        assert_eq!(values.iter().map(|e| e.size_bytes).collect::<Vec<_>>(), vec![30, 20]);
    }
}
