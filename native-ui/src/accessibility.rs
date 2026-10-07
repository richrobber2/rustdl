//! Semantic state for the Android adapter; never serialize the raw AccessKit node.
use accesskit::{Action, Affine, Node, NodeId, TreeId, TreeUpdate};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

const MAX_NODES: usize = 4096;
#[derive(Serialize)]
pub(crate) struct AndroidNode {
    pub id: String,
    pub parent: Option<String>,
    pub role: String,
    pub label: Option<String>,
    pub description: Option<String>,
    pub value: Option<String>,
    pub bounds: Option<[f64; 4]>,
    pub disabled: bool,
    pub selected: Option<bool>,
    pub checked: Option<bool>,
    pub mixed: bool,
    pub actions: Vec<u8>,
    pub numeric_value: Option<f64>,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
}
fn intersect(a: [f64; 4], b: [f64; 4]) -> [f64; 4] {
    let left = a[0].max(b[0]);
    let top = a[1].max(b[1]);
    [left, top, a[2].min(b[2]).max(left), a[3].min(b[3]).max(top)]
}
fn bounded_text(text: Option<&str>) -> Option<String> {
    text.map(|text| text.chars().take(2048).collect())
}
const ACTIONS: &[(u8, Action)] = &[
    (1, Action::Click),
    (2, Action::Focus),
    (3, Action::Blur),
    (4, Action::Increment),
    (5, Action::Decrement),
    (6, Action::SetValue),
    (7, Action::ScrollIntoView),
    (8, Action::Expand),
    (9, Action::Collapse),
    (10, Action::ScrollDown),
    (11, Action::ScrollUp),
];
#[derive(Default)]
pub(crate) struct SemanticTree {
    context: Option<(u64, u8)>,
    pub(crate) nodes: BTreeMap<NodeId, Node>,
    pub(crate) root: Option<NodeId>,
    pub(crate) focus: Option<NodeId>,
}
impl SemanticTree {
    pub(crate) fn action_request(
        &self,
        context: (u64, u8),
        id: &str,
        code: u8,
        value: f64,
    ) -> Option<accesskit::ActionRequest> {
        if self.context != Some(context)
            || id.is_empty()
            || id.len() > 20
            || !id.bytes().all(|byte| byte.is_ascii_digit())
        {
            return None;
        }
        let target = NodeId(id.parse().ok()?);
        let node = self.nodes.get(&target)?;
        if node.is_disabled() {
            return None;
        }
        let projection = self.project();
        let visible = projection.iter().find(|node| node.id == id)?;
        let action = ACTIONS.iter().find(|(candidate, _)| *candidate == code)?.1;
        if matches!(
            action,
            Action::Click
                | Action::SetValue
                | Action::Increment
                | Action::Decrement
                | Action::Expand
                | Action::Collapse
        ) && !visible
            .bounds
            .is_some_and(|bounds| bounds[2] > bounds[0] && bounds[3] > bounds[1])
        {
            return None;
        }
        if !node.supports_action(action) {
            return None;
        }
        let data = if action == Action::SetValue {
            let minimum = node.min_numeric_value()?;
            let maximum = node.max_numeric_value()?;
            if !value.is_finite()
                || !minimum.is_finite()
                || !maximum.is_finite()
                || minimum > maximum
                || value < minimum
                || value > maximum
            {
                return None;
            }
            Some(accesskit::ActionData::NumericValue(value))
        } else if matches!(action, Action::ScrollDown | Action::ScrollUp) {
            Some(accesskit::ActionData::ScrollUnit(
                accesskit::ScrollUnit::Page,
            ))
        } else {
            None
        };
        Some(accesskit::ActionRequest {
            action,
            target_tree: TreeId::ROOT,
            target_node: target,
            data,
        })
    }
    /// Explicit Android fields only: no author IDs, provenance, source paths,
    /// image references or debug properties cross this presentation boundary.
    pub(crate) fn project(&self) -> Vec<AndroidNode> {
        let Some(root) = self.root else {
            return Vec::new();
        };
        let mut output = Vec::new();
        let mut pending = vec![(root, None, Affine::IDENTITY, None::<[f64; 4]>)];
        while let Some((id, parent, ancestor, clip)) = pending.pop() {
            let Some(node) = self.nodes.get(&id) else {
                continue;
            };
            if node.is_hidden() {
                continue;
            }
            let transform = ancestor * node.transform().copied().unwrap_or(Affine::IDENTITY);
            if !transform.is_finite() {
                continue;
            }
            let bounds = node
                .bounds()
                .map(|bounds| transform.transform_rect_bbox(bounds))
                .map(|bounds| [bounds.x0, bounds.y0, bounds.x1, bounds.y1])
                .filter(|bounds| bounds.iter().all(|value| value.is_finite()));
            let bounds = bounds.map(|bounds| clip.map_or(bounds, |clip| intersect(bounds, clip)));
            let child_clip = if node.clips_children() || node.role() == accesskit::Role::ScrollView
            {
                bounds.or(clip)
            } else {
                clip
            };
            output.push(AndroidNode {
                id: id.0.to_string(),
                parent: parent.map(|id: NodeId| id.0.to_string()),
                role: format!("{:?}", node.role()),
                label: bounded_text(node.label()),
                description: bounded_text(node.description()),
                value: bounded_text(node.value()),
                bounds,
                disabled: node.is_disabled(),
                selected: node.is_selected(),
                checked: node
                    .toggled()
                    .map(|state| state == accesskit::Toggled::True),
                mixed: node.toggled() == Some(accesskit::Toggled::Mixed),
                actions: ACTIONS
                    .iter()
                    .filter(|(_, action)| node.supports_action(*action))
                    .map(|(code, _)| *code)
                    .collect(),
                numeric_value: node.numeric_value().filter(|value| value.is_finite()),
                minimum: node.min_numeric_value().filter(|value| value.is_finite()),
                maximum: node.max_numeric_value().filter(|value| value.is_finite()),
            });
            pending.extend(
                node.children()
                    .iter()
                    .rev()
                    .map(|child| (*child, Some(id), transform, child_clip)),
            );
        }
        output
    }
    pub(crate) fn clear(&mut self) {
        self.nodes.clear();
        self.root = None;
        self.focus = None;
    }
    /// A new privacy/screen context needs a complete initial tree. Old nodes
    /// must never supply text or action targets for the replacement screen.
    pub(crate) fn apply(&mut self, revision: u64, screen: u8, update: TreeUpdate) -> bool {
        let context = (revision, screen);
        if self.context != Some(context) {
            self.clear();
            self.context = Some(context);
        }
        if update.tree_id != TreeId::ROOT || update.nodes.len() > MAX_NODES {
            self.clear();
            return false;
        }
        let root = update.tree.map(|tree| tree.root).or(self.root);
        let Some(root) = root else {
            return false;
        };
        let mut ids = BTreeSet::new();
        if update.nodes.iter().any(|(id, _)| !ids.insert(*id)) {
            self.clear();
            return false;
        }
        let mut nodes = self.nodes.clone();
        nodes.extend(update.nodes);
        if nodes.len() > MAX_NODES {
            self.clear();
            return false;
        }
        let mut reachable = BTreeSet::new();
        let mut pending = vec![root];
        while let Some(id) = pending.pop() {
            // Duplicate parents and cycles are invalid semantic trees.
            if !reachable.insert(id) {
                self.clear();
                return false;
            }
            let Some(node) = nodes.get(&id) else {
                self.clear();
                return false;
            };
            pending.extend(node.children().iter().copied());
        }
        if !reachable.contains(&update.focus) {
            self.clear();
            return false;
        }
        nodes.retain(|id, _| reachable.contains(id));
        self.nodes = nodes;
        self.root = Some(root);
        self.focus = Some(update.focus);
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use accesskit::{Role, Tree};
    fn update(children: &[u64], initial: bool) -> TreeUpdate {
        let mut root = Node::new(Role::Window);
        root.set_children(children.iter().copied().map(NodeId).collect::<Vec<_>>());
        let mut nodes = vec![(NodeId(1), root)];
        nodes.extend(
            children
                .iter()
                .map(|id| (NodeId(*id), Node::new(Role::Button))),
        );
        TreeUpdate {
            nodes,
            tree: initial.then(|| Tree::new(NodeId(1))),
            tree_id: TreeId::ROOT,
            focus: NodeId(1),
        }
    }
    #[test]
    fn clipped_controls_cannot_be_activated_until_visible() {
        let mut initial = update(&[2], true);
        initial.nodes[0].1.set_role(accesskit::Role::ScrollView);
        initial.nodes[0]
            .1
            .set_bounds(accesskit::Rect::new(0.0, 0.0, 100.0, 100.0));
        initial.nodes[1]
            .1
            .set_bounds(accesskit::Rect::new(0.0, 150.0, 50.0, 200.0));
        initial.nodes[1].1.add_action(Action::Click);
        let mut tree = SemanticTree::default();
        assert!(tree.apply(1, 5, initial.clone()));
        assert!(tree.action_request((1, 5), "2", 1, 0.0).is_none());
        initial.nodes[1]
            .1
            .set_bounds(accesskit::Rect::new(0.0, 20.0, 50.0, 70.0));
        assert!(tree.apply(1, 5, initial));
        assert!(tree.action_request((1, 5), "2", 1, 0.0).is_some());
    }
    #[test]
    fn semantic_scroll_actions_carry_page_units_and_clip_viewports() {
        let mut initial = update(&[2], true);
        initial.nodes[0].1.set_role(accesskit::Role::ScrollView);
        initial.nodes[0]
            .1
            .set_bounds(accesskit::Rect::new(0.0, 0.0, 100.0, 100.0));
        initial.nodes[0].1.add_action(Action::ScrollDown);
        initial.nodes[1]
            .1
            .set_bounds(accesskit::Rect::new(90.0, 90.0, 150.0, 150.0));
        let mut tree = SemanticTree::default();
        assert!(tree.apply(1, 5, initial));
        let action = tree.action_request((1, 5), "1", 10, 0.0).unwrap();
        assert_eq!(action.action, Action::ScrollDown);
        assert!(matches!(
            action.data,
            Some(accesskit::ActionData::ScrollUnit(
                accesskit::ScrollUnit::Page
            ))
        ));
        assert_eq!(tree.project()[1].bounds, Some([90.0, 90.0, 100.0, 100.0]));
    }
    #[test]
    fn android_projection_clips_descendant_touch_bounds() {
        let mut initial = update(&[2], true);
        initial.nodes[0]
            .1
            .set_bounds(accesskit::Rect::new(0.0, 0.0, 100.0, 100.0));
        initial.nodes[0].1.set_clips_children();
        initial.nodes[1]
            .1
            .set_bounds(accesskit::Rect::new(80.0, 90.0, 180.0, 190.0));
        let mut tree = SemanticTree::default();
        assert!(tree.apply(1, 0, initial));
        assert_eq!(tree.project()[1].bounds, Some([80.0, 90.0, 100.0, 100.0]));
    }
    #[test]
    fn semantic_actions_reject_stale_context_unknown_actions_and_invalid_numeric_values() {
        let mut initial = update(&[2], true);
        initial.nodes[1]
            .1
            .set_bounds(accesskit::Rect::new(0.0, 0.0, 20.0, 20.0));
        initial.nodes[1].1.add_action(Action::Click);
        initial.nodes[1].1.add_action(Action::SetValue);
        initial.nodes[1].1.set_min_numeric_value(0.0);
        initial.nodes[1].1.set_max_numeric_value(100.0);
        let mut tree = SemanticTree::default();
        assert!(tree.apply(2, 12, initial));
        assert!(tree.action_request((2, 12), "2", 1, 0.0).is_some());
        assert!(tree.action_request((2, 12), "2", 6, 50.0).is_some());
        for value in [f64::NAN, f64::INFINITY, -1.0, 101.0] {
            assert!(tree.action_request((2, 12), "2", 6, value).is_none());
        }
        assert!(tree.action_request((1, 12), "2", 1, 0.0).is_none());
        assert!(tree.action_request((2, 5), "2", 1, 0.0).is_none());
        assert!(tree.action_request((2, 12), "+2", 1, 0.0).is_none());
        assert!(tree.action_request((2, 12), "2", 99, 0.0).is_none());
        assert!(tree.action_request((2, 12), "2", 2, 0.0).is_none());
    }
    #[test]
    fn android_projection_composes_bounds_omits_hidden_nodes_and_debug_properties() {
        let mut initial = update(&[2, 3], true);
        initial.nodes[0]
            .1
            .set_transform(Affine::translate((10.0, 20.0)));
        initial.nodes[1]
            .1
            .set_bounds(accesskit::Rect::new(1.0, 2.0, 11.0, 12.0));
        initial.nodes[1].1.set_label("Synthetic control");
        initial.nodes[1]
            .1
            .set_author_id("https://example.invalid/private-path");
        initial.nodes[1].1.add_action(Action::Click);
        initial.nodes[1].1.set_selected(true);
        initial.nodes[1].1.set_toggled(accesskit::Toggled::True);
        initial.nodes[2].1.set_hidden();
        let mut tree = SemanticTree::default();
        assert!(tree.apply(1, 0, initial));
        let projected = tree.project();
        assert_eq!(projected.len(), 2);
        assert_eq!(projected[1].bounds, Some([11.0, 22.0, 21.0, 32.0]));
        assert_eq!(projected[1].actions, vec![1]);
        assert_eq!(projected[1].id, "2");
        assert_eq!(projected[1].selected, Some(true));
        assert_eq!(projected[1].checked, Some(true));
        let json = serde_json::to_string(&projected).unwrap();
        assert!(!json.contains("private-path"));
        assert!(!json.contains("author"));
    }
    #[test]
    fn semantic_updates_prune_removed_nodes_and_clear_changed_privacy_context() {
        let mut tree = SemanticTree::default();
        assert!(tree.apply(1, 5, update(&[2, 3], true)));
        assert!(tree.apply(1, 5, update(&[3], false)));
        assert!(!tree.nodes.contains_key(&NodeId(2)));
        assert!(!tree.apply(2, 5, update(&[3], false)));
        assert!(tree.nodes.is_empty());
        assert!(tree.apply(2, 5, update(&[4], true)));
        assert!(!tree.apply(2, 12, update(&[4], false)));
        assert!(tree.nodes.is_empty());
    }
    #[test]
    fn semantic_updates_reject_cycles_missing_nodes_and_oversized_trees() {
        let mut tree = SemanticTree::default();
        assert!(!tree.apply(1, 0, update(&[1], true)));
        let mut missing = update(&[2], true);
        missing.nodes.pop();
        assert!(!tree.apply(1, 0, missing));
        assert!(!tree.apply(
            1,
            0,
            update(&(2..MAX_NODES as u64 + 2).collect::<Vec<_>>(), true)
        ));
        assert!(tree.nodes.is_empty());
    }
}
