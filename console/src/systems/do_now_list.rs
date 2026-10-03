use ahash::{HashMap, HashSet};
use chrono::{DateTime, Utc};
use current_mode::CurrentMode;
use ouroboros::self_referencing;
use surrealdb::RecordId;

pub(crate) mod current_mode;
pub(crate) mod current_mode_node;

use crate::{
    base_data::{BaseData, time_spent::TimeSpent},
    calculated_data::CalculatedData,
    data_storage::surrealdb_layer::surreal_item::SurrealUrgency,
    node::{
        Filter,
        action_with_item_status::{ActionWithItemStatus, WhyInScopeActionListsByUrgency},
        event_node::EventNode,
        item_status::ItemStatus,
        urgency_level_item_with_item_status::UrgencyLevelItemWithItemStatus,
        why_in_scope_and_action_with_item_status::{WhyInScope, WhyInScopeAndActionWithItemStatus},
    },
    systems::{do_now_list::current_mode_node::CurrentModeNode, upcoming::Upcoming},
};

#[self_referencing]
pub(crate) struct DoNowList {
    calculated_data: CalculatedData,

    #[borrows(calculated_data)]
    #[covariant]
    ordered_do_now_list: Vec<UrgencyLevelItemWithItemStatus<'this>>,

    #[borrows(calculated_data)]
    #[covariant]
    upcoming: Upcoming<'this>,
}

impl DoNowList {
    pub(crate) fn new_do_now_list(
        calculated_data: CalculatedData,
        current_time: &DateTime<Utc>,
    ) -> Self {
        DoNowListBuilder {
            calculated_data,
            ordered_do_now_list_builder: |calculated_data| {
                //Get all top level items
                let everything_that_has_no_parent = calculated_data
                    .get_items_status()
                    .values()
                    .filter(|x| !x.has_parents(Filter::Active) && x.is_active())
                    .collect::<Vec<_>>();

                let all_items_status = calculated_data.get_items_status();
                let current_mode = calculated_data.get_current_mode();
                let most_important_items = everything_that_has_no_parent
                    .iter()
                    .filter(|x| current_mode.is_importance_in_the_mode(x.get_item_node()))
                    .filter_map(|x| x.recursive_get_most_important_and_ready(all_items_status))
                    .map(ActionWithItemStatus::MakeProgress)
                    .filter(|action| current_mode.is_importance_in_the_mode(action.get_item_node()))
                    .map(|action| {
                        let mut why_in_scope = HashSet::default();
                        why_in_scope.insert(WhyInScope::Importance);
                        WhyInScopeAndActionWithItemStatus::new(why_in_scope, action)
                    });
                let urgent_items = everything_that_has_no_parent
                    .iter()
                    .flat_map(|x| {
                        x.recursive_get_urgent_bullet_list(all_items_status, Vec::default())
                    })
                    .filter(|action| current_mode.is_urgency_in_the_mode(action.get_item_node()))
                    .map(|action| {
                        let mut why_in_scope = HashSet::default();
                        why_in_scope.insert(WhyInScope::Urgency);
                        WhyInScopeAndActionWithItemStatus::new(why_in_scope, action)
                    });

                let items = most_important_items.chain(urgent_items).fold(
                    HashSet::default(),
                    |mut acc: HashSet<WhyInScopeAndActionWithItemStatus>,
                     x: WhyInScopeAndActionWithItemStatus| {
                        match HashSet::take(&mut acc, &x) {
                            Some(mut existing) => {
                                existing.extend_why_in_scope(x.get_why_in_scope());
                                acc.insert(existing);
                            }
                            None => {
                                acc.insert(x);
                            }
                        }
                        acc
                    },
                );

                let mut bullet_lists_by_urgency = WhyInScopeActionListsByUrgency::default();

                for item in items.iter().filter(|x| x.is_in_scope_for_importance()) {
                    bullet_lists_by_urgency
                        .in_the_mode_maybe_urgent_and_by_importance
                        .push_if_new(item.clone());
                }

                for item in items.into_iter() {
                    match item.get_urgency_now() {
                        SurrealUrgency::MoreUrgentThanAnythingIncludingScheduled => {
                            bullet_lists_by_urgency
                                .more_urgent_than_anything_including_scheduled
                                .push_if_new(item);
                        }
                        SurrealUrgency::ScheduledAnyMode(_) => {
                            bullet_lists_by_urgency.scheduled_any_mode.push_if_new(item);
                        }
                        SurrealUrgency::MoreUrgentThanMode => {
                            bullet_lists_by_urgency
                                .more_urgent_than_mode
                                .push_if_new(item);
                        }
                        SurrealUrgency::InTheModeScheduled(_) => {
                            if current_mode.is_urgency_in_the_mode(item.get_item_node()) {
                                bullet_lists_by_urgency
                                    .in_the_mode_scheduled
                                    .push_if_new(item);
                            }
                        }
                        SurrealUrgency::InTheModeDefinitelyUrgent => {
                            if current_mode.is_urgency_in_the_mode(item.get_item_node()) {
                                bullet_lists_by_urgency
                                    .in_the_mode_definitely_urgent
                                    .push_if_new(item);
                            }
                        }
                        SurrealUrgency::InTheModeMaybeUrgent
                        | SurrealUrgency::InTheModeByImportance => {
                            if current_mode.is_urgency_in_the_mode(item.get_item_node()) {
                                bullet_lists_by_urgency
                                    .in_the_mode_maybe_urgent_and_by_importance
                                    .push_if_new(item);
                            }
                        }
                    }
                }

                let all_priorities = calculated_data.get_in_the_moment_priorities();

                bullet_lists_by_urgency.apply_in_the_moment_priorities(
                    all_priorities,
                    calculated_data.get_current_mode().get_mode_id(),
                )
            },
            upcoming_builder: |calculated_data| Upcoming::new(calculated_data, current_time),
        }
        .build()
    }

    pub(crate) fn get_ordered_do_now_list(&self) -> &[UrgencyLevelItemWithItemStatus<'_>] {
        self.borrow_ordered_do_now_list()
    }

    pub(crate) fn get_all_items_status(&self) -> &HashMap<&RecordId, ItemStatus<'_>> {
        self.borrow_calculated_data().get_items_status()
    }

    pub(crate) fn get_upcoming(&self) -> &Upcoming<'_> {
        self.borrow_upcoming()
    }

    pub(crate) fn get_now(&self) -> &DateTime<Utc> {
        self.borrow_calculated_data().get_now()
    }

    pub(crate) fn get_time_spent_log(&self) -> &[TimeSpent<'_>] {
        self.borrow_calculated_data().get_time_spent_log()
    }

    pub(crate) fn get_current_mode(&self) -> &CurrentMode {
        self.borrow_calculated_data().get_current_mode()
    }

    pub(crate) fn get_current_mode_node<'a>(&'a self) -> &'a CurrentModeNode<'a> {
        self.borrow_calculated_data().get_current_mode_node()
    }

    pub(crate) fn get_event_nodes(&self) -> &HashMap<&RecordId, EventNode<'_>> {
        self.borrow_calculated_data().get_event_nodes()
    }

    pub(crate) fn get_base_data(&self) -> &BaseData {
        self.borrow_calculated_data().get_base_data()
    }
}

trait PushIfNew<'t> {
    fn push_if_new(&mut self, item: WhyInScopeAndActionWithItemStatus<'t>);
}

impl<'t> PushIfNew<'t> for Vec<WhyInScopeAndActionWithItemStatus<'t>> {
    fn push_if_new(&mut self, item: WhyInScopeAndActionWithItemStatus<'t>) {
        match self.iter().find(|x| x.get_action() == item.get_action()) {
            None => {
                self.push(item);
            }
            Some(x) => {
                //Do nothing, Item is already there
                if item.get_why_in_scope() != x.get_why_in_scope() {
                    println!("item: {:?}", item);
                    println!("x: {:?}", x);
                }
                assert!(item.get_why_in_scope() == x.get_why_in_scope());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use crate::{
        base_data::BaseData,
        calculated_data::CalculatedData,
        data_storage::surrealdb_layer::{
            surreal_current_mode::SurrealCurrentMode,
            surreal_item::{
                SurrealItemBuilder, SurrealItemModeScope, SurrealItemType, SurrealUrgency,
                SurrealUrgencyPlan,
            },
            surreal_mode::SurrealMode,
            surreal_tables::SurrealTablesBuilder,
        },
        node::urgency_level_item_with_item_status::UrgencyLevelItemWithItemStatus,
    };

    use super::DoNowList;

    #[test]
    fn do_now_list_excludes_items_not_in_current_mode_scope() {
        let work_mode_id: surrealdb::RecordId = "modes:work".parse().unwrap();
        let play_mode_id: surrealdb::RecordId = "modes:play".parse().unwrap();

        let all_modes_item = SurrealItemBuilder::default()
            .id(Some(("surreal_item", "all").into()))
            .summary("All Modes Item")
            .item_type(SurrealItemType::Action)
            .mode_scope(SurrealItemModeScope::AllModes)
            .build()
            .unwrap();

        let only_play_item = SurrealItemBuilder::default()
            .id(Some(("surreal_item", "only_play").into()))
            .summary("Only Play Item")
            .item_type(SurrealItemType::Action)
            .mode_scope(SurrealItemModeScope::OnlyModes(vec![play_mode_id.clone()]))
            .build()
            .unwrap();

        let except_work_item = SurrealItemBuilder::default()
            .id(Some(("surreal_item", "except_work").into()))
            .summary("Except Work Item")
            .item_type(SurrealItemType::Action)
            .mode_scope(SurrealItemModeScope::ExceptModes(vec![work_mode_id.clone()]))
            .build()
            .unwrap();

        let surreal_tables = SurrealTablesBuilder::default()
            .surreal_items(vec![all_modes_item.clone(), only_play_item, except_work_item])
            .surreal_modes(vec![
                SurrealMode {
                    id: Some(work_mode_id.clone()),
                    name: "Work".to_string(),
                    version: 0,
                    parent: None,
                },
                SurrealMode {
                    id: Some(play_mode_id),
                    name: "Play".to_string(),
                    version: 0,
                    parent: None,
                },
            ])
            .surreal_current_modes(vec![SurrealCurrentMode {
                id: Some(("current_modes", "current_mode").into()),
                version: 0,
                current_mode: Some(work_mode_id),
            }])
            .build()
            .unwrap();

        let now = Utc::now();
        let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
        let calculated_data = CalculatedData::new_from_base_data(base_data);
        let do_now = DoNowList::new_do_now_list(calculated_data, &now);

        let summaries = do_now
            .get_ordered_do_now_list()
            .iter()
            .flat_map(|entry| match entry {
                UrgencyLevelItemWithItemStatus::SingleItem(item) => {
                    vec![item.get_action().get_item_node().get_item().get_summary().to_string()]
                }
                UrgencyLevelItemWithItemStatus::MultipleItems(items) => items
                    .iter()
                    .map(|item| item.get_action().get_item_node().get_item().get_summary().to_string())
                    .collect::<Vec<_>>(),
            })
            .collect::<Vec<_>>();

        assert!(summaries.iter().any(|x| x == "All Modes Item"));
        assert!(!summaries.iter().any(|x| x == "Only Play Item"));
        assert!(!summaries.iter().any(|x| x == "Except Work Item"));

        assert_eq!(do_now.get_current_mode().get_name(), "Work");
        assert_eq!(all_modes_item.mode_scope, SurrealItemModeScope::AllModes);
    }

    #[test]
    fn do_now_list_includes_mode_excluded_item_when_urgency_overrides_mode() {
        let work_mode_id: surrealdb::RecordId = "modes:work".parse().unwrap();

        let mode_excluded_but_overriding_urgency = SurrealItemBuilder::default()
            .id(Some(("surreal_item", "override_urgency").into()))
            .summary("Override urgency item")
            .item_type(SurrealItemType::Action)
            .mode_scope(SurrealItemModeScope::ExceptModes(vec![work_mode_id.clone()]))
            .urgency_plan(Some(SurrealUrgencyPlan::StaysTheSame(
                SurrealUrgency::MoreUrgentThanMode,
            )))
            .build()
            .unwrap();

        let surreal_tables = SurrealTablesBuilder::default()
            .surreal_items(vec![mode_excluded_but_overriding_urgency])
            .surreal_modes(vec![SurrealMode {
                id: Some(work_mode_id.clone()),
                name: "Work".to_string(),
                version: 0,
                parent: None,
            }])
            .surreal_current_modes(vec![SurrealCurrentMode {
                id: Some(("current_modes", "current_mode").into()),
                version: 0,
                current_mode: Some(work_mode_id),
            }])
            .build()
            .unwrap();

        let now = Utc::now();
        let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
        let calculated_data = CalculatedData::new_from_base_data(base_data);
        let do_now = DoNowList::new_do_now_list(calculated_data, &now);

        let summaries = do_now
            .get_ordered_do_now_list()
            .iter()
            .flat_map(|entry| match entry {
                UrgencyLevelItemWithItemStatus::SingleItem(item) => {
                    vec![item.get_action().get_item_node().get_item().get_summary().to_string()]
                }
                UrgencyLevelItemWithItemStatus::MultipleItems(items) => items
                    .iter()
                    .map(|item| item.get_action().get_item_node().get_item().get_summary().to_string())
                    .collect::<Vec<_>>(),
            })
            .collect::<Vec<_>>();

        assert!(summaries.iter().any(|x| x == "Override urgency item"));
    }
}
