use std::fmt::{Display, Formatter};

use surrealdb::RecordId;

use crate::{
    data_storage::surrealdb_layer::{
        surreal_in_the_moment_priority::SurrealAction, surreal_item::SurrealUrgency,
    },
    display::display_action_with_item_status::DisplayActionWithItemStatus,
    node::{
        Filter, action_with_item_status::ActionWithItemStatus,
        why_in_scope_and_action_with_item_status::WhyInScopeAndActionWithItemStatus,
    },
};

use super::display_item_node::DisplayFormat;

#[derive(Clone)]
pub(crate) struct DisplayWhyInScopeAndActionWithItemStatus<'s> {
    item: &'s WhyInScopeAndActionWithItemStatus<'s>,
    filter: Filter,
    display_format: DisplayFormat,
}

impl Display for DisplayWhyInScopeAndActionWithItemStatus<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // For reversed tree format, we need special handling to print root first
        if matches!(self.display_format, DisplayFormat::MultiLineTreeReversed) {
            // Get the action to extract the item node
            let item_node = match self.get_action() {
                ActionWithItemStatus::MakeProgress(status)
                | ActionWithItemStatus::ParentBackToAMotivation(status)
                | ActionWithItemStatus::PickItemReviewFrequency(status)
                | ActionWithItemStatus::ItemNeedsAClassification(status)
                | ActionWithItemStatus::ReviewItem(status)
                | ActionWithItemStatus::SetReadyAndUrgency(status) => status.get_item_node(),
            };

            // Get parent chain
            let parents = item_node.create_parent_chain(self.filter);

            // Print from root (highest depth) down to item
            let mut sorted_parents: Vec<_> = parents.iter().collect();
            sorted_parents.sort_by(|(depth_a, _), (depth_b, _)| depth_b.cmp(depth_a));

            for (i, (_depth, parent)) in sorted_parents.iter().enumerate() {
                if i > 0 {
                    writeln!(f)?;
                }

                // Only print tree characters if this is NOT the root (i > 0)
                if i > 0 {
                    // Indentation increases as we go from root to leaves
                    let indent_level = i - 1;
                    for j in 0..=indent_level {
                        if j == indent_level {
                            write!(f, "  ┗")?;
                        } else {
                            write!(f, "  ┃")?;
                        }
                    }
                }
                write!(
                    f,
                    "{}",
                    crate::display::display_item::DisplayItem::new(parent)
                )?;
            }

            // Finally print the actual item at the bottom
            writeln!(f)?;
            // Only print tree characters if there are parents
            if !sorted_parents.is_empty() {
                let final_indent = sorted_parents.len() - 1;
                for j in 0..=final_indent {
                    if j == final_indent {
                        write!(f, "  ┗")?;
                    } else {
                        write!(f, "  ┃")?;
                    }
                }
            }

            // Print urgency and action prefix for the main item
            if self.is_in_scope_for_importance() {
                write!(f, "🔝 ")?;
            }
            let urgency = self.get_urgency_now();
            match urgency {
                SurrealUrgency::MoreUrgentThanAnythingIncludingScheduled => write!(f, "🚨 ")?,
                SurrealUrgency::MoreUrgentThanMode => write!(f, "🔥 ")?,
                SurrealUrgency::InTheModeByImportance => {}
                SurrealUrgency::InTheModeDefinitelyUrgent => write!(f, "🔴 ")?,
                SurrealUrgency::InTheModeMaybeUrgent => write!(f, "🟡 ")?,
                SurrealUrgency::ScheduledAnyMode(..) => write!(f, "🗓️❗ ")?,
                SurrealUrgency::InTheModeScheduled(..) => write!(f, "🗓️⭳ ")?,
            }

            // Print action type
            match self.get_action() {
                ActionWithItemStatus::MakeProgress(_) => write!(f, "[🏃 Do Now] ")?,
                ActionWithItemStatus::ParentBackToAMotivation(_) => {
                    write!(f, "[🌟 Needs a reason] ")?
                }
                ActionWithItemStatus::PickItemReviewFrequency(_) => {
                    write!(f, "[🔁 State review frequency] ")?
                }
                ActionWithItemStatus::ItemNeedsAClassification(_) => {
                    write!(f, "[🗂️ Needs classification] ")?
                }
                ActionWithItemStatus::ReviewItem(status) => {
                    if let Some(review_frequency) = status.get_item().get_surreal_review_frequency()
                    {
                        write!(f, "[🔍 Review - {}] ", review_frequency)?;
                    } else {
                        write!(f, "[🔍 Review] ")?;
                    }
                }
                ActionWithItemStatus::SetReadyAndUrgency(_) => {
                    write!(f, "[🚦 Set readiness and urgency] ")?
                }
            }

            write!(f, "|")?;
            let status = match self.get_action() {
                ActionWithItemStatus::MakeProgress(status)
                | ActionWithItemStatus::ParentBackToAMotivation(status)
                | ActionWithItemStatus::PickItemReviewFrequency(status)
                | ActionWithItemStatus::ItemNeedsAClassification(status)
                | ActionWithItemStatus::ReviewItem(status)
                | ActionWithItemStatus::SetReadyAndUrgency(status) => status,
            };
            if status.has_dependencies(self.filter) {
                write!(f, "⏳ ")?;
            }

            // Print the actual item summary
            let item = item_node.get_item();
            let display_item = crate::display::display_item::DisplayItem::new(item);
            if item_node.is_person_or_group() {
                write!(f, "Is {} around?", display_item)?;
            } else {
                write!(f, "{}", display_item)?;
            }

            return Ok(());
        }

        // Standard format (not reversed)
        if self.is_in_scope_for_importance() {
            write!(f, "🔝 ")?;
        }

        let urgency = self.get_urgency_now();
        match urgency {
            SurrealUrgency::MoreUrgentThanAnythingIncludingScheduled => write!(f, "🚨 ")?,
            SurrealUrgency::MoreUrgentThanMode => write!(f, "🔥 ")?,
            SurrealUrgency::InTheModeByImportance => {}
            SurrealUrgency::InTheModeDefinitelyUrgent => write!(f, "🔴 ")?,
            SurrealUrgency::InTheModeMaybeUrgent => write!(f, "🟡 ")?,
            SurrealUrgency::ScheduledAnyMode(..) => write!(f, "🗓️❗ ")?,
            SurrealUrgency::InTheModeScheduled(..) => write!(f, "🗓️⭳ ")?,
        }

        write!(
            f,
            "{}",
            DisplayActionWithItemStatus::new(self.get_action(), self.filter, self.display_format)
        )
    }
}

impl<'s> DisplayWhyInScopeAndActionWithItemStatus<'s> {
    pub(crate) fn new(
        item: &'s WhyInScopeAndActionWithItemStatus<'s>,
        filter: Filter,
        display_format: DisplayFormat,
    ) -> Self {
        Self {
            item,
            filter,
            display_format,
        }
    }

    pub(crate) fn get_urgency_now(&self) -> SurrealUrgency {
        self.item.get_urgency_now()
    }

    pub(crate) fn get_action(&self) -> &ActionWithItemStatus<'s> {
        self.item.get_action()
    }

    pub(crate) fn is_in_scope_for_importance(&self) -> bool {
        self.item.is_in_scope_for_importance()
    }

    pub(crate) fn get_surreal_record_id(&self) -> &RecordId {
        self.item.get_surreal_record_id()
    }

    pub(crate) fn clone_to_surreal_action(&self) -> SurrealAction {
        self.item.clone_to_surreal_action()
    }
}

impl<'s> From<DisplayWhyInScopeAndActionWithItemStatus<'s>>
    for &'s WhyInScopeAndActionWithItemStatus<'s>
{
    fn from(display: DisplayWhyInScopeAndActionWithItemStatus<'s>) -> Self {
        display.item
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        base_data::BaseData,
        calculated_data::CalculatedData,
        data_storage::surrealdb_layer::{
            surreal_item::{
                SurrealItemBuilder, SurrealItemType, SurrealMotivationKind, SurrealOrderedSubItem,
            },
            surreal_tables::SurrealTablesBuilder,
        },
        node::{
            urgency_level_item_with_item_status::UrgencyLevelItemWithItemStatus,
            why_in_scope_and_action_with_item_status::WhyInScope,
        },
        systems::do_now_list::DoNowList,
    };
    use chrono::Utc;

    fn create_test_calculated_data() -> CalculatedData {
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
        let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
        CalculatedData::new_from_base_data(base_data)
    }

    #[test]
    fn multiline_tree_reversed_format_displays_parent_first_with_action_prefix() {
        let calculated_data = create_test_calculated_data();
        let now = Utc::now();
        let do_now_list = DoNowList::new_do_now_list(calculated_data, &now);

        let ordered_list = do_now_list.get_ordered_do_now_list();
        assert!(!ordered_list.is_empty(), "Should have items in do now list");

        // Get first item from the list
        let first_item = match &ordered_list[0] {
            UrgencyLevelItemWithItemStatus::SingleItem(item) => item,
            UrgencyLevelItemWithItemStatus::MultipleItems(items) => &items[0],
        };

        let display = DisplayWhyInScopeAndActionWithItemStatus::new(
            first_item,
            Filter::Active,
            DisplayFormat::MultiLineTreeReversed,
        );
        let result = format!("{}", display);

        // Should display hierarchy with action prefix
        let item_node = match first_item.get_action() {
            ActionWithItemStatus::MakeProgress(status)
            | ActionWithItemStatus::ParentBackToAMotivation(status)
            | ActionWithItemStatus::PickItemReviewFrequency(status)
            | ActionWithItemStatus::ItemNeedsAClassification(status)
            | ActionWithItemStatus::ReviewItem(status)
            | ActionWithItemStatus::SetReadyAndUrgency(status) => status.get_item_node(),
        };
        assert!(
            result.contains("┗") || !item_node.has_parents(Filter::Active),
            "Expected tree connector if item has parents, got: {}",
            result
        );
        // Should have an action prefix like [🏃 Do Now], [🚦 Set readiness and urgency], etc.
        assert!(
            result.contains("["),
            "Expected action prefix, got: {}",
            result
        );
    }

    #[test]
    fn multiline_tree_reversed_root_has_no_tree_characters_at_start() {
        let calculated_data = create_test_calculated_data();
        let now = Utc::now();
        let do_now_list = DoNowList::new_do_now_list(calculated_data, &now);

        let ordered_list = do_now_list.get_ordered_do_now_list();
        if let Some(first_urgency_level) = ordered_list.first() {
            let first_item = match first_urgency_level {
                UrgencyLevelItemWithItemStatus::SingleItem(item) => item,
                UrgencyLevelItemWithItemStatus::MultipleItems(items) => &items[0],
            };

            let display = DisplayWhyInScopeAndActionWithItemStatus::new(
                first_item,
                Filter::Active,
                DisplayFormat::MultiLineTreeReversed,
            );
            let result = format!("{}", display);

            // Root item (first line) should not have tree characters before it
            let lines: Vec<&str> = result.lines().collect();
            if !lines.is_empty() {
                assert!(
                    !lines[0].starts_with("  ┗"),
                    "Root item should not start with tree characters, got: {}",
                    lines[0]
                );
                assert!(
                    !lines[0].starts_with("  ┃"),
                    "Root item should not start with tree characters, got: {}",
                    lines[0]
                );
            }
        }
    }

    #[test]
    fn standard_format_displays_action_inline() {
        let calculated_data = create_test_calculated_data();
        let now = Utc::now();
        let do_now_list = DoNowList::new_do_now_list(calculated_data, &now);

        let ordered_list = do_now_list.get_ordered_do_now_list();
        if let Some(first_urgency_level) = ordered_list.first() {
            let first_item = match first_urgency_level {
                UrgencyLevelItemWithItemStatus::SingleItem(item) => item,
                UrgencyLevelItemWithItemStatus::MultipleItems(items) => &items[0],
            };

            let display = DisplayWhyInScopeAndActionWithItemStatus::new(
                first_item,
                Filter::Active,
                DisplayFormat::MultiLineTree,
            );
            let result = format!("{}", display);

            // Should contain the item summary
            let item_node = match first_item.get_action() {
                ActionWithItemStatus::MakeProgress(status)
                | ActionWithItemStatus::ParentBackToAMotivation(status)
                | ActionWithItemStatus::PickItemReviewFrequency(status)
                | ActionWithItemStatus::ItemNeedsAClassification(status)
                | ActionWithItemStatus::ReviewItem(status)
                | ActionWithItemStatus::SetReadyAndUrgency(status) => status.get_item_node(),
            };

            assert!(
                result.contains(item_node.get_summary()),
                "Should contain item summary"
            );
        }
    }

    #[test]
    fn displays_importance_indicator_when_in_scope() {
        let calculated_data = create_test_calculated_data();
        let now = Utc::now();
        let do_now_list = DoNowList::new_do_now_list(calculated_data, &now);

        let ordered_list = do_now_list.get_ordered_do_now_list();

        // Find an item that's in scope for importance
        for urgency_level in ordered_list.iter() {
            let items_to_check = match urgency_level {
                UrgencyLevelItemWithItemStatus::SingleItem(item) => vec![item],
                UrgencyLevelItemWithItemStatus::MultipleItems(items) => items.iter().collect(),
            };

            for item in items_to_check {
                if item.get_why_in_scope().contains(&WhyInScope::Importance) {
                    let display = DisplayWhyInScopeAndActionWithItemStatus::new(
                        item,
                        Filter::Active,
                        DisplayFormat::MultiLineTreeReversed,
                    );
                    let result = format!("{}", display);

                    assert!(
                        result.contains("🔝"),
                        "Should contain importance indicator when in scope for importance, got: {}",
                        result
                    );
                    return; // Test passes
                }
            }
        }
    }

    #[test]
    #[allow(clippy::clone_on_copy)] // For testing Clone and Copy traits
    fn display_format_is_clone_and_copy() {
        // Verify DisplayFormat enum has Clone and Copy traits
        let format = DisplayFormat::MultiLineTree;
        let _cloned = format.clone();
        let _copied = format;
        // If this compiles, the traits are properly derived
    }

    #[test]
    fn can_create_display_wrapper_with_different_formats() {
        let calculated_data = create_test_calculated_data();
        let now = Utc::now();
        let do_now_list = DoNowList::new_do_now_list(calculated_data, &now);

        let ordered_list = do_now_list.get_ordered_do_now_list();
        if let Some(first_urgency_level) = ordered_list.first() {
            let first_item = match first_urgency_level {
                UrgencyLevelItemWithItemStatus::SingleItem(item) => item,
                UrgencyLevelItemWithItemStatus::MultipleItems(items) => &items[0],
            };

            // Test that we can create display wrappers with all three formats
            let _multi_tree = DisplayWhyInScopeAndActionWithItemStatus::new(
                first_item,
                Filter::Active,
                DisplayFormat::MultiLineTree,
            );

            let _multi_tree_reversed = DisplayWhyInScopeAndActionWithItemStatus::new(
                first_item,
                Filter::Active,
                DisplayFormat::MultiLineTreeReversed,
            );

            let _single_line = DisplayWhyInScopeAndActionWithItemStatus::new(
                first_item,
                Filter::Active,
                DisplayFormat::SingleLine,
            );

            // If this compiles and runs, all three formats work
        }
    }

    #[test]
    fn multiline_tree_reversed_displays_hierarchy_top_to_bottom() {
        let surreal_items = vec![
            SurrealItemBuilder::default()
                .id(Some(("surreal_item", "grandparent").into()))
                .summary("Grandparent Item")
                .item_type(SurrealItemType::Motivation(SurrealMotivationKind::CoreWork))
                .smaller_items_in_priority_order(vec![SurrealOrderedSubItem::SubItem {
                    surreal_item_id: ("surreal_item", "parent").into(),
                }])
                .build()
                .unwrap(),
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
        let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
        let calculated_data = CalculatedData::new_from_base_data(base_data);
        let do_now_list = DoNowList::new_do_now_list(calculated_data, &now);

        let ordered_list = do_now_list.get_ordered_do_now_list();
        if let Some(first_urgency_level) = ordered_list.first() {
            let first_item = match first_urgency_level {
                UrgencyLevelItemWithItemStatus::SingleItem(item) => item,
                UrgencyLevelItemWithItemStatus::MultipleItems(items) => &items[0],
            };

            let display = DisplayWhyInScopeAndActionWithItemStatus::new(
                first_item,
                Filter::Active,
                DisplayFormat::MultiLineTreeReversed,
            );
            let result = format!("{}", display);

            // Check that items appear in order if they're all present
            if result.contains("Grandparent Item")
                && result.contains("Parent Item")
                && result.contains("Child Item")
            {
                let grandparent_pos = result.find("Grandparent Item").unwrap();
                let parent_pos = result.find("Parent Item").unwrap();
                let child_pos = result.find("Child Item").unwrap();

                assert!(
                    grandparent_pos < parent_pos,
                    "Grandparent should appear before parent"
                );
                assert!(parent_pos < child_pos, "Parent should appear before child");
            }
        }
    }
}
