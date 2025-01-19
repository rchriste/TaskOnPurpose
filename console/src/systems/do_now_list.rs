use ahash::{HashMap, HashSet};
use chrono::{DateTime, Utc};
use current_mode::CurrentMode;
use ouroboros::self_referencing;
use surrealdb::opt::RecordId;

pub(crate) mod current_mode;
use crate::{
    base_data::{event::Event, mode::ModeCategory, time_spent::TimeSpent, BaseData},
    calculated_data::CalculatedData,
    data_storage::surrealdb_layer::surreal_item::SurrealUrgency,
    node::{
        Filter,
        action_with_item_status::{ActionWithItemStatus, WhyInScopeActionListsByUrgency},
        item_status::ItemStatus,
        urgency_level_item_with_item_status::UrgencyLevelItemWithItemStatus,
        why_in_scope_and_action_with_item_status::{WhyInScope, WhyInScopeAndActionWithItemStatus},
    },
    systems::{do_now_list::current_mode::IsInTheMode, upcoming::Upcoming},
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
                    .filter_map(|x| {
                        match current_mode.get_category_by_importance(x.get_item_node()) {
                            ModeCategory::Core | ModeCategory::NonCore => x
                                .recursive_get_most_important_and_ready(all_items_status)
                                .map(ActionWithItemStatus::MakeProgress),
                            ModeCategory::OutOfScope => None,
                            ModeCategory::NotDeclared { item_to_specify } => {
                                let item_status = all_items_status
                                    .get(item_to_specify)
                                    .expect("Item must exist");
                                let mode_node = current_mode.as_ref().expect(
                                    "This path will only be selected if there is a current mode",
                                ).get_mode();
                                Some(ActionWithItemStatus::StateIfInMode(item_status, mode_node))
                            }
                        }
                    })
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
                        .maybe_urgent_and_by_importance
                        .push_if_new(item.clone());
                }

                for item in items.into_iter() {
                    match item.get_urgency_now() {
                        Some(SurrealUrgency::CrisesUrgent(modes_in_scope)) => {
                            push_to_urgency_bullet_list(
                                item,
                                current_mode,
                                &mut bullet_lists_by_urgency.crises_urgency,
                                all_items_status,
                            );
                        }
                        Some(SurrealUrgency::Scheduled(modes_in_scope, _)) => {
                            push_to_urgency_bullet_list(
                                item,
                                current_mode,
                                &mut bullet_lists_by_urgency.scheduled,
                                all_items_status,
                            );
                        }
                        Some(SurrealUrgency::DefinitelyUrgent(modes_in_scope)) => {
                            push_to_urgency_bullet_list(
                                item,
                                current_mode,
                                &mut bullet_lists_by_urgency.definitely_urgent,
                                all_items_status,
                            );
                        }
                        Some(SurrealUrgency::MaybeUrgent(modes_in_scope)) => {
                            push_to_urgency_bullet_list(
                                item,
                                current_mode,
                                &mut bullet_lists_by_urgency.maybe_urgent_and_by_importance,
                                all_items_status,
                            );
                        }
                        None => {
                            //Do nothing
                        }
                    }
                }

                let all_priorities = calculated_data.get_in_the_moment_priorities();

                bullet_lists_by_urgency.apply_in_the_moment_priorities(all_priorities)
            },
            upcoming_builder: |calculated_data| Upcoming::new(calculated_data, current_time),
        }
        .build()
    }

    pub(crate) fn get_calculated_data(&self) -> &CalculatedData {
        self.borrow_calculated_data()
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

    pub(crate) fn get_current_mode(&self) -> &Option<CurrentMode> {
        self.borrow_calculated_data().get_current_mode()
    }

    pub(crate) fn get_events(&self) -> &HashMap<&RecordId, Event<'_>> {
        self.borrow_calculated_data().get_events()
    }
}

fn push_to_urgency_bullet_list<'a>(
    item: WhyInScopeAndActionWithItemStatus<'a>,
    current_mode: &'a Option<CurrentMode>,
    urgency_list: &mut Vec<WhyInScopeAndActionWithItemStatus<'a>>,
    all_items_status: &'a HashMap<&RecordId, ItemStatus<'a>>,
) {
    match current_mode.get_category_by_urgency(&item) {
        ModeCategory::Core | ModeCategory::NonCore => {
            urgency_list.push_if_new(item);
        }
        ModeCategory::OutOfScope => {
            //Do nothing
        }
        ModeCategory::NotDeclared { item_to_specify } => {
            let item_status = all_items_status
                .get(item_to_specify)
                .expect("Item must exist");
            let mode_node = current_mode
                .as_ref()
                .expect("This path will only be selected if there is a current mode")
                .get_mode();
            let mut why_in_scope = HashSet::default();
            why_in_scope.insert(WhyInScope::Urgency);

            urgency_list.push_if_new(WhyInScopeAndActionWithItemStatus::new(
                why_in_scope,
                ActionWithItemStatus::StateIfInMode(item_status, mode_node),
            ));
        }
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
