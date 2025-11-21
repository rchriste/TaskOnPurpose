use ahash::{HashMap, HashSet};
use chrono::{DateTime, Utc};
use current_mode::CurrentMode;
use ouroboros::self_referencing;
use surrealdb::opt::RecordId;

pub(crate) mod current_mode;
use crate::{
    base_data::{event::Event, mode::ModeCategory, time_spent::TimeSpent},
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

    pub(crate) fn get_current_mode(&self) -> &Option<CurrentMode<'_>> {
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

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use tokio::sync::mpsc;

    use crate::{
        base_data::BaseData,
        base_data::mode::ModeCategory,
        calculated_data::CalculatedData,
        data_storage::surrealdb_layer::{
            data_layer_commands::{DataLayerCommands, data_storage_start_and_run},
            surreal_current_mode::NewCurrentMode,
            surreal_item::{SurrealItemType, SurrealUrgencyPlan},
            surreal_mode::SurrealMode,
            surreal_tables::SurrealTables,
        },
        new_item::NewItemBuilder,
        new_mode::NewModeBuilder,
        node::Filter,
        node::urgency_level_item_with_item_status::UrgencyLevelItemWithItemStatus,
        systems::do_now_list::DoNowList,
    };

    /// Scenario:
    /// - Motivation "test motivation"
    /// - Child step "test step", marked as:
    ///   - smaller item of the motivation with importance scope AllModes
    ///   - Ready now
    ///   - Not urgent
    /// - Mode: none selected
    /// Expectation:
    /// - "test step" appears on the Do Now list as available to work on.
    #[tokio::test]
    async fn test_step_with_ready_now_and_not_urgent_shows_up_on_do_now_list_without_mode() {
        // Arrange DB and data storage layer
        let (sender, receiver) = mpsc::channel(1);
        let data_storage_join_handle =
            tokio::spawn(async move { data_storage_start_and_run(receiver, "mem://").await });

        let now = Utc::now();

        // Create the parent motivation item.
        sender
            .send(DataLayerCommands::NewItem(
                NewItemBuilder::default()
                    .summary("test motivation")
                    .item_type(SurrealItemType::Motivation)
                    .build()
                    .expect("Valid motivation"),
            ))
            .await
            .expect("Should send motivation");

        // Create the child "test step" item with:
        // - Ready now (finished = None)
        // - Not urgent (no urgency plan)
        sender
            .send(DataLayerCommands::NewItem(
                NewItemBuilder::default()
                    .summary("test step")
                    .item_type(SurrealItemType::Action)
                    .urgency_plan(Some(SurrealUrgencyPlan::StaysTheSame(None)))
                    .build()
                    .expect("Valid step"),
            ))
            .await
            .expect("Should send step");

        // Load tables so we can look up the created items.
        let surreal_tables = SurrealTables::new(&sender)
            .await
            .expect("Should load initial tables");

        // Look up the created items to get their record IDs.
        let motivation_id = surreal_tables
            .surreal_items
            .iter()
            .find(|item| item.summary == "test motivation")
            .expect("Motivation should exist")
            .id
            .as_ref()
            .expect("Motivation must have id")
            .clone();
        let step_id = surreal_tables
            .surreal_items
            .iter()
            .find(|item| item.summary == "test step")
            .expect("Step should exist")
            .id
            .as_ref()
            .expect("Step must have id")
            .clone();

        // Use the same data-layer command interface to declare that
        // "test step" is a smaller item of "test motivation" with
        // importance scope AllModes.
        sender
            .send(DataLayerCommands::ParentItemWithExistingItem {
                child: step_id,
                parent: motivation_id,
                // None means "at the end of the list"; the importance scope
                // for the smaller item itself is encoded in the SurrealImportance
                // that the data layer will create.
                higher_importance_than_this: None,
            })
            .await
            .expect("Should send parent-child relationship command");

        // Reload tables so the parent-child relationship is reflected.
        let surreal_tables = SurrealTables::new(&sender)
            .await
            .expect("Should load updated tables");

        // No mode is created or selected in this scenario.

        // Build BaseData and CalculatedData from the adjusted tables.
        let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
        let calculated_data = CalculatedData::new_from_base_data(base_data);

        // Act: build the Do Now list with no current mode.
        let do_now_list = DoNowList::new_do_now_list(calculated_data, &now);
        let ordered = do_now_list.get_ordered_do_now_list();

        // Assert: there is at least one item in the Do Now list and
        // "test step" is among them.
        assert!(
            !ordered.is_empty(),
            "Do Now list should not be empty when a ready, non-urgent step exists"
        );

        let mut found_test_step = false;
        for entry in ordered {
            match entry {
                UrgencyLevelItemWithItemStatus::SingleItem(why) => {
                    if why.get_action().get_item_node().get_item().get_summary() == "test step" {
                        found_test_step = true;
                        break;
                    }
                }
                UrgencyLevelItemWithItemStatus::MultipleItems(list) => {
                    if list.iter().any(|why| {
                        why.get_action().get_item_node().get_item().get_summary() == "test step"
                    }) {
                        found_test_step = true;
                        break;
                    }
                }
            }
        }

        assert!(
            found_test_step,
            "Expected 'test step' to appear on the Do Now list"
        );

        drop(sender);
        data_storage_join_handle
            .await
            .expect("Data storage loop should exit");
    }

    /// Scenario:
    /// - Motivation "test motivation"
    /// - Child step "test step" as in the previous test
    /// - Mode "Test Mode" where "test motivation" is explicitly out of scope
    /// Expectation (current behavior to expose a bug):
    /// - Building the Do Now list should hit the assertion in
    ///   `action_with_item_status.rs` that complains about all choices
    ///   being removed; for now we encode this as a should_panic test.
    #[tokio::test]
    async fn test_step_hidden_when_parent_motivation_is_explicitly_out_of_scope_for_mode() {
        // Arrange DB and data storage layer
        let (sender, receiver) = mpsc::channel(1);
        let data_storage_join_handle =
            tokio::spawn(async move { data_storage_start_and_run(receiver, "mem://").await });

        let now = Utc::now();

        // Create the parent motivation item.
        sender
            .send(DataLayerCommands::NewItem(
                NewItemBuilder::default()
                    .summary("test motivation")
                    .item_type(SurrealItemType::Motivation)
                    .build()
                    .expect("Valid motivation"),
            ))
            .await
            .expect("Should send motivation");

        // Create the child "test step" item with:
        // - Ready now (finished = None)
        // - Not urgent (no urgency plan)
        sender
            .send(DataLayerCommands::NewItem(
                NewItemBuilder::default()
                    .summary("test step")
                    .item_type(SurrealItemType::Action)
                    .urgency_plan(Some(SurrealUrgencyPlan::StaysTheSame(None)))
                    .build()
                    .expect("Valid step"),
            ))
            .await
            .expect("Should send step");

        // Load tables so we can look up the created items.
        let surreal_tables = SurrealTables::new(&sender)
            .await
            .expect("Should load initial tables");

        // Look up the created items to get their record IDs.
        let motivation_id = surreal_tables
            .surreal_items
            .iter()
            .find(|item| item.summary == "test motivation")
            .expect("Motivation should exist")
            .id
            .as_ref()
            .expect("Motivation must have id")
            .clone();
        let step_id = surreal_tables
            .surreal_items
            .iter()
            .find(|item| item.summary == "test step")
            .expect("Step should exist")
            .id
            .as_ref()
            .expect("Step must have id")
            .clone();

        // Use the same data-layer command interface to declare that
        // "test step" is a smaller item of "test motivation" with
        // importance scope AllModes.
        sender
            .send(DataLayerCommands::ParentItemWithExistingItem {
                child: step_id,
                parent: motivation_id,
                // None means "at the end of the list"; the importance scope
                // for the smaller item itself is encoded in the SurrealImportance
                // that the data layer will create.
                higher_importance_than_this: None,
            })
            .await
            .expect("Should send parent-child relationship command");

        // Reload tables so the parent-child relationship is reflected.
        let surreal_tables = SurrealTables::new(&sender)
            .await
            .expect("Should load updated tables");

        // Look up the created items to get their record IDs.
        let motivation_id = surreal_tables
            .surreal_items
            .iter()
            .find(|item| item.summary == "test motivation")
            .expect("Motivation should exist")
            .id
            .as_ref()
            .expect("Motivation must have id")
            .clone();

        // Create a mode "Test Mode" where the motivation is explicitly out of scope.
        let (mode_sender, mode_receiver) = tokio::sync::oneshot::channel::<SurrealMode>();
        let new_mode = NewModeBuilder::default()
            .summary("Test Mode")
            .explicitly_out_of_scope_items(vec![motivation_id.clone()])
            .build()
            .expect("Valid mode");
        sender
            .send(DataLayerCommands::NewMode(new_mode, mode_sender))
            .await
            .expect("Should send new mode");
        let surreal_mode = mode_receiver.await.expect("Mode should be created");
        let mode_id = surreal_mode
            .id
            .as_ref()
            .expect("Newly created mode should have id")
            .clone();

        // Set current mode to "Test Mode".
        let new_current_mode = NewCurrentMode::new(Some(mode_id));
        sender
            .send(DataLayerCommands::SetCurrentMode(new_current_mode))
            .await
            .expect("Should set current mode");

        // Rebuild BaseData and CalculatedData so they include the new mode and current mode.
        let surreal_tables = SurrealTables::new(&sender)
            .await
            .expect("Should load updated tables");
        let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
        let calculated_data = CalculatedData::new_from_base_data(base_data);

        // Act: build the Do Now list; with the current logic this is expected
        // to trigger the assertion that all choices were removed.
        let do_now_list = DoNowList::new_do_now_list(calculated_data, &now);

        assert!(do_now_list.get_ordered_do_now_list().is_empty());

        drop(sender);
        data_storage_join_handle
            .await
            .expect("Data storage loop should exit");
    }

    /// Scenario:
    /// - Motivation "test motivation" (top-level)
    /// - Child "test step" is a smaller item of the motivation, with importance scope
    ///   `DefaultModesWithChanges` and `extra_modes_included` containing an extra mode.
    /// - Current mode "Test Mode" explicitly marks the motivation out of scope.
    ///
    /// Expectation (to trigger a known TODO):
    /// - Calling `get_category_by_importance` on the child while a current mode exists
    ///   should panic at the `todo!` in `Mode::get_category_by_importance` for
    ///   `DefaultModesWithChanges`.
    ///
    /// This test is intentionally *not* marked `should_panic` so it fails until the
    /// TODO is implemented.
    #[tokio::test]
    async fn test_default_modes_with_changes_importance_scope__default_modes_with_changes_treats_a_child_as_non_core()
     {
        // Arrange DB and data storage layer.
        let (sender, receiver) = mpsc::channel(1);
        let data_storage_join_handle =
            tokio::spawn(async move { data_storage_start_and_run(receiver, "mem://").await });

        let now = Utc::now();

        // Create the parent motivation item.
        sender
            .send(DataLayerCommands::NewItem(
                NewItemBuilder::default()
                    .summary("test motivation")
                    .item_type(SurrealItemType::Motivation)
                    .build()
                    .expect("Valid motivation"),
            ))
            .await
            .expect("Should send motivation");

        // Create the child "test step" item.
        sender
            .send(DataLayerCommands::NewItem(
                NewItemBuilder::default()
                    .summary("test step")
                    .item_type(SurrealItemType::Action)
                    .urgency_plan(Some(SurrealUrgencyPlan::StaysTheSame(None)))
                    .build()
                    .expect("Valid step"),
            ))
            .await
            .expect("Should send step");

        // Load tables to look up the created items.
        let surreal_tables = SurrealTables::new(&sender)
            .await
            .expect("Should load initial tables");

        let motivation_id = surreal_tables
            .surreal_items
            .iter()
            .find(|item| item.summary == "test motivation")
            .expect("Motivation should exist")
            .id
            .as_ref()
            .expect("Motivation must have id")
            .clone();
        let step_id = surreal_tables
            .surreal_items
            .iter()
            .find(|item| item.summary == "test step")
            .expect("Step should exist")
            .id
            .as_ref()
            .expect("Step must have id")
            .clone();
        // Create a current mode where the parent motivation is explicitly out of scope.
        let (mode_sender, mode_receiver) = tokio::sync::oneshot::channel::<SurrealMode>();
        let new_mode = NewModeBuilder::default()
            .summary("Test Mode")
            .explicitly_out_of_scope_items(vec![motivation_id.clone()])
            .build()
            .expect("Valid mode");
        sender
            .send(DataLayerCommands::NewMode(new_mode, mode_sender))
            .await
            .expect("Should send new mode");
        let surreal_mode = mode_receiver.await.expect("Mode should be created");
        let mode_id = surreal_mode
            .id
            .as_ref()
            .expect("Newly created mode should have id")
            .clone();

        // Parent "test step" under "test motivation" with DefaultModesWithChanges scope.
        sender
            .send(DataLayerCommands::ParentItemWithExistingItem {
                child: step_id.clone(),
                parent: motivation_id.clone(),
                higher_importance_than_this: Some((
                    crate::data_storage::surrealdb_layer::surreal_item::SurrealModeScope::DefaultModesWithChanges {
                        extra_modes_included: vec![mode_id.clone()],
                    },
                    None,
                )),
            })
            .await
            .expect("Should send parent-child relationship command");

        // Set current mode to "Test Mode".
        let new_current_mode = NewCurrentMode::new(Some(mode_id));
        sender
            .send(DataLayerCommands::SetCurrentMode(new_current_mode))
            .await
            .expect("Should set current mode");

        // Rebuild BaseData and CalculatedData so they include the new mode and current mode.
        let surreal_tables = SurrealTables::new(&sender)
            .await
            .expect("Should load updated tables");
        let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);
        let calculated_data = CalculatedData::new_from_base_data(base_data);

        // Build a Do Now list to mirror normal flow (this itself should not hit the TODO).
        let do_now_list = DoNowList::new_do_now_list(calculated_data, &now);

        // Now explicitly categorize the child by importance under the current mode.
        let child_status = do_now_list
            .get_all_items_status()
            .get(&step_id)
            .expect("Child status should exist");
        let current_mode = do_now_list
            .get_current_mode()
            .as_ref()
            .expect("Current mode should exist");

        // Validate test preconditions: the child is active and has the motivation as a parent.
        assert!(
            child_status.get_item_node().has_parents(Filter::Active),
            "Child should have an active parent"
        );
        let immediate_parents = child_status
            .get_item_node()
            .get_immediate_parents(Filter::Active)
            .collect::<Vec<_>>();
        assert_eq!(
            immediate_parents.len(),
            1,
            "Expected exactly one immediate parent"
        );
        assert_eq!(
            immediate_parents[0].get_surreal_record_id(),
            &motivation_id,
            "Parent should be the motivation item"
        );

        let category = current_mode.get_category_by_importance(child_status.get_item_node());
        match category {
            ModeCategory::NonCore => {}
            _ => {
                let child_summary = child_status.get_item_node().get_item().get_summary();
                let parent_summary = immediate_parents[0].get_item().get_summary();
                let parent_importance_scope_debug = immediate_parents[0]
                    .get_importance_scope()
                    .map(|scope| format!("{scope:?}"))
                    .unwrap_or_else(|| "None".to_string());
                let mode_node = current_mode.get_mode();
                let mode_name = mode_node.get_name();
                let mode_id = mode_node.get_surreal_id();

                panic!(
                    "Expected DefaultModesWithChanges scope to treat child as NonCore.\n\
actual category: {category:?}\n\
child: summary='{child_summary}', id={step_id:?}\n\
parent: summary='{parent_summary}', id={motivation_id:?}, link_importance_scope={parent_importance_scope_debug}\n\
current_mode: name='{mode_name}', id={mode_id:?}"
                );
            }
        }

        drop(sender);
        data_storage_join_handle
            .await
            .expect("Data storage loop should exit");
    }
}
