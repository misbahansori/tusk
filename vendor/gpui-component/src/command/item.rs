use std::rc::Rc;

use gpui::{Action, AnyElement, App, IntoElement, SharedString, Window};

use crate::{Disableable, Icon};

/// A single command in a [`crate::command::Command`] palette.
///
pub struct CommandItem {
    label: Option<SharedString>,
    keywords: Vec<SharedString>,
    /// Boxed: an [`Icon`] carries a whole `StyleRefinement`, which would make
    /// every item — and so the palette's item vector — kilobytes wide.
    pub(crate) icon: Option<Box<Icon>>,
    pub(crate) action: Option<Box<dyn Action>>,
    pub(crate) checked: bool,
    disabled: bool,
    pub(crate) content: Option<Rc<CommandItemContent>>,
}

impl Clone for CommandItem {
    fn clone(&self) -> Self {
        Self {
            label: self.label.clone(),
            keywords: self.keywords.clone(),
            icon: self.icon.clone(),
            action: self.action.as_ref().map(|action| action.boxed_clone()),
            checked: self.checked,
            disabled: self.disabled,
            content: self.content.clone(),
        }
    }
}

impl CommandItem {
    /// Create an empty command item.
    pub fn new() -> Self {
        Self {
            label: None,
            keywords: Vec::new(),
            icon: None,
            action: None,
            checked: false,
            disabled: false,
            content: None,
        }
    }

    /// Set the label to display and search.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set the leading icon.
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(Box::new(icon.into()));
        self
    }

    /// Set the Action dispatched when this item is clicked or confirmed.
    ///
    /// The Action's active keybinding is also shown by the default row.
    pub fn action(mut self, action: Box<dyn Action>) -> Self {
        self.action = Some(action);
        self
    }

    /// Mark this item as the chosen one, drawing a check at the right end of
    /// the row.
    ///
    /// A resolved Action binding takes that slot, so an item with one shows no
    /// check.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Add extra terms the search matches against, besides the label.
    pub fn keywords<I, S>(mut self, keywords: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<SharedString>,
    {
        self.keywords
            .extend(keywords.into_iter().map(|keyword| keyword.into()));
        self
    }

    /// Replace the row content (icon and label) with a lazily built child.
    ///
    /// The builder may run more than once for measurement and rendering, so it
    /// must be side-effect-free. Custom children own their complete visual
    /// presentation, including any keybinding hint.
    pub fn child<F, E>(mut self, builder: F) -> Self
    where
        F: Fn(&mut Window, &mut App) -> E + 'static,
        E: IntoElement,
    {
        self.content = Some(Rc::new(move |window, cx| {
            builder(window, cx).into_any_element()
        }));
        self
    }

    /// Whether this item is non-interactive.
    pub(crate) fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// Whether this item matches the search query: the query's characters
    /// appear in the label or a keyword in order, not necessarily adjacent
    /// (`canclus` finds `candidate_cluster`). Ignores case.
    ///
    /// An empty query matches everything.
    pub(crate) fn matches(&self, query: &str) -> bool {
        self.score(query).is_some()
    }

    /// How well this item matches the query, higher is better; `None` when it
    /// does not match. An empty query scores 0 for every item. See
    /// [`fuzzy_score`].
    pub(crate) fn score(&self, query: &str) -> Option<i32> {
        if query.is_empty() {
            return Some(0);
        }

        let label = self
            .label
            .as_ref()
            .and_then(|label| fuzzy_score(label, query));
        // A keyword is a weaker hit than the label itself.
        let keyword = self
            .keywords
            .iter()
            .filter_map(|keyword| fuzzy_score(keyword, query))
            .max()
            .map(|score| score - 4);

        label.max(keyword)
    }

    pub(crate) fn label_text(&self) -> Option<&SharedString> {
        self.label.as_ref()
    }
}

impl Default for CommandItem {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) type CommandItemContent = dyn Fn(&mut Window, &mut App) -> AnyElement;

impl Disableable for CommandItem {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// A titled section of [`CommandItem`]s.
///
/// The heading is hidden while every item in the group is filtered out.
pub struct CommandGroup {
    heading: Option<SharedString>,
    pub(crate) items: Vec<CommandItem>,
}

impl Clone for CommandGroup {
    fn clone(&self) -> Self {
        Self {
            heading: self.heading.clone(),
            items: self.items.clone(),
        }
    }
}

impl CommandGroup {
    /// Create a new group without a label.
    pub fn new() -> Self {
        Self {
            heading: None,
            items: Vec::new(),
        }
    }

    /// Set the label displayed above the group's items.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.heading = Some(label.into());
        self
    }

    /// Add an item to the group.
    pub fn item(mut self, item: CommandItem) -> Self {
        self.items.push(item);
        self
    }

    /// Add multiple items to the group.
    pub fn items(mut self, items: impl IntoIterator<Item = CommandItem>) -> Self {
        self.items.extend(items);
        self
    }

    /// The heading of the group, when it has one.
    pub fn heading(&self) -> Option<&SharedString> {
        self.heading.as_ref()
    }
}

/// A top-level entry in a [`crate::command::Command`].
pub enum CommandEntry {
    /// A single ungrouped item.
    Item(CommandItem),
    /// A titled group of items.
    Group(CommandGroup),
    /// A divider between groups.
    ///
    /// A separator that ends up leading, trailing, or next to another
    /// separator once the query has filtered the list is not rendered.
    Separator,
}

impl Clone for CommandEntry {
    fn clone(&self) -> Self {
        match self {
            Self::Item(item) => Self::Item(item.clone()),
            Self::Group(group) => Self::Group(group.clone()),
            Self::Separator => Self::Separator,
        }
    }
}

impl From<CommandItem> for CommandEntry {
    fn from(item: CommandItem) -> Self {
        Self::Item(item)
    }
}

impl From<CommandGroup> for CommandEntry {
    fn from(group: CommandGroup) -> Self {
        Self::Group(group)
    }
}

const MATCH: i32 = 1;
const CONSECUTIVE: i32 = 6;
const WORD_START: i32 = 8;
const TEXT_START: i32 = 10;
const CAMEL_START: i32 = 6;
const MAX_GAP_PENALTY: i32 = 5;
const MAX_LEAD_PENALTY: i32 = 3;

/// Score `query` as an in-order subsequence of `text` (as VS Code's palette
/// does), or `None` when its characters do not all appear in order.
///
/// Whitespace in the query is ignored, so `app quit` finds `app: quit`.
/// Matches that are adjacent, or that start a word (after a separator such as
/// `_`, `.`, `:` or a space, or at a lower-to-upper camel-case step), score
/// higher; skipped characters cost a little. The best alignment is chosen,
/// not the first one found.
fn fuzzy_score(text: &str, query: &str) -> Option<i32> {
    let text: Vec<char> = text.chars().collect();
    let lower: Vec<char> = text
        .iter()
        .map(|c| c.to_lowercase().next().unwrap_or(*c))
        .collect();
    let query: Vec<char> = query
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| c.to_lowercase().next().unwrap_or(c))
        .collect();

    if query.is_empty() {
        return Some(0);
    }
    if query.len() > text.len() {
        return None;
    }

    let boundary = |i: usize| -> i32 {
        if i == 0 {
            return TEXT_START;
        }
        let (prev, cur) = (text[i - 1], text[i]);
        if !prev.is_alphanumeric() {
            WORD_START
        } else if prev.is_lowercase() && cur.is_uppercase() {
            CAMEL_START
        } else {
            0
        }
    };

    // best[i]: the best score with the previous query character matched at
    // text[i]. Rebuilt for each query character.
    let mut best: Vec<Option<i32>> = (0..text.len())
        .map(|i| {
            (lower[i] == query[0])
                .then(|| MATCH + boundary(i) - (i as i32).min(MAX_LEAD_PENALTY))
        })
        .collect();

    for &qc in &query[1..] {
        let mut next: Vec<Option<i32>> = vec![None; text.len()];
        for i in 1..text.len() {
            if lower[i] != qc {
                continue;
            }
            next[i] = (0..i)
                .filter_map(|k| {
                    best[k].map(|prev| {
                        let link = if k + 1 == i {
                            CONSECUTIVE
                        } else {
                            -((i - k - 1) as i32).min(MAX_GAP_PENALTY)
                        };
                        prev + link
                    })
                })
                .max()
                .map(|score| score + MATCH + boundary(i));
        }
        best = next;
    }

    best.into_iter().flatten().max()
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, rc::Rc};

    use gpui::{TestAppContext, actions, div};

    use super::*;

    actions!(command_item_test, [CloneAction]);

    #[gpui::test]
    fn cloned_entries_keep_actions_and_lazy_children_usable(cx: &mut TestAppContext) {
        let action_count = Rc::new(Cell::new(0));
        let child_count = Rc::new(Cell::new(0));
        let action_count_for_handler = action_count.clone();
        cx.update(|cx| {
            cx.on_action(move |_: &CloneAction, _| {
                action_count_for_handler.set(action_count_for_handler.get() + 1);
            });
        });

        let child_count_for_builder = child_count.clone();
        let entry = CommandEntry::Group(
            CommandGroup::new().label("Group").item(
                CommandItem::new()
                    .label("cloneable")
                    .action(Box::new(CloneAction))
                    .child(move |_, _| {
                        child_count_for_builder.set(child_count_for_builder.get() + 1);
                        div()
                    }),
            ),
        );
        let cloned = entry.clone();
        let CommandEntry::Group(group) = cloned else {
            panic!("the cloned entry should remain a group");
        };
        let cloned_item = group.items.into_iter().next().unwrap();

        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let child = cloned_item.content.as_ref().unwrap().clone();
            _ = child(window, cx);
            window.dispatch_action(cloned_item.action.as_ref().unwrap().boxed_clone(), cx);
        });

        assert_eq!(child_count.get(), 1);
        assert_eq!(action_count.get(), 1);
    }

    #[test]
    fn label_is_optional_for_custom_content() {
        assert_eq!(CommandItem::new().label_text(), None);
        assert_eq!(
            CommandItem::new().label("Calendar").label_text(),
            Some(&"Calendar".into())
        );
    }

    #[test]
    fn matches_label_and_keywords() {
        let item = CommandItem::new()
            .label("Profile")
            .keywords(["account", "user"]);

        assert!(item.matches(""));
        assert!(item.matches("PRO"));
        assert!(item.matches("Account"));
        assert!(!item.matches("billing"));
    }

    #[test]
    fn matches_in_order_subsequence() {
        let item = CommandItem::new().label("candidate_cluster");

        assert!(item.matches("canclus"));
        assert!(item.matches("CC"));
        assert!(item.matches("cand clus"));
        assert!(!item.matches("clucan"));
        assert!(!item.matches("candidatex"));
    }

    #[test]
    fn ranks_tighter_matches_first() {
        let score = |label: &str| CommandItem::new().label(label).score("canclus");

        assert!(score("candidate_cluster") > score("scan_cluster_status_all"));
        assert!(score("cancel_cluster") > score("c_a_n_x_x_x_x_c_x_l_x_u_x_s"));
        assert_eq!(score("nothing"), None);
    }

    #[test]
    fn word_starts_beat_scattered_hits() {
        let score = |label: &str| CommandItem::new().label(label).score("sq");

        assert!(score("app: sql") > score("disqualify"));
        assert!(score("show query") > score("disqualify"));
    }
}
