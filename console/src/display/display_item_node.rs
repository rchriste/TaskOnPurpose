use std::{cmp::Ordering, fmt::Display};

use crate::{
    base_data::item::Item,
    node::{Filter, item_node::ItemNode},
};

use super::{
    display_item::DisplayItem,
    tree_renderer::{ReversedTreeNode, TreeNodeWithDepth, TreeRenderer},
};

#[derive(Clone, Copy)]
pub(crate) enum DisplayFormat {
    MultiLineTree,
    MultiLineTreeReversed,
    SingleLine,
}

pub struct DisplayItemNode<'s> {
    item_node: &'s ItemNode<'s>,
    filter: Filter,
    display_format: DisplayFormat,
}

impl Display for DisplayItemNode<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let display_item = DisplayItem::new(self.item_node.get_item());

        let parents = self.item_node.create_parent_chain(self.filter);
        match self.display_format {
            DisplayFormat::MultiLineTree => {
                // Standard format: print item first, then parents below
                write!(
                    f,
                    "{}",
                    ItemWithPersonCheck {
                        item_node: self.item_node,
                        display_item: &display_item,
                    }
                )?;

                if !parents.is_empty() {
                    // Extract depths for continuation detection
                    let depths: Vec<usize> = parents.iter().map(|(d, _)| *d as usize).collect();

                    // Create tree nodes
                    let tree_nodes: Vec<_> = parents
                        .iter()
                        .enumerate()
                        .map(|(idx, (depth, item))| {
                            TreeNodeWithDepth::new(
                                *depth as usize,
                                DisplayItem::new(item),
                                idx,
                                &depths,
                            )
                        })
                        .collect();

                    writeln!(f)?;
                    let renderer = TreeRenderer::new(&tree_nodes, DisplayFormat::MultiLineTree);
                    renderer.render(f)?;
                }
            }
            DisplayFormat::MultiLineTreeReversed => {
                // Reversed format: print root parent first, descend to the actual item at bottom
                if parents.is_empty() {
                    // No parents, just print the item
                    write!(
                        f,
                        "{}",
                        ItemWithPersonCheck {
                            item_node: self.item_node,
                            display_item: &display_item,
                        }
                    )?;
                } else {
                    // Reverse the parent order
                    let mut reversed_parents: Vec<_> = parents.iter().collect();
                    reversed_parents.reverse();

                    // Create reversed tree nodes
                    let tree_nodes: Vec<_> = reversed_parents
                        .iter()
                        .enumerate()
                        .map(|(idx, (_depth, item))| {
                            ReversedTreeNode::new(
                                idx,
                                DisplayItem::new(item),
                                reversed_parents.len() + 1,
                            )
                        })
                        .collect();

                    let renderer =
                        TreeRenderer::new(&tree_nodes, DisplayFormat::MultiLineTreeReversed);
                    renderer.render(f)?;

                    // Now print the actual item at the bottom
                    writeln!(f)?;
                    let final_node = ReversedTreeNode::new(
                        parents.len(),
                        ItemWithPersonCheck {
                            item_node: self.item_node,
                            display_item: &display_item,
                        },
                        parents.len() + 1,
                    );
                    write!(f, "{}", final_node)?;
                }
            }
            DisplayFormat::SingleLine => {
                // Single line format: print item first, then parents inline
                write!(
                    f,
                    "{}",
                    ItemWithPersonCheck {
                        item_node: self.item_node,
                        display_item: &display_item,
                    }
                )?;
                let mut last_depth = 0;
                let mut visited = Vec::new();
                for (depth, item) in parents.iter() {
                    if visited.contains(item) {
                        // reload symbol
                        write!(f, "↺")?;
                        continue;
                    } else {
                        visited.push(item);
                    }

                    let display_item = DisplayItem::new(item);

                    if last_depth < *depth {
                        write!(f, " ⬅ {}", display_item)?;
                    } else {
                        write!(f, " // {}", display_item)?;
                    }
                    last_depth = *depth;
                }
            }
        }
        Ok(())
    }
}

/// Helper struct for rendering an item with person/group check
struct ItemWithPersonCheck<'a> {
    item_node: &'a ItemNode<'a>,
    display_item: &'a DisplayItem<'a>,
}

impl<'a> Display for ItemWithPersonCheck<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.item_node.is_person_or_group() {
            write!(f, "Is {} around?", self.display_item)
        } else {
            write!(f, "{} ", self.display_item)
        }
    }
}

impl<'s> DisplayItemNode<'s> {
    pub(crate) fn new(
        item_node: &'s ItemNode<'s>,
        filter: Filter,
        display_format: DisplayFormat,
    ) -> Self {
        DisplayItemNode {
            item_node,
            filter,
            display_format,
        }
    }

    pub(crate) fn make_list(
        item_nodes: &'s [ItemNode<'s>],
        filter: Filter,
        display_format: DisplayFormat,
    ) -> Vec<DisplayItemNode<'s>> {
        item_nodes
            .iter()
            .map(|x| DisplayItemNode::new(x, filter, display_format))
            .collect()
    }

    pub(crate) fn get_item_node(&self) -> &'s ItemNode<'s> {
        self.item_node
    }

    pub(crate) fn is_type_motivation(&self) -> bool {
        self.item_node.is_type_motivation()
    }

    pub(crate) fn is_type_goal(&self) -> bool {
        self.item_node.is_type_goal()
    }

    pub(crate) fn get_created(&self) -> &chrono::DateTime<chrono::Utc> {
        self.item_node.get_created()
    }

    pub(crate) fn get_item(&self) -> &Item<'s> {
        self.item_node.get_item()
    }
}

pub(crate) trait DisplayItemNodeSortExt<'s> {
    fn sort_motivations_first_by_summary_then_created(&mut self);
    fn sort_newest_first(&mut self);
}

impl<'s> DisplayItemNodeSortExt<'s> for [DisplayItemNode<'s>] {
    fn sort_motivations_first_by_summary_then_created(&mut self) {
        self.sort_by(|a, b| compare_motivation_first(a, b));
    }

    fn sort_newest_first(&mut self) {
        self.sort_by(|a, b| a.get_created().cmp(b.get_created()).reverse());
    }
}

fn compare_motivation_first(a: &DisplayItemNode<'_>, b: &DisplayItemNode<'_>) -> Ordering {
    let motivation_goal_ordering = if a.is_type_motivation() && !b.is_type_motivation() {
        Ordering::Less
    } else if !a.is_type_motivation() && b.is_type_motivation() {
        Ordering::Greater
    } else if a.is_type_goal() && !b.is_type_goal() {
        Ordering::Less
    } else if !a.is_type_goal() && b.is_type_goal() {
        Ordering::Greater
    } else {
        Ordering::Equal
    };

    motivation_goal_ordering
        .then_with(|| a.get_item().get_summary().cmp(b.get_item().get_summary()))
        .then_with(|| a.get_created().cmp(b.get_created()).reverse())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use crate::{
        calculated_data::parent_lookup::ParentLookup,
        data_storage::surrealdb_layer::{
            surreal_item::{
                SurrealItemBuilder, SurrealItemType, SurrealMotivationKind, SurrealOrderedSubItem,
            },
            surreal_tables::SurrealTablesBuilder,
        },
        node::Filter,
    };

    use super::{DisplayFormat, DisplayItemNode};

    #[test]
    fn multiline_tree_format_displays_item_first_then_parent() {
        // Create a parent with a child - parent has child in smaller_items list
        let surreal_items = vec![
            SurrealItemBuilder::default()
                .id(Some(("surreal_item", "parent").into()))
                .summary("Parent Item")
                .item_type(SurrealItemType::Motivation(SurrealMotivationKind::CoreWork))
                .smaller_items_in_priority_order(vec![SurrealOrderedSubItem::SubItem {
                    surreal_item_id: ("surreal_item", "child").into(),
                }])
                .build()
                .unwrap(),
            SurrealItemBuilder::default()
                .id(Some(("surreal_item", "child").into()))
                .summary("Child Item")
                .item_type(SurrealItemType::Action)
                .smaller_items_in_priority_order(vec![SurrealOrderedSubItem::SubItem {
                    surreal_item_id: ("surreal_item", "grandchild").into(),
                }])
                .build()
                .unwrap(),
            SurrealItemBuilder::default()
                .id(Some(("surreal_item", "grandchild").into()))
                .summary("Grandchild Item")
                .item_type(SurrealItemType::Action)
                .build()
                .unwrap(),
        ];

        let surreal_tables = SurrealTablesBuilder::default()
            .surreal_items(surreal_items)
            .build()
            .unwrap();

        let now = Utc::now();
        let items = surreal_tables.make_items(&now);
        let parent_lookup = ParentLookup::new(&items);
        let all_time_spent = surreal_tables.make_time_spent_log().collect::<Vec<_>>();
        let events = surreal_tables.make_events();

        // Find the child item
        let child_item = items
            .values()
            .find(|i| i.get_summary() == "Grandchild Item")
            .unwrap();
        let child_node = crate::node::item_node::ItemNode::new(
            child_item,
            &items,
            &parent_lookup,
            &events,
            &all_time_spent,
        );

        let display =
            DisplayItemNode::new(&child_node, Filter::Active, DisplayFormat::MultiLineTree);
        let result = format!("{}", display);

        // Should display: "🪜 Child Item \n  ┗🎯 Parent Item"
        // Note: Item may have type icons like 🪜 before the summary
        assert!(
            result.contains("Child Item"),
            "Expected child item, got: {}",
            result
        );
        assert!(
            result.contains("\n  ┗"),
            "Expected tree connector, got: {}",
            result
        );
        assert!(
            result.contains("Parent Item"),
            "Expected parent item, got: {}",
            result
        );
        assert!(
            result.contains("Grandchild Item"),
            "Should contain grandchild item, got: {}",
            result
        );
        assert!(
            !result.contains("┃"),
            "Should not contain tree vertical lines as all items have a single parent, got: {}",
            result
        );

        // Child should come before parent in the string
        let child_pos = result.find("Child Item").unwrap();
        let parent_pos = result.find("Parent Item").unwrap();
        assert!(child_pos < parent_pos, "Child should appear before parent");
    }

    #[test]
    fn multiline_tree_reversed_format_displays_parent_first_then_child() {
        // Create a parent with a child
        let surreal_items = vec![
            SurrealItemBuilder::default()
                .id(Some(("surreal_item", "parent").into()))
                .summary("Parent Item")
                .item_type(SurrealItemType::Motivation(SurrealMotivationKind::CoreWork))
                .smaller_items_in_priority_order(vec![SurrealOrderedSubItem::SubItem {
                    surreal_item_id: ("surreal_item", "child").into(),
                }])
                .build()
                .unwrap(),
            SurrealItemBuilder::default()
                .id(Some(("surreal_item", "child").into()))
                .summary("Child Item")
                .item_type(SurrealItemType::Action)
                .smaller_items_in_priority_order(vec![SurrealOrderedSubItem::SubItem {
                    surreal_item_id: ("surreal_item", "grandchild").into(),
                }])
                .build()
                .unwrap(),
            SurrealItemBuilder::default()
                .id(Some(("surreal_item", "grandchild").into()))
                .summary("Grandchild Item")
                .item_type(SurrealItemType::Action)
                .build()
                .unwrap(),
        ];

        let surreal_tables = SurrealTablesBuilder::default()
            .surreal_items(surreal_items)
            .build()
            .unwrap();

        let now = Utc::now();
        let items = surreal_tables.make_items(&now);
        let parent_lookup = ParentLookup::new(&items);
        let all_time_spent = surreal_tables.make_time_spent_log().collect::<Vec<_>>();
        let events = surreal_tables.make_events();

        let child_item = items
            .values()
            .find(|i| i.get_summary() == "Grandchild Item")
            .unwrap();
        let child_node = crate::node::item_node::ItemNode::new(
            child_item,
            &items,
            &parent_lookup,
            &events,
            &all_time_spent,
        );

        let display = DisplayItemNode::new(
            &child_node,
            Filter::Active,
            DisplayFormat::MultiLineTreeReversed,
        );
        let result = format!("{}", display);

        // Should display: "🎯 Parent Item\n  ┗🪜 Child Item "
        // Note: Items may have type icons before summaries
        assert!(
            result.contains("Parent Item"),
            "Expected parent item, got: {}",
            result
        );
        assert!(
            result.contains("\n  ┗"),
            "Expected tree connector, got: {}",
            result
        );
        assert!(
            result.contains("Child Item"),
            "Expected child item, got: {}",
            result
        );
        assert!(
            result.contains("Grandchild Item"),
            "Should contain grandchild item, got: {}",
            result
        );
        assert!(
            !result.contains("┃"),
            "Should not contain tree vertical lines as all items have a single parent, got: {}",
            result
        );

        // Parent should come before child in the string
        let parent_pos = result.find("Parent Item").unwrap();
        let child_pos = result.find("Child Item").unwrap();
        assert!(
            parent_pos < child_pos,
            "Parent should appear before child in reversed format"
        );
    }

    #[test]
    fn multiline_tree_reversed_root_has_no_tree_characters() {
        let surreal_items = vec![
            SurrealItemBuilder::default()
                .id(Some(("surreal_item", "root").into()))
                .summary("Root Item")
                .item_type(SurrealItemType::Motivation(SurrealMotivationKind::CoreWork))
                .smaller_items_in_priority_order(vec![SurrealOrderedSubItem::SubItem {
                    surreal_item_id: ("surreal_item", "child").into(),
                }])
                .build()
                .unwrap(),
            SurrealItemBuilder::default()
                .id(Some(("surreal_item", "child").into()))
                .summary("Child Item")
                .item_type(SurrealItemType::Action)
                .smaller_items_in_priority_order(vec![SurrealOrderedSubItem::SubItem {
                    surreal_item_id: ("surreal_item", "grandchild").into(),
                }])
                .build()
                .unwrap(),
            SurrealItemBuilder::default()
                .id(Some(("surreal_item", "grandchild").into()))
                .summary("Grandchild Item")
                .item_type(SurrealItemType::Action)
                .build()
                .unwrap(),
        ];

        let surreal_tables = SurrealTablesBuilder::default()
            .surreal_items(surreal_items)
            .build()
            .unwrap();

        let now = Utc::now();
        let items = surreal_tables.make_items(&now);
        let parent_lookup = ParentLookup::new(&items);
        let all_time_spent = surreal_tables.make_time_spent_log().collect::<Vec<_>>();
        let events = surreal_tables.make_events();

        let child_item = items
            .values()
            .find(|i| i.get_summary() == "Child Item")
            .unwrap();
        let child_node = crate::node::item_node::ItemNode::new(
            child_item,
            &items,
            &parent_lookup,
            &events,
            &all_time_spent,
        );

        let display = DisplayItemNode::new(
            &child_node,
            Filter::Active,
            DisplayFormat::MultiLineTreeReversed,
        );
        let result = format!("{}", display);

        // Root item should not have tree characters before it
        let lines: Vec<&str> = result.lines().collect();
        assert!(
            !lines[0].contains("┗"),
            "Root item should not have tree characters, got: {}",
            lines[0]
        );
        assert!(
            !lines[0].contains("┃"),
            "Root item should not have tree characters, got: {}",
            lines[0]
        );

        // Only the child should have tree connector
        assert!(lines.len() >= 2, "Expected at least 2 lines");
        assert!(lines[1].contains("┗"), "Child should have tree connector");
    }

    #[test]
    fn single_line_format_shows_inline_parents() {
        let surreal_items = vec![
            SurrealItemBuilder::default()
                .id(Some(("surreal_item", "parent").into()))
                .summary("Parent Item")
                .item_type(SurrealItemType::Motivation(SurrealMotivationKind::CoreWork))
                .smaller_items_in_priority_order(vec![SurrealOrderedSubItem::SubItem {
                    surreal_item_id: ("surreal_item", "child").into(),
                }])
                .build()
                .unwrap(),
            SurrealItemBuilder::default()
                .id(Some(("surreal_item", "child").into()))
                .summary("Child Item")
                .item_type(SurrealItemType::Action)
                .build()
                .unwrap(),
        ];

        let surreal_tables = SurrealTablesBuilder::default()
            .surreal_items(surreal_items)
            .build()
            .unwrap();

        let now = Utc::now();
        let items = surreal_tables.make_items(&now);
        let parent_lookup = ParentLookup::new(&items);
        let all_time_spent = surreal_tables.make_time_spent_log().collect::<Vec<_>>();
        let events = surreal_tables.make_events();

        let child_item = items
            .values()
            .find(|i| i.get_summary() == "Child Item")
            .unwrap();
        let child_node = crate::node::item_node::ItemNode::new(
            child_item,
            &items,
            &parent_lookup,
            &events,
            &all_time_spent,
        );

        let display = DisplayItemNode::new(&child_node, Filter::Active, DisplayFormat::SingleLine);
        let result = format!("{}", display);

        // Should be single line with arrow separator
        assert!(
            !result.contains('\n'),
            "SingleLine format should not have newlines"
        );
        assert!(result.contains("Child Item"), "Should contain child item");
        assert!(result.contains("⬅"), "Should contain arrow separator");
        assert!(result.contains("Parent Item"), "Should contain parent item");

        // Child should come before parent
        let child_pos = result.find("Child Item").unwrap();
        let parent_pos = result.find("Parent Item").unwrap();
        assert!(
            child_pos < parent_pos,
            "Child should appear before parent in single line format"
        );
    }

    #[test]
    fn multiline_tree_handles_item_with_no_parents() {
        let surreal_items = vec![
            SurrealItemBuilder::default()
                .id(Some(("surreal_item", "standalone").into()))
                .summary("Standalone Item")
                .item_type(SurrealItemType::Action)
                .build()
                .unwrap(),
        ];

        let surreal_tables = SurrealTablesBuilder::default()
            .surreal_items(surreal_items)
            .build()
            .unwrap();

        let now = Utc::now();
        let items = surreal_tables.make_items(&now);
        let parent_lookup = ParentLookup::new(&items);
        let all_time_spent = surreal_tables.make_time_spent_log().collect::<Vec<_>>();
        let events = surreal_tables.make_events();

        let standalone_item = items
            .values()
            .find(|i| i.get_summary() == "Standalone Item")
            .unwrap();
        let standalone_node = crate::node::item_node::ItemNode::new(
            standalone_item,
            &items,
            &parent_lookup,
            &events,
            &all_time_spent,
        );

        let display = DisplayItemNode::new(
            &standalone_node,
            Filter::Active,
            DisplayFormat::MultiLineTree,
        );
        let result = format!("{}", display);

        // Should just display the item with no tree characters
        assert!(
            result.contains("Standalone Item"),
            "Should contain the item"
        );
        assert!(!result.contains("┗"), "Should not have tree connectors");
        assert!(!result.contains("┃"), "Should not have tree pipes");
        assert!(
            !result.contains('\n'),
            "Should be single line when no parents"
        );
    }

    #[test]
    fn multiline_tree_reversed_handles_item_with_no_parents() {
        let surreal_items = vec![
            SurrealItemBuilder::default()
                .id(Some(("surreal_item", "standalone").into()))
                .summary("Standalone Item")
                .item_type(SurrealItemType::Action)
                .build()
                .unwrap(),
        ];

        let surreal_tables = SurrealTablesBuilder::default()
            .surreal_items(surreal_items)
            .build()
            .unwrap();

        let now = Utc::now();
        let items = surreal_tables.make_items(&now);
        let parent_lookup = ParentLookup::new(&items);
        let all_time_spent = surreal_tables.make_time_spent_log().collect::<Vec<_>>();
        let events = surreal_tables.make_events();

        let standalone_item = items
            .values()
            .find(|i| i.get_summary() == "Standalone Item")
            .unwrap();
        let standalone_node = crate::node::item_node::ItemNode::new(
            standalone_item,
            &items,
            &parent_lookup,
            &events,
            &all_time_spent,
        );

        let display = DisplayItemNode::new(
            &standalone_node,
            Filter::Active,
            DisplayFormat::MultiLineTreeReversed,
        );
        let result = format!("{}", display);

        // Should just display the item with no tree characters
        assert!(
            result.contains("Standalone Item"),
            "Should contain the item"
        );
        assert!(!result.contains("┗"), "Should not have tree connectors");
        assert!(!result.contains("┃"), "Should not have tree pipes");
        assert!(
            !result.contains('\n'),
            "Should be single line when no parents"
        );
    }

    #[test]
    fn single_line_handles_item_with_no_parents() {
        let surreal_items = vec![
            SurrealItemBuilder::default()
                .id(Some(("surreal_item", "standalone").into()))
                .summary("Standalone Item")
                .item_type(SurrealItemType::Action)
                .build()
                .unwrap(),
        ];

        let surreal_tables = SurrealTablesBuilder::default()
            .surreal_items(surreal_items)
            .build()
            .unwrap();

        let now = Utc::now();
        let items = surreal_tables.make_items(&now);
        let parent_lookup = ParentLookup::new(&items);
        let all_time_spent = surreal_tables.make_time_spent_log().collect::<Vec<_>>();
        let events = surreal_tables.make_events();

        let standalone_item = items
            .values()
            .find(|i| i.get_summary() == "Standalone Item")
            .unwrap();
        let standalone_node = crate::node::item_node::ItemNode::new(
            standalone_item,
            &items,
            &parent_lookup,
            &events,
            &all_time_spent,
        );

        let display =
            DisplayItemNode::new(&standalone_node, Filter::Active, DisplayFormat::SingleLine);
        let result = format!("{}", display);

        // Should just display the item with no arrows
        assert!(
            result.contains("Standalone Item"),
            "Should contain the item"
        );
        assert!(
            !result.contains("⬅"),
            "Should not have arrow when no parents"
        );
        assert!(!result.contains('\n'), "Should be single line");
    }
}
