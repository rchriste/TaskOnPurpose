use std::{cmp::Ordering, fmt::Display};

use crate::{
    base_data::item::Item,
    node::{Filter, item_node::ItemNode},
};

use super::display_item::DisplayItem;

#[derive(Clone, Copy)]
pub(crate) enum DisplayFormat {
    MultiLineTree,
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

        if self.item_node.is_person_or_group() {
            write!(f, "Is {} around?", display_item)?;
        } else {
            write!(f, "{} ", display_item)?;
        }
        let parents = self.item_node.create_parent_chain(self.filter);
        match self.display_format {
            DisplayFormat::MultiLineTree => {
                for (j, (depth, item)) in parents.iter().enumerate() {
                    writeln!(f)?;
                    for i in 0..*depth {
                        if i == *depth - 1 {
                            write!(f, "  ┗{}", DisplayItem::new(item))?;
                        } else if parents
                            .iter()
                            .skip(j + 1)
                            .take_while(|(d, _)| (*d - 1) >= i)
                            .any(|(d, _)| *d - 1 == i)
                        {
                            write!(f, "  ┃")?;
                        } else {
                            write!(f, "   ")?;
                        }
                    }
                }
            }
            DisplayFormat::SingleLine => {
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
