use std::{hash::Hash, mem};

use ahash::HashMap;
use itertools::Itertools;
use surrealdb::RecordId;

use crate::{
    base_data::in_the_moment_priority::InTheMomentPriorityWithItemAction,
    data_storage::surrealdb_layer::{
        surreal_in_the_moment_priority::{SurrealAction, SurrealPriorityKind},
        surreal_item::SurrealUrgency,
    },
};

use super::{
    item_node::ItemNode,
    item_status::{ActionWithItemNode, ItemStatus},
    urgency_level_item_with_item_status::UrgencyLevelItemWithItemStatus,
    why_in_scope_and_action_with_item_status::WhyInScopeAndActionWithItemStatus,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ActionWithItemStatus<'e> {
    SetReadyAndUrgency(&'e ItemStatus<'e>),
    ParentBackToAMotivation(&'e ItemStatus<'e>),
    ItemNeedsAClassification(&'e ItemStatus<'e>),
    ReviewItem(&'e ItemStatus<'e>),
    PickItemReviewFrequency(&'e ItemStatus<'e>),
    MakeProgress(&'e ItemStatus<'e>),
}

impl Hash for ActionWithItemStatus<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        mem::discriminant(self).hash(state);
        self.get_surreal_record_id().hash(state);
    }
}

impl<'e> ActionWithItemStatus<'e> {
    pub(crate) fn new(
        action: &ActionWithItemNode<'_>,
        items_status: &'e HashMap<&'e RecordId, ItemStatus<'e>>,
    ) -> Self {
        match action {
            ActionWithItemNode::SetReadyAndUrgency(action) => {
                let item_status = items_status
                    .get(action.get_surreal_record_id())
                    .expect("All items are there");
                ActionWithItemStatus::SetReadyAndUrgency(item_status)
            }
            ActionWithItemNode::ParentBackToAMotivation(action) => {
                let item_status = items_status
                    .get(action.get_surreal_record_id())
                    .expect("All items are there");
                ActionWithItemStatus::ParentBackToAMotivation(item_status)
            }
            ActionWithItemNode::ReviewItem(action) => {
                let item_status = items_status
                    .get(action.get_surreal_record_id())
                    .expect("All items are there");
                ActionWithItemStatus::ReviewItem(item_status)
            }
            ActionWithItemNode::PickItemReviewFrequency(action) => {
                let item_status = items_status
                    .get(action.get_surreal_record_id())
                    .expect("All items are there");
                ActionWithItemStatus::PickItemReviewFrequency(item_status)
            }
            ActionWithItemNode::ItemNeedsAClassification(action) => {
                let item_status = items_status
                    .get(action.get_surreal_record_id())
                    .expect("All items are there");
                ActionWithItemStatus::ItemNeedsAClassification(item_status)
            }
            ActionWithItemNode::MakeProgress(action) => {
                let item_status = items_status
                    .get(action.get_surreal_record_id())
                    .expect("All items are there");
                ActionWithItemStatus::MakeProgress(item_status)
            }
        }
    }

    pub(crate) fn from_surreal_action(
        action: &SurrealAction,
        items_status: &'e HashMap<&'e RecordId, ItemStatus<'e>>,
    ) -> Self {
        match action {
            SurrealAction::SetReadyAndUrgency(record_id) => {
                let item_status = items_status.get(record_id).expect("All items are there");
                ActionWithItemStatus::SetReadyAndUrgency(item_status)
            }
            SurrealAction::ParentBackToAMotivation(record_id) => {
                let item_status = items_status.get(record_id).expect("All items are there");
                ActionWithItemStatus::ParentBackToAMotivation(item_status)
            }
            SurrealAction::ReviewItem(record_id) => {
                let item_status = items_status.get(record_id).expect("All items are there");
                ActionWithItemStatus::ReviewItem(item_status)
            }
            SurrealAction::PickItemReviewFrequency(record_id) => {
                let item_status = items_status.get(record_id).expect("All items are there");
                ActionWithItemStatus::PickItemReviewFrequency(item_status)
            }
            SurrealAction::MakeProgress(record_id) => {
                let item_status = items_status.get(record_id).expect("All items are there");
                ActionWithItemStatus::MakeProgress(item_status)
            }
            SurrealAction::ItemNeedsAClassification(record_id) => {
                let item_status = items_status.get(record_id).expect("All items are there");
                ActionWithItemStatus::ItemNeedsAClassification(item_status)
            }
        }
    }

    pub(crate) fn clone_to_surreal_action(&self) -> SurrealAction {
        match self {
            ActionWithItemStatus::MakeProgress(item) => {
                SurrealAction::MakeProgress(item.get_surreal_record_id().clone())
            }
            ActionWithItemStatus::ParentBackToAMotivation(item) => {
                SurrealAction::ParentBackToAMotivation(item.get_surreal_record_id().clone())
            }
            ActionWithItemStatus::PickItemReviewFrequency(item) => {
                SurrealAction::PickItemReviewFrequency(item.get_surreal_record_id().clone())
            }
            ActionWithItemStatus::ItemNeedsAClassification(item) => {
                SurrealAction::ItemNeedsAClassification(item.get_surreal_record_id().clone())
            }
            ActionWithItemStatus::ReviewItem(item) => {
                SurrealAction::ReviewItem(item.get_surreal_record_id().clone())
            }
            ActionWithItemStatus::SetReadyAndUrgency(item) => {
                SurrealAction::SetReadyAndUrgency(item.get_surreal_record_id().clone())
            }
        }
    }

    pub(crate) fn get_surreal_record_id(&self) -> &RecordId {
        match self {
            ActionWithItemStatus::SetReadyAndUrgency(item)
            | ActionWithItemStatus::ParentBackToAMotivation(item)
            | ActionWithItemStatus::ReviewItem(item)
            | ActionWithItemStatus::PickItemReviewFrequency(item)
            | ActionWithItemStatus::ItemNeedsAClassification(item)
            | ActionWithItemStatus::MakeProgress(item) => item.get_surreal_record_id(),
        }
    }

    pub(crate) fn get_urgency_now(&self) -> SurrealUrgency {
        match self {
            ActionWithItemStatus::MakeProgress(item_status, ..) => item_status
                .get_urgency_now()
                .unwrap_or(&SurrealUrgency::InTheModeByImportance)
                .clone(),
            ActionWithItemStatus::ParentBackToAMotivation(..)
            | ActionWithItemStatus::ItemNeedsAClassification(..) => {
                SurrealUrgency::MoreUrgentThanMode
            }
            ActionWithItemStatus::PickItemReviewFrequency(..) => {
                SurrealUrgency::InTheModeMaybeUrgent
            }
            ActionWithItemStatus::ReviewItem(..) => SurrealUrgency::InTheModeMaybeUrgent,
            ActionWithItemStatus::SetReadyAndUrgency(..) => {
                SurrealUrgency::InTheModeDefinitelyUrgent
            }
        }
    }

    pub(crate) fn get_item_node(&self) -> &ItemNode<'e> {
        match self {
            ActionWithItemStatus::SetReadyAndUrgency(item)
            | ActionWithItemStatus::ParentBackToAMotivation(item)
            | ActionWithItemStatus::ReviewItem(item)
            | ActionWithItemStatus::PickItemReviewFrequency(item)
            | ActionWithItemStatus::ItemNeedsAClassification(item)
            | ActionWithItemStatus::MakeProgress(item) => item.get_item_node(),
        }
    }
}

#[derive(Default)]
pub(crate) struct WhyInScopeActionListsByUrgency<'s> {
    pub(crate) more_urgent_than_anything_including_scheduled:
        Vec<WhyInScopeAndActionWithItemStatus<'s>>,
    pub(crate) scheduled_any_mode: Vec<WhyInScopeAndActionWithItemStatus<'s>>,
    pub(crate) more_urgent_than_mode: Vec<WhyInScopeAndActionWithItemStatus<'s>>,
    pub(crate) in_the_mode_scheduled: Vec<WhyInScopeAndActionWithItemStatus<'s>>,
    pub(crate) in_the_mode_definitely_urgent: Vec<WhyInScopeAndActionWithItemStatus<'s>>,
    pub(crate) in_the_mode_maybe_urgent_and_by_importance:
        Vec<WhyInScopeAndActionWithItemStatus<'s>>,
}

impl<'s> WhyInScopeActionListsByUrgency<'s> {
    pub(crate) fn apply_in_the_moment_priorities(
        self,
        all_priorities: &'s [InTheMomentPriorityWithItemAction<'s>],
    ) -> Vec<UrgencyLevelItemWithItemStatus<'s>> {
        let mut ordered_bullet_list = Vec::new();

        if let Some(more_urgent_than_anything_including_scheduled) = self
            .more_urgent_than_anything_including_scheduled
            .apply_in_the_moment_priorities(all_priorities)
        {
            ordered_bullet_list.push(more_urgent_than_anything_including_scheduled);
        }

        if let Some(scheduled_any_mode) = self
            .scheduled_any_mode
            .apply_in_the_moment_priorities(all_priorities)
        {
            ordered_bullet_list.push(scheduled_any_mode);
        }

        if let Some(more_urgent_than_mode) = self
            .more_urgent_than_mode
            .apply_in_the_moment_priorities(all_priorities)
        {
            ordered_bullet_list.push(more_urgent_than_mode);
        }

        if let Some(in_the_mode_scheduled) = self
            .in_the_mode_scheduled
            .apply_in_the_moment_priorities(all_priorities)
        {
            ordered_bullet_list.push(in_the_mode_scheduled);
        }

        if let Some(in_the_mode_definitely_urgent) = self
            .in_the_mode_definitely_urgent
            .apply_in_the_moment_priorities(all_priorities)
        {
            ordered_bullet_list.push(in_the_mode_definitely_urgent);
        }

        if let Some(in_the_mode_maybe_urgent_and_by_importance) = self
            .in_the_mode_maybe_urgent_and_by_importance
            .apply_in_the_moment_priorities(all_priorities)
        {
            ordered_bullet_list.push(in_the_mode_maybe_urgent_and_by_importance);
        }

        ordered_bullet_list
    }
}

trait ApplyInTheMomentPriorities<'s> {
    fn apply_in_the_moment_priorities(
        self,
        all_priorities: &'s [InTheMomentPriorityWithItemAction<'s>],
    ) -> Option<UrgencyLevelItemWithItemStatus<'s>>;
}

impl<'s> ApplyInTheMomentPriorities<'s> for Vec<WhyInScopeAndActionWithItemStatus<'s>> {
    fn apply_in_the_moment_priorities(
        self,
        all_priorities: &'s [InTheMomentPriorityWithItemAction<'s>],
    ) -> Option<UrgencyLevelItemWithItemStatus<'s>> {
        let mut choices = self.clone();
        for priority in all_priorities.iter().filter(|x| x.is_active()) {
            match priority.get_kind() {
                SurrealPriorityKind::HighestPriority => {
                    if choices
                        .iter()
                        .any(|item_action| priority.get_choice() == item_action.get_action())
                    {
                        for lower_priority in priority.get_not_chosen() {
                            if let Some((i, _)) = choices
                                .iter()
                                .find_position(|x| x.get_action() == lower_priority)
                            {
                                choices.swap_remove(i);
                            }
                        }
                    }
                }
                SurrealPriorityKind::LowestPriority => {
                    if let Some((position, _)) = choices.iter().find_position(|item_action| {
                        priority.get_choice() == item_action.get_action()
                    }) && choices.iter().any(|item_action| {
                        priority
                            .get_not_chosen()
                            .iter()
                            .any(|lower_priority| item_action.get_action() == lower_priority)
                    }) {
                        choices.swap_remove(position);
                    }
                }
            }
        }

        if self.len() > 1 {
            assert!(
                !choices.is_empty(),
                "I am not expecting that it will ever be possible to remove all choices if so then this should be debugged"
            );
        }

        if choices.is_empty() {
            None
        } else if choices.len() == 1 {
            Some(UrgencyLevelItemWithItemStatus::SingleItem(
                choices
                    .into_iter()
                    .next()
                    .expect("Size is checked to be at least 1"),
            ))
        } else {
            Some(UrgencyLevelItemWithItemStatus::new_multiple_items(choices))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::iter::once;

    use ahash::HashSet;
    use chrono::Utc;
    use itertools::chain;

    use crate::{
        base_data::BaseData,
        calculated_data,
        data_storage::surrealdb_layer::{
            SurrealTrigger,
            surreal_in_the_moment_priority::{
                SurrealAction, SurrealInTheMomentPriorityBuilder, SurrealPriorityKind,
            },
            surreal_item::{
                SurrealItemBuilder, SurrealItemType, SurrealMotivationKind, SurrealOrderedSubItem,
            },
            surreal_tables::SurrealTablesBuilder,
        },
        node::{
            action_with_item_status::{
                ActionWithItemStatus, ApplyInTheMomentPriorities, UrgencyLevelItemWithItemStatus,
                WhyInScopeAndActionWithItemStatus,
            },
            why_in_scope_and_action_with_item_status::WhyInScope,
        },
    };

    fn test_default_mode_why_in_scope() -> HashSet<WhyInScope> {
        chain!(once(WhyInScope::Importance), once(WhyInScope::Urgency)).collect()
    }

    #[test]
    fn apply_in_the_moment_priorities_when_only_one_item_is_given_with_no_in_the_moment_priorities_then_that_one_item_is_returned()
     {
        let only_item = SurrealItemBuilder::default()
            .id(Some(("surreal_item", "1").into()))
            .summary("Only item")
            .build()
            .unwrap();

        let surreal_tables = SurrealTablesBuilder::default()
            .surreal_items(vec![only_item.clone()])
            .build()
            .unwrap();

        let now = Utc::now();
        let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
        let calculated_data = calculated_data::CalculatedData::new_from_base_data(base_data);

        let (_, item_status) = calculated_data.get_items_status().iter().next().unwrap();
        let why_in_scope = test_default_mode_why_in_scope();
        let item_action = ActionWithItemStatus::MakeProgress(item_status);
        let why_in_scope_and_action_with_item_status =
            WhyInScopeAndActionWithItemStatus::new(why_in_scope, item_action);

        let blank_in_the_moment_priorities = calculated_data.get_in_the_moment_priorities();

        let dut = vec![why_in_scope_and_action_with_item_status];
        let result = dut.apply_in_the_moment_priorities(blank_in_the_moment_priorities);

        assert!(result.is_some());
        let result = result.expect("assert.is_some() should have passed");
        assert!(matches!(
            result,
            UrgencyLevelItemWithItemStatus::SingleItem(_)
        ));
        match result {
            UrgencyLevelItemWithItemStatus::MultipleItems(..) => panic!("Test Failure"),
            UrgencyLevelItemWithItemStatus::SingleItem(result) => {
                assert_eq!(
                    result.clone_to_surreal_action().get_record_id(),
                    &only_item.id.unwrap()
                );

                assert_eq!(
                    result.get_action(),
                    &ActionWithItemStatus::MakeProgress(item_status)
                );
            }
        }
    }

    #[test]
    fn apply_in_the_moment_priorities_when_only_two_items_are_given_pick_between_them_is_returned()
    {
        let first_item = SurrealItemBuilder::default()
            .id(Some(("surreal_item", "1").into()))
            .summary("First item")
            .build()
            .unwrap();
        let second_item = SurrealItemBuilder::default()
            .id(Some(("surreal_item", "2").into()))
            .summary("Second item")
            .build()
            .unwrap();

        let surreal_tables = SurrealTablesBuilder::default()
            .surreal_items(vec![first_item.clone(), second_item.clone()])
            .build()
            .unwrap();

        let now = Utc::now();
        let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
        let calculated_data = calculated_data::CalculatedData::new_from_base_data(base_data);
        let items_status = calculated_data.get_items_status();

        let first_item_status = items_status
            .get(first_item.id.as_ref().unwrap())
            .expect("First item status not found");
        let second_item_status = items_status
            .get(second_item.id.as_ref().unwrap())
            .expect("Second item status not found");

        let first_item_action = ActionWithItemStatus::MakeProgress(first_item_status);
        let first_item_action = WhyInScopeAndActionWithItemStatus::new(
            test_default_mode_why_in_scope(),
            first_item_action,
        );
        let second_item_action = ActionWithItemStatus::MakeProgress(second_item_status);
        let second_item_action = WhyInScopeAndActionWithItemStatus::new(
            test_default_mode_why_in_scope(),
            second_item_action,
        );

        let blank_in_the_moment_priorities = calculated_data.get_in_the_moment_priorities();

        let dut = vec![first_item_action.clone(), second_item_action.clone()];
        let result = dut.apply_in_the_moment_priorities(blank_in_the_moment_priorities);

        assert!(result.is_some());
        let result = result.expect("assert.is_some() should have passed");
        match result {
            UrgencyLevelItemWithItemStatus::SingleItem(..) => panic!("Test Failure"),
            UrgencyLevelItemWithItemStatus::MultipleItems(vec) => {
                assert_eq!(
                    vec.len(),
                    vec![first_item_action, second_item_action,].len()
                );
            }
        }
    }

    #[test]
    fn apply_in_the_moment_priorities_when_two_items_are_given_and_one_is_the_highest_in_the_moment_priority_that_one_is_returned()
     {
        let first_item = SurrealItemBuilder::default()
            .id(Some(("surreal_item", "1").into()))
            .summary("First item")
            .build()
            .unwrap();
        let second_item = SurrealItemBuilder::default()
            .id(Some(("surreal_item", "2").into()))
            .summary("Second item")
            .build()
            .unwrap();

        let in_an_hour = Utc::now() + chrono::Duration::hours(1);
        let in_the_moment_priority = SurrealInTheMomentPriorityBuilder::default()
            .id(Some(("surreal_in_the_moment_priority", "1").into()))
            .kind(SurrealPriorityKind::HighestPriority)
            .choice(SurrealAction::MakeProgress(
                first_item.id.clone().expect("hard coded to a value"),
            ))
            .not_chosen(vec![SurrealAction::MakeProgress(
                second_item.id.clone().expect("hard coded to a value"),
            )])
            .in_effect_until(vec![SurrealTrigger::WallClockDateTime(in_an_hour.into())])
            .build()
            .unwrap();

        let surreal_tables = SurrealTablesBuilder::default()
            .surreal_items(vec![first_item.clone(), second_item.clone()])
            .surreal_in_the_moment_priorities(vec![in_the_moment_priority])
            .build()
            .unwrap();

        let now = Utc::now();
        let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
        let calculated_data = calculated_data::CalculatedData::new_from_base_data(base_data);
        let items_status = calculated_data.get_items_status();

        let first_item_status = items_status
            .get(first_item.id.as_ref().unwrap())
            .expect("First item status not found");
        let second_item_status = items_status
            .get(second_item.id.as_ref().unwrap())
            .expect("Second item status not found");

        let first_item_action = ActionWithItemStatus::MakeProgress(first_item_status);
        let first_item_action = WhyInScopeAndActionWithItemStatus::new(
            test_default_mode_why_in_scope(),
            first_item_action,
        );
        let second_item_action = ActionWithItemStatus::MakeProgress(second_item_status);
        let second_item_action = WhyInScopeAndActionWithItemStatus::new(
            test_default_mode_why_in_scope(),
            second_item_action,
        );

        let in_the_moment_priorities = calculated_data.get_in_the_moment_priorities();

        let dut = vec![first_item_action.clone(), second_item_action.clone()];
        let result = dut.apply_in_the_moment_priorities(in_the_moment_priorities);

        assert!(result.is_some());
        let result = result.expect("assert.is_some() should have passed");
        match result {
            UrgencyLevelItemWithItemStatus::SingleItem(result) => {
                assert_eq!(result.get_action(), first_item_action.get_action());
            }
            UrgencyLevelItemWithItemStatus::MultipleItems(..) => panic!("Test Failure"),
        }
    }

    #[test]
    fn apply_in_the_moment_priorities_when_two_items_are_given_and_one_is_the_lowest_in_the_moment_priority_the_other_one_is_returned()
     {
        let first_item = SurrealItemBuilder::default()
            .id(Some(("surreal_item", "1").into()))
            .summary("First item")
            .build()
            .unwrap();
        let second_item = SurrealItemBuilder::default()
            .id(Some(("surreal_item", "2").into()))
            .summary("Second item")
            .build()
            .unwrap();

        let in_an_hour = Utc::now() + chrono::Duration::hours(1);
        let in_the_moment_priority = SurrealInTheMomentPriorityBuilder::default()
            .id(Some(("surreal_in_the_moment_priority", "1").into()))
            .kind(SurrealPriorityKind::LowestPriority)
            .choice(SurrealAction::MakeProgress(
                first_item.id.clone().expect("hard coded to a value"),
            ))
            .not_chosen(vec![SurrealAction::MakeProgress(
                second_item.id.clone().expect("hard coded to a value"),
            )])
            .in_effect_until(vec![SurrealTrigger::WallClockDateTime(in_an_hour.into())])
            .build()
            .unwrap();

        let surreal_tables = SurrealTablesBuilder::default()
            .surreal_items(vec![first_item.clone(), second_item.clone()])
            .surreal_in_the_moment_priorities(vec![in_the_moment_priority])
            .build()
            .unwrap();

        let now = Utc::now();
        let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
        let calculated_data = calculated_data::CalculatedData::new_from_base_data(base_data);
        let items_status = calculated_data.get_items_status();

        let first_item_status = items_status
            .get(first_item.id.as_ref().unwrap())
            .expect("First item status not found");
        let second_item_status = items_status
            .get(second_item.id.as_ref().unwrap())
            .expect("Second item status not found");

        let first_item_action = ActionWithItemStatus::MakeProgress(first_item_status);
        let first_item_action = WhyInScopeAndActionWithItemStatus::new(
            test_default_mode_why_in_scope(),
            first_item_action,
        );
        let second_item_action = ActionWithItemStatus::MakeProgress(second_item_status);
        let second_item_action = WhyInScopeAndActionWithItemStatus::new(
            test_default_mode_why_in_scope(),
            second_item_action,
        );

        let in_the_moment_priorities = calculated_data.get_in_the_moment_priorities();

        let dut = vec![first_item_action.clone(), second_item_action.clone()];
        let result = dut.apply_in_the_moment_priorities(in_the_moment_priorities);

        assert!(result.is_some());
        let result = result.expect("assert.is_some() should have passed");
        match result {
            UrgencyLevelItemWithItemStatus::SingleItem(result) => {
                assert_eq!(result.get_action(), second_item_action.get_action());
            }
            UrgencyLevelItemWithItemStatus::MultipleItems(..) => panic!("Test Failure"),
        }
    }

    #[test]
    fn apply_in_the_moment_priorities_when_three_items_are_given_and_one_is_the_highest_in_the_moment_priority_over_one_other_item_then_the_other_two_are_returned_to_pick_between()
     {
        let first_item = SurrealItemBuilder::default()
            .id(Some(("surreal_item", "1").into()))
            .summary("First item")
            .build()
            .unwrap();
        let second_item = SurrealItemBuilder::default()
            .id(Some(("surreal_item", "2").into()))
            .summary("Second item")
            .build()
            .unwrap();
        let third_item = SurrealItemBuilder::default()
            .id(Some(("surreal_item", "3").into()))
            .summary("Third item")
            .build()
            .unwrap();

        let in_an_hour = Utc::now() + chrono::Duration::hours(1);
        let in_the_moment_priority = SurrealInTheMomentPriorityBuilder::default()
            .id(Some(("surreal_in_the_moment_priority", "1").into()))
            .kind(SurrealPriorityKind::HighestPriority)
            .choice(SurrealAction::MakeProgress(
                first_item.id.clone().expect("hard coded to a value"),
            ))
            .not_chosen(vec![SurrealAction::MakeProgress(
                second_item.id.clone().expect("hard coded to a value"),
            )])
            .in_effect_until(vec![SurrealTrigger::WallClockDateTime(in_an_hour.into())])
            .build()
            .unwrap();

        let surreal_tables = SurrealTablesBuilder::default()
            .surreal_items(vec![
                first_item.clone(),
                second_item.clone(),
                third_item.clone(),
            ])
            .surreal_in_the_moment_priorities(vec![in_the_moment_priority])
            .build()
            .unwrap();

        let now = Utc::now();
        let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
        let calculated_data = calculated_data::CalculatedData::new_from_base_data(base_data);
        let items_status = calculated_data.get_items_status();

        let first_item_status = items_status
            .get(first_item.id.as_ref().unwrap())
            .expect("First item status not found");
        let second_item_status = items_status
            .get(second_item.id.as_ref().unwrap())
            .expect("Second item status not found");
        let third_item_status = items_status
            .get(third_item.id.as_ref().unwrap())
            .expect("Third item status not found");

        let first_item_action = ActionWithItemStatus::MakeProgress(first_item_status);
        let first_item_action = WhyInScopeAndActionWithItemStatus::new(
            test_default_mode_why_in_scope(),
            first_item_action,
        );
        let second_item_action = ActionWithItemStatus::MakeProgress(second_item_status);
        let second_item_action = WhyInScopeAndActionWithItemStatus::new(
            test_default_mode_why_in_scope(),
            second_item_action,
        );
        let third_item_action = ActionWithItemStatus::MakeProgress(third_item_status);
        let third_item_action = WhyInScopeAndActionWithItemStatus::new(
            test_default_mode_why_in_scope(),
            third_item_action,
        );

        let in_the_moment_priorities = calculated_data.get_in_the_moment_priorities();

        let dut = vec![
            first_item_action.clone(),
            second_item_action.clone(),
            third_item_action.clone(),
        ];
        let result = dut.apply_in_the_moment_priorities(in_the_moment_priorities);

        assert!(result.is_some());
        let result = result.expect("assert.is_some() should have passed");
        match result {
            UrgencyLevelItemWithItemStatus::SingleItem(..) => panic!("Test Failure"),
            UrgencyLevelItemWithItemStatus::MultipleItems(vec) => {
                //WhyInScopeAndActionWithItemStatus doesn't support Eq so do the following
                assert_eq!(
                    vec.len(),
                    vec![first_item_action.clone(), third_item_action.clone(),].len()
                );
                assert_eq!(
                    vec[0].get_action(),
                    vec![first_item_action.clone(), third_item_action.clone(),][0].get_action()
                );
                assert_eq!(
                    vec[1].get_action(),
                    vec![first_item_action, third_item_action,][1].get_action()
                );
            }
        }
    }

    #[test]
    fn apply_in_the_moment_priorities_when_three_items_are_given_and_one_is_the_lowest_in_the_moment_priority_over_one_other_item_then_the_other_two_are_returned_to_pick_between()
     {
        let first_item = SurrealItemBuilder::default()
            .id(Some(("surreal_item", "1").into()))
            .summary("First item")
            .build()
            .unwrap();
        let second_item = SurrealItemBuilder::default()
            .id(Some(("surreal_item", "2").into()))
            .summary("Second item")
            .build()
            .unwrap();
        let third_item = SurrealItemBuilder::default()
            .id(Some(("surreal_item", "3").into()))
            .summary("Third item")
            .build()
            .unwrap();

        let in_an_hour = Utc::now() + chrono::Duration::hours(1);
        let in_the_moment_priority = SurrealInTheMomentPriorityBuilder::default()
            .id(Some(("surreal_in_the_moment_priority", "1").into()))
            .kind(SurrealPriorityKind::LowestPriority)
            .choice(SurrealAction::MakeProgress(
                first_item.id.clone().expect("hard coded to a value"),
            ))
            .not_chosen(vec![SurrealAction::MakeProgress(
                second_item.id.clone().expect("hard coded to a value"),
            )])
            .in_effect_until(vec![SurrealTrigger::WallClockDateTime(in_an_hour.into())])
            .build()
            .unwrap();

        let surreal_tables = SurrealTablesBuilder::default()
            .surreal_items(vec![
                first_item.clone(),
                second_item.clone(),
                third_item.clone(),
            ])
            .surreal_in_the_moment_priorities(vec![in_the_moment_priority])
            .build()
            .unwrap();

        let now = Utc::now();
        let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
        let calculated_data = calculated_data::CalculatedData::new_from_base_data(base_data);
        let items_status = calculated_data.get_items_status();

        let first_item_status = items_status
            .get(first_item.id.as_ref().unwrap())
            .expect("First item status not found");
        let second_item_status = items_status
            .get(second_item.id.as_ref().unwrap())
            .expect("Second item status not found");
        let third_item_status = items_status
            .get(third_item.id.as_ref().unwrap())
            .expect("Third item status not found");

        let first_item_action = ActionWithItemStatus::MakeProgress(first_item_status);
        let first_item_action = WhyInScopeAndActionWithItemStatus::new(
            test_default_mode_why_in_scope(),
            first_item_action,
        );
        let second_item_action = ActionWithItemStatus::MakeProgress(second_item_status);
        let second_item_action = WhyInScopeAndActionWithItemStatus::new(
            test_default_mode_why_in_scope(),
            second_item_action,
        );
        let third_item_action = ActionWithItemStatus::MakeProgress(third_item_status);
        let third_item_action = WhyInScopeAndActionWithItemStatus::new(
            test_default_mode_why_in_scope(),
            third_item_action,
        );

        let in_the_moment_priorities = calculated_data.get_in_the_moment_priorities();

        let dut = vec![
            first_item_action.clone(),
            second_item_action.clone(),
            third_item_action.clone(),
        ];
        let result = dut.apply_in_the_moment_priorities(in_the_moment_priorities);

        assert!(result.is_some());
        let result = result.expect("assert.is_some() should have passed");
        match result {
            UrgencyLevelItemWithItemStatus::SingleItem(..) => panic!("Test Failure"),
            UrgencyLevelItemWithItemStatus::MultipleItems(vec) => {
                //WhyInScopeAndActionWithItemStatus doesn't support Eq so do the following
                assert_eq!(
                    vec.len(),
                    vec![second_item_action.clone(), third_item_action.clone(),].len()
                );
                assert!(
                    vec![second_item_action.clone(), third_item_action.clone()]
                        .iter()
                        .any(|x| vec[0].get_action() == x.get_action())
                );
                assert!(
                    vec![second_item_action.clone(), third_item_action.clone()]
                        .iter()
                        .any(|x| vec[1].get_action() == x.get_action())
                );
            }
        }
    }

    #[test]
    fn apply_in_the_moment_priorities_when_three_items_are_given_and_one_is_the_highest_priority_over_one_other_and_the_third_is_the_lowest_priority_over_the_one_other_then_the_highest_priority_is_returned()
     {
        let first_item = SurrealItemBuilder::default()
            .id(Some(("surreal_item", "1").into()))
            .summary("First item")
            .build()
            .unwrap();
        let second_item = SurrealItemBuilder::default()
            .id(Some(("surreal_item", "2").into()))
            .summary("Second item")
            .build()
            .unwrap();
        let third_item = SurrealItemBuilder::default()
            .id(Some(("surreal_item", "3").into()))
            .summary("Third item")
            .build()
            .unwrap();

        let in_an_hour = Utc::now() + chrono::Duration::hours(1);
        let highest_in_the_moment_priority = SurrealInTheMomentPriorityBuilder::default()
            .id(Some(("surreal_in_the_moment_priority", "1").into()))
            .kind(SurrealPriorityKind::HighestPriority)
            .choice(SurrealAction::MakeProgress(
                first_item.id.clone().expect("hard coded to a value"),
            ))
            .not_chosen(vec![SurrealAction::MakeProgress(
                second_item.id.clone().expect("hard coded to a value"),
            )])
            .in_effect_until(vec![SurrealTrigger::WallClockDateTime(in_an_hour.into())])
            .build()
            .unwrap();
        let lowest_in_the_moment_priority = SurrealInTheMomentPriorityBuilder::default()
            .id(Some(("surreal_in_the_moment_priority", "3").into()))
            .kind(SurrealPriorityKind::LowestPriority)
            .choice(SurrealAction::MakeProgress(
                third_item.id.clone().expect("hard coded to a value"),
            ))
            .not_chosen(vec![SurrealAction::MakeProgress(
                first_item.id.clone().expect("hard coded to a value"),
            )])
            .in_effect_until(vec![SurrealTrigger::WallClockDateTime(in_an_hour.into())])
            .build()
            .unwrap();

        let surreal_tables = SurrealTablesBuilder::default()
            .surreal_items(vec![
                first_item.clone(),
                second_item.clone(),
                third_item.clone(),
            ])
            .surreal_in_the_moment_priorities(vec![
                highest_in_the_moment_priority,
                lowest_in_the_moment_priority,
            ])
            .build()
            .unwrap();

        let now = Utc::now();
        let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
        let calculated_data = calculated_data::CalculatedData::new_from_base_data(base_data);
        let items_status = calculated_data.get_items_status();

        let first_item_status = items_status
            .get(first_item.id.as_ref().unwrap())
            .expect("First item status not found");
        let second_item_status = items_status
            .get(second_item.id.as_ref().unwrap())
            .expect("Second item status not found");
        let third_item_status = items_status
            .get(third_item.id.as_ref().unwrap())
            .expect("Third item status not found");

        let first_item_action = ActionWithItemStatus::MakeProgress(first_item_status);
        let first_item_action = WhyInScopeAndActionWithItemStatus::new(
            test_default_mode_why_in_scope(),
            first_item_action,
        );
        let second_item_action = ActionWithItemStatus::MakeProgress(second_item_status);
        let second_item_action = WhyInScopeAndActionWithItemStatus::new(
            test_default_mode_why_in_scope(),
            second_item_action,
        );
        let third_item_action = ActionWithItemStatus::MakeProgress(third_item_status);
        let third_item_action = WhyInScopeAndActionWithItemStatus::new(
            test_default_mode_why_in_scope(),
            third_item_action,
        );

        let in_the_moment_priorities = calculated_data.get_in_the_moment_priorities();

        let dut = vec![
            first_item_action.clone(),
            second_item_action.clone(),
            third_item_action.clone(),
        ];
        let result = dut.apply_in_the_moment_priorities(in_the_moment_priorities);

        assert!(result.is_some());
        let result = result.expect("assert.is_some() should have passed");
        match result {
            UrgencyLevelItemWithItemStatus::SingleItem(result) => {
                assert_eq!(result.get_action(), first_item_action.get_action());
            }
            UrgencyLevelItemWithItemStatus::MultipleItems(..) => panic!("Test Failure"),
        }
    }

    #[test]
    fn apply_in_the_moment_priorities_when_the_priority_is_out_of_mode_and_lowest_priority_the_priority_should_not_apply()
     {
        let first_item = SurrealItemBuilder::default()
            .id(Some(("surreal_item", "1").into()))
            .summary("First item")
            .build()
            .unwrap();
        let second_item = SurrealItemBuilder::default()
            .id(Some(("surreal_item", "2").into()))
            .summary("Second item")
            .build()
            .unwrap();
        let third_item = SurrealItemBuilder::default()
            .id(Some(("surreal_item", "3").into()))
            .summary("Third item")
            .build()
            .unwrap();

        let core_motivation = SurrealItemBuilder::default()
            .id(Some(("surreal_item", "core").into()))
            .item_type(SurrealItemType::Motivation(SurrealMotivationKind::CoreWork))
            .smaller_items_in_priority_order(vec![
                SurrealOrderedSubItem::SubItem {
                    surreal_item_id: ("surreal_item", "1").into(),
                },
                SurrealOrderedSubItem::SubItem {
                    surreal_item_id: ("surreal_item", "2").into(),
                },
            ])
            .summary("Core motivation")
            .build()
            .unwrap();

        let non_core_motivation = SurrealItemBuilder::default()
            .id(Some(("surreal_item", "noncore").into()))
            .item_type(SurrealItemType::Motivation(
                SurrealMotivationKind::NonCoreWork,
            ))
            .smaller_items_in_priority_order(vec![SurrealOrderedSubItem::SubItem {
                surreal_item_id: ("surreal_item", "3").into(),
            }])
            .summary("Core motivation")
            .build()
            .unwrap();

        let in_an_hour = Utc::now() + chrono::Duration::hours(1);
        let lowest_in_the_moment_priority = SurrealInTheMomentPriorityBuilder::default()
            .id(Some(("surreal_in_the_moment_priority", "1").into()))
            .kind(SurrealPriorityKind::LowestPriority)
            .choice(SurrealAction::MakeProgress(
                third_item.id.clone().expect("hard coded to a value"),
            ))
            .not_chosen(vec![
                SurrealAction::MakeProgress(first_item.id.clone().expect("hard coded to a value")),
                SurrealAction::MakeProgress(second_item.id.clone().expect("hard coded to a value")),
            ])
            .in_effect_until(vec![SurrealTrigger::WallClockDateTime(in_an_hour.into())])
            .build()
            .unwrap();

        let surreal_tables = SurrealTablesBuilder::default()
            .surreal_items(vec![
                first_item.clone(),
                second_item.clone(),
                third_item.clone(),
                core_motivation.clone(),
                non_core_motivation.clone(),
            ])
            .surreal_in_the_moment_priorities(vec![lowest_in_the_moment_priority])
            .build()
            .unwrap();

        let now = Utc::now();
        let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
        let calculated_data = calculated_data::CalculatedData::new_from_base_data(base_data);
        let items_status = calculated_data.get_items_status();

        let first_item_status = items_status
            .get(first_item.id.as_ref().unwrap())
            .expect("First item status not found");
        let second_item_status = items_status
            .get(second_item.id.as_ref().unwrap())
            .expect("Second item status not found");

        //We are just doing core motivation is in mode which is limited to the first and second item

        let first_item_action = ActionWithItemStatus::MakeProgress(first_item_status);
        let first_item_action = WhyInScopeAndActionWithItemStatus::new(
            test_default_mode_why_in_scope(),
            first_item_action,
        );
        let second_item_action = ActionWithItemStatus::MakeProgress(second_item_status);
        let second_item_action = WhyInScopeAndActionWithItemStatus::new(
            test_default_mode_why_in_scope(),
            second_item_action,
        );

        let in_the_moment_priorities = calculated_data.get_in_the_moment_priorities();

        let dut = vec![first_item_action.clone(), second_item_action.clone()];
        let result = dut.apply_in_the_moment_priorities(in_the_moment_priorities);

        assert!(result.is_some(), "Result should not be blank");
        let result = result.expect("assert.is_some() should have passed");
        match result {
            UrgencyLevelItemWithItemStatus::SingleItem(..) => {
                panic!("Both items should still be there");
            }
            UrgencyLevelItemWithItemStatus::MultipleItems(results) => {
                assert_eq!(results.len(), 2);
                assert_eq!(results[0].get_action(), first_item_action.get_action());
                assert_eq!(results[1].get_action(), second_item_action.get_action());
            }
        }
    }

    #[test]
    fn apply_in_the_moment_priorities_when_the_priority_is_out_of_mode_and_highest_priority_the_priority_should_not_apply()
     {
        let first_item = SurrealItemBuilder::default()
            .id(Some(("surreal_item", "1").into()))
            .summary("First item")
            .build()
            .unwrap();
        let second_item = SurrealItemBuilder::default()
            .id(Some(("surreal_item", "2").into()))
            .summary("Second item")
            .build()
            .unwrap();
        let third_item = SurrealItemBuilder::default()
            .id(Some(("surreal_item", "3").into()))
            .summary("Third item")
            .build()
            .unwrap();

        let core_motivation = SurrealItemBuilder::default()
            .id(Some(("surreal_item", "core").into()))
            .item_type(SurrealItemType::Motivation(SurrealMotivationKind::CoreWork))
            .smaller_items_in_priority_order(vec![
                SurrealOrderedSubItem::SubItem {
                    surreal_item_id: ("surreal_item", "1").into(),
                },
                SurrealOrderedSubItem::SubItem {
                    surreal_item_id: ("surreal_item", "2").into(),
                },
            ])
            .summary("Core motivation")
            .build()
            .unwrap();

        let non_core_motivation = SurrealItemBuilder::default()
            .id(Some(("surreal_item", "noncore").into()))
            .item_type(SurrealItemType::Motivation(
                SurrealMotivationKind::NonCoreWork,
            ))
            .smaller_items_in_priority_order(vec![SurrealOrderedSubItem::SubItem {
                surreal_item_id: ("surreal_item", "3").into(),
            }])
            .summary("Core motivation")
            .build()
            .unwrap();

        let in_an_hour = Utc::now() + chrono::Duration::hours(1);
        let highest_in_the_moment_priority = SurrealInTheMomentPriorityBuilder::default()
            .id(Some(("surreal_in_the_moment_priority", "1").into()))
            .kind(SurrealPriorityKind::HighestPriority)
            .choice(SurrealAction::MakeProgress(
                third_item.id.clone().expect("hard coded to a value"),
            ))
            .not_chosen(vec![
                SurrealAction::MakeProgress(first_item.id.clone().expect("hard coded to a value")),
                SurrealAction::MakeProgress(second_item.id.clone().expect("hard coded to a value")),
            ])
            .in_effect_until(vec![SurrealTrigger::WallClockDateTime(in_an_hour.into())])
            .build()
            .unwrap();

        let surreal_tables = SurrealTablesBuilder::default()
            .surreal_items(vec![
                first_item.clone(),
                second_item.clone(),
                third_item.clone(),
                core_motivation.clone(),
                non_core_motivation.clone(),
            ])
            .surreal_in_the_moment_priorities(vec![highest_in_the_moment_priority])
            .build()
            .unwrap();

        let now = Utc::now();
        let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
        let calculated_data = calculated_data::CalculatedData::new_from_base_data(base_data);
        let items_status = calculated_data.get_items_status();

        let first_item_status = items_status
            .get(first_item.id.as_ref().unwrap())
            .expect("First item status not found");
        let second_item_status = items_status
            .get(second_item.id.as_ref().unwrap())
            .expect("Second item status not found");

        //We are just doing core motivation is in mode which is limited to the first and second item

        let first_item_action = ActionWithItemStatus::MakeProgress(first_item_status);
        let first_item_action = WhyInScopeAndActionWithItemStatus::new(
            test_default_mode_why_in_scope(),
            first_item_action,
        );
        let second_item_action = ActionWithItemStatus::MakeProgress(second_item_status);
        let second_item_action = WhyInScopeAndActionWithItemStatus::new(
            test_default_mode_why_in_scope(),
            second_item_action,
        );

        let in_the_moment_priorities = calculated_data.get_in_the_moment_priorities();

        let dut = vec![first_item_action.clone(), second_item_action.clone()];
        let result = dut.apply_in_the_moment_priorities(in_the_moment_priorities);

        assert!(result.is_some(), "Result should not be blank");
        let result = result.expect("assert.is_some() should have passed");
        match result {
            UrgencyLevelItemWithItemStatus::SingleItem(..) => {
                panic!("Both items should still be there");
            }
            UrgencyLevelItemWithItemStatus::MultipleItems(results) => {
                assert_eq!(results.len(), 2);
                assert_eq!(results[0].get_action(), first_item_action.get_action());
                assert_eq!(results[1].get_action(), second_item_action.get_action());
            }
        }
    }
}
