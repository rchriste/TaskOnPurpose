use std::fmt::{self, Display};

use chrono::{DateTime, Local};

use crate::node::{Filter, item_status::DependencyWithItemNode};

use super::display_item_node::{DisplayFormat, DisplayItemNode};

pub(crate) struct DisplayDependenciesWithItemNode<'s> {
    dependencies: &'s Vec<&'s DependencyWithItemNode<'s>>,
    filter: Filter,
    display_format: DisplayFormat,
}

impl Display for DisplayDependenciesWithItemNode<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.dependencies.is_empty() {
            write!(f, "Always")
        } else {
            for waiting_on in self.dependencies.iter() {
                match waiting_on {
                    DependencyWithItemNode::AfterDateTime {
                        after,
                        is_active: _is_active,
                    } => {
                        let datetime: DateTime<Local> = after.with_timezone(&Local);
                        write!(f, "After {}", datetime.format("%a %d %b %Y %I:%M:%S%P"))?;
                    }
                    DependencyWithItemNode::UntilScheduled {
                        after,
                        is_active: _is_active,
                    } => {
                        let datetime: DateTime<Local> = after.with_timezone(&Local);
                        write!(
                            f,
                            "Until scheduled {}",
                            datetime.format("%a %d %b %Y %I:%M:%S%P")
                        )?
                    }
                    DependencyWithItemNode::AfterItem(dependency) => {
                        let display_item_node =
                            DisplayItemNode::new(dependency, self.filter, self.display_format);
                        write!(f, "After dependency {}", display_item_node)?
                    }
                    DependencyWithItemNode::AfterChildItem(smaller_item) => {
                        let display_item_node =
                            DisplayItemNode::new(smaller_item, self.filter, self.display_format);
                        write!(f, "After child item {}", display_item_node)?
                    }
                    DependencyWithItemNode::DuringItem(item_node) => {
                        let display_item_node =
                            DisplayItemNode::new(item_node, self.filter, self.display_format);
                        write!(f, "During item {}", display_item_node)?
                    }
                    DependencyWithItemNode::WaitingToBeInterrupted => {
                        write!(f, "Waiting to be interrupted")?
                    }
                    DependencyWithItemNode::AfterEvent(event) => {
                        write!(f, "After event {}", event.get_summary())?
                    }
                }
            }
            Ok(())
        }
    }
}

impl<'s> DisplayDependenciesWithItemNode<'s> {
    pub(crate) fn new(
        dependencies: &'s Vec<&'s DependencyWithItemNode<'s>>,
        filter: Filter,
        display_format: DisplayFormat,
    ) -> Self {
        DisplayDependenciesWithItemNode {
            dependencies,
            filter,
            display_format,
        }
    }
}
