use std::{iter, time::Duration};

use ahash::HashMap;
use chrono::{DateTime, Utc};
use surrealdb::RecordId;

use crate::{
    base_data::{event::Event, item::Item},
    data_storage::surrealdb_layer::surreal_item::{
        SurrealDependency, SurrealScheduled, SurrealUrgency,
    },
};

use super::{
    Filter, IsActive, IsTriggered,
    action_with_item_status::ActionWithItemStatus,
    item_node::{
        ActionWithItem, DependencyWithItem, ItemNode, ItemsInScopeWithItem, TriggerWithItem,
        UrgencyPlanWithItem,
    },
};

#[derive(Clone, Debug)]
pub(crate) struct ItemStatus<'s> {
    item_node: &'s ItemNode<'s>,
    dependencies: Vec<DependencyWithItemNode<'s>>,
    children: Vec<&'s ItemNode<'s>>,
    parents: Vec<&'s ItemNode<'s>>,
    urgency_plan: Option<UrgencyPlanWithItemNode<'s>>,
    urgent_action_items: Vec<ActionWithItemNode<'s>>,
}

#[derive(Clone, Debug)]
pub(crate) enum DependencyWithItemNode<'e> {
    AfterDateTime {
        after: DateTime<Utc>,
        is_active: bool,
    },
    UntilScheduled {
        after: DateTime<Utc>,
        is_active: bool,
    },
    AfterItem(&'e ItemNode<'e>),
    AfterEvent(&'e Event<'e>),
    AfterChildItem(&'e ItemNode<'e>),
    DuringItem(&'e ItemNode<'e>),
    WaitingToBeInterrupted,
}

impl IsActive for DependencyWithItemNode<'_> {
    fn is_active(&self) -> bool {
        match self {
            DependencyWithItemNode::AfterDateTime { is_active, .. } => *is_active,
            DependencyWithItemNode::UntilScheduled { is_active, .. } => *is_active,
            DependencyWithItemNode::AfterItem(item) => item.is_active(),
            DependencyWithItemNode::AfterChildItem(item) => item.is_active(),
            DependencyWithItemNode::DuringItem(item) => item.is_active(),
            DependencyWithItemNode::AfterEvent(event) => !event.is_triggered(),
            DependencyWithItemNode::WaitingToBeInterrupted => true,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum UrgencyPlanWithItemNode<'e> {
    WillEscalate {
        initial: SurrealUrgency,
        triggers: Vec<TriggerWithItemNode<'e>>,
        later: SurrealUrgency,
    },
    StaysTheSame(SurrealUrgency),
}

#[derive(Clone, Debug)]
pub(crate) enum TriggerWithItemNode<'e> {
    WallClockDateTime {
        after: DateTime<Utc>,
        is_triggered: bool,
    },
    LoggedInvocationCount {
        starting: DateTime<Utc>,
        count_needed: u32,
        current_count: u32,
        items_in_scope: ItemsInScopeWithItemNode<'e>,
    },
    LoggedAmountOfTime {
        starting: DateTime<Utc>,
        duration_needed: Duration,
        current_duration: Duration,
        items_in_scope: ItemsInScopeWithItemNode<'e>,
    },
}

impl IsTriggered for Vec<TriggerWithItemNode<'_>> {
    fn is_triggered(&self) -> bool {
        if self.is_empty() {
            true
        } else {
            self.iter().any(|x| x.is_triggered())
        }
    }
}

impl IsTriggered for TriggerWithItemNode<'_> {
    fn is_triggered(&self) -> bool {
        match self {
            TriggerWithItemNode::WallClockDateTime { is_triggered, .. } => *is_triggered,
            TriggerWithItemNode::LoggedInvocationCount {
                count_needed,
                current_count,
                ..
            } => current_count >= count_needed,
            TriggerWithItemNode::LoggedAmountOfTime {
                duration_needed,
                current_duration,
                ..
            } => current_duration >= duration_needed,
        }
    }
}

impl<'e> TriggerWithItemNode<'e> {
    pub(crate) fn new(
        trigger: &TriggerWithItem<'_>,
        all_nodes: &'e HashMap<&'e RecordId, ItemNode<'e>>,
    ) -> Self {
        match trigger {
            TriggerWithItem::WallClockDateTime {
                after,
                is_triggered,
            } => TriggerWithItemNode::WallClockDateTime {
                after: *after,
                is_triggered: *is_triggered,
            },
            TriggerWithItem::LoggedInvocationCount {
                starting,
                count_needed,
                current_count,
                items_in_scope,
            } => TriggerWithItemNode::LoggedInvocationCount {
                starting: *starting,
                count_needed: *count_needed,
                current_count: *current_count,
                items_in_scope: ItemsInScopeWithItemNode::new(items_in_scope, all_nodes),
            },
            TriggerWithItem::LoggedAmountOfTime {
                starting,
                duration_needed,
                current_duration,
                items_in_scope,
            } => TriggerWithItemNode::LoggedAmountOfTime {
                starting: *starting,
                duration_needed: *duration_needed,
                current_duration: *current_duration,
                items_in_scope: ItemsInScopeWithItemNode::new(items_in_scope, all_nodes),
            },
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum ItemsInScopeWithItemNode<'e> {
    All,
    Include(Vec<&'e ItemNode<'e>>),
    Exclude(Vec<&'e ItemNode<'e>>),
}

impl From<DependencyWithItemNode<'_>> for SurrealDependency {
    fn from(dependency_with_item_node: DependencyWithItemNode) -> Self {
        match dependency_with_item_node {
            DependencyWithItemNode::AfterDateTime {
                after,
                is_active: _is_active,
            } => SurrealDependency::AfterDateTime(after.into()),
            DependencyWithItemNode::UntilScheduled { .. } => panic!(
                "Programming error, UntilScheduled, is not represented in SurrealDependency, it is derived. Do not call into on this."
            ),
            DependencyWithItemNode::AfterItem(item_node) => {
                SurrealDependency::AfterItem(item_node.get_surreal_record_id().clone())
            }
            DependencyWithItemNode::AfterChildItem(..) => panic!(
                "Programming error, AfterSmallerItem, is not represented in SurrealDependency, it is derived. Do not call into on this."
            ),
            DependencyWithItemNode::DuringItem(item_node) => {
                SurrealDependency::DuringItem(item_node.get_surreal_record_id().clone())
            }
            DependencyWithItemNode::WaitingToBeInterrupted => panic!(
                "Programming error, WaitingToBeInterrupted, is not represented in SurrealDependency, it is derived. Do not call into on this."
            ),
            DependencyWithItemNode::AfterEvent(event) => {
                SurrealDependency::AfterEvent(event.get_surreal_record_id().clone())
            }
        }
    }
}

impl<'e> ItemsInScopeWithItemNode<'e> {
    pub(crate) fn new(
        items_in_scope_with_item: &ItemsInScopeWithItem<'_>,
        all_nodes: &'e HashMap<&'e RecordId, ItemNode<'e>>,
    ) -> Self {
        match items_in_scope_with_item {
            ItemsInScopeWithItem::All => ItemsInScopeWithItemNode::All,
            ItemsInScopeWithItem::Include(items) => ItemsInScopeWithItemNode::Include(
                items
                    .iter()
                    .map(|item| {
                        all_nodes
                            .get(item.get_surreal_record_id())
                            .expect("Item came from here and should be here")
                    })
                    .collect(),
            ),
            ItemsInScopeWithItem::Exclude(items) => ItemsInScopeWithItemNode::Exclude(
                items
                    .iter()
                    .map(|item| {
                        all_nodes
                            .get(item.get_surreal_record_id())
                            .expect("Item came from here and should be here")
                    })
                    .collect(),
            ),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum ActionWithItemNode<'e> {
    SetReadyAndUrgency(&'e ItemNode<'e>),
    ParentBackToAMotivation(&'e ItemNode<'e>),
    ReviewItem(&'e ItemNode<'e>),
    ItemNeedsAClassification(&'e ItemNode<'e>),
    PickItemReviewFrequency(&'e ItemNode<'e>),
    MakeProgress(&'e ItemNode<'e>),
}

impl<'e> ActionWithItemNode<'e> {
    pub(crate) fn new(
        action_with_item: &ActionWithItem<'_>,
        all_nodes: &'e HashMap<&'e RecordId, ItemNode<'e>>,
    ) -> Self {
        match action_with_item {
            ActionWithItem::SetReadyAndUrgency(item) => ActionWithItemNode::SetReadyAndUrgency(
                all_nodes
                    .get(item.get_surreal_record_id())
                    .expect("Item came from here and should be here"),
            ),
            ActionWithItem::ParentBackToAMotivation(item) => {
                ActionWithItemNode::ParentBackToAMotivation(
                    all_nodes
                        .get(item.get_surreal_record_id())
                        .expect("Item came from here and should be here"),
                )
            }
            ActionWithItem::ReviewItem(item) => ActionWithItemNode::ReviewItem(
                all_nodes
                    .get(item.get_surreal_record_id())
                    .expect("Item came from here and should be here"),
            ),
            ActionWithItem::PickItemReviewFrequency(item) => {
                ActionWithItemNode::PickItemReviewFrequency(
                    all_nodes
                        .get(item.get_surreal_record_id())
                        .expect("Item came from here and should be here"),
                )
            }
            ActionWithItem::ItemNeedsAClassification(item) => {
                ActionWithItemNode::ItemNeedsAClassification(
                    all_nodes
                        .get(item.get_surreal_record_id())
                        .expect("Item came from here and should be here"),
                )
            }
            ActionWithItem::MakeProgress(item) => ActionWithItemNode::MakeProgress(
                all_nodes
                    .get(item.get_surreal_record_id())
                    .expect("Item came from here and should be here"),
            ),
        }
    }
}

impl PartialEq for ItemStatus<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.item_node == other.item_node
    }
}

pub(crate) struct MostImportantReadyAndBlocked<'s> {
    pub(crate) ready: Option<&'s ItemStatus<'s>>,
    pub(crate) blocked: Vec<&'s ItemStatus<'s>>,
}

impl<'s> ItemStatus<'s> {
    pub(crate) fn new(
        item_node: &'s ItemNode<'s>,
        all_nodes: &'s HashMap<&'s RecordId, ItemNode<'s>>,
    ) -> Self {
        let dependencies = calculate_dependencies(item_node, all_nodes);
        let children = calculate_children(item_node, all_nodes);
        let parents = calculate_parents(item_node, all_nodes);
        let urgency_plan = item_node.get_urgency_plan().as_ref().map(|x| match x {
            UrgencyPlanWithItem::WillEscalate {
                initial,
                triggers,
                later,
            } => UrgencyPlanWithItemNode::WillEscalate {
                initial: initial.clone(),
                triggers: triggers
                    .iter()
                    .map(|trigger| TriggerWithItemNode::new(trigger, all_nodes))
                    .collect::<Vec<_>>(),
                later: later.clone(),
            },
            UrgencyPlanWithItem::StaysTheSame(urgency) => {
                UrgencyPlanWithItemNode::StaysTheSame(urgency.clone())
            }
        });
        let urgent_action_items =
            calculate_urgent_action_items(item_node.get_urgent_action_items(), all_nodes);
        Self {
            item_node,
            dependencies,
            children,
            parents,
            urgency_plan,
            urgent_action_items,
        }
    }

    pub(crate) fn is_finished(&self) -> bool {
        self.item_node.is_finished()
    }

    pub(crate) fn get_item_node(&'s self) -> &'s ItemNode<'s> {
        self.item_node
    }

    pub(crate) fn is_type_motivation(&self) -> bool {
        self.item_node.is_type_motivation()
    }

    pub(crate) fn is_type_motivation_kind_core(&self) -> bool {
        self.item_node.is_type_motivation_kind_core()
    }

    pub(crate) fn is_type_motivation_kind_non_core(&self) -> bool {
        self.item_node.is_type_motivation_kind_non_core()
    }

    pub(crate) fn is_type_motivation_kind_neither(&self) -> bool {
        self.item_node.is_type_motivation_kind_neither()
    }

    pub(crate) fn is_person_or_group(&self) -> bool {
        self.item_node.is_person_or_group()
    }

    pub(crate) fn get_item(&self) -> &Item<'s> {
        self.item_node.get_item()
    }

    pub(crate) fn get_summary(&self) -> &str {
        self.item_node.get_summary()
    }

    pub(crate) fn has_children(&self, filter: Filter) -> bool {
        self.item_node.has_children(filter)
    }

    pub(crate) fn get_finished_at(&self) -> &Option<DateTime<Utc>> {
        self.item_node.get_finished_at()
    }

    pub(crate) fn get_children(
        &'s self,
        filter: Filter,
    ) -> Box<dyn Iterator<Item = &'s ItemNode<'s>> + 's> {
        match filter {
            Filter::All => Box::new(self.children.iter().copied()),
            Filter::Active => Box::new(self.children.iter().copied().filter(|x| x.is_active())),
            Filter::Finished => Box::new(self.children.iter().copied().filter(|x| x.is_finished())),
        }
    }

    pub(crate) fn get_surreal_record_id(&self) -> &RecordId {
        self.item_node.get_surreal_record_id()
    }

    pub(crate) fn has_parents(&self, filter: Filter) -> bool {
        self.item_node.has_parents(filter)
    }

    pub(crate) fn get_parents(
        &'s self,
        filter: Filter,
    ) -> Box<dyn Iterator<Item = &'s ItemNode<'s>> + 's> {
        match filter {
            Filter::All => Box::new(self.parents.iter().copied()),
            Filter::Active => Box::new(self.parents.iter().copied().filter(|x| x.is_active())),
            Filter::Finished => Box::new(self.parents.iter().copied().filter(|x| x.is_finished())),
        }
    }

    pub(crate) fn get_self_and_parents_flattened(&'s self, filter: Filter) -> Vec<&'s Item<'s>> {
        //TODO This should be updated to return ItemNode from itself rather than calling into the next layer down
        self.item_node.get_self_and_parents(filter)
    }

    pub(crate) fn is_active(&self) -> bool {
        self.item_node.is_active()
    }

    pub(crate) fn is_scheduled_now(&self) -> bool {
        self.item_node.is_scheduled_now()
    }

    pub(crate) fn get_scheduled_now(&self) -> Option<&SurrealScheduled> {
        self.item_node.get_scheduled_now()
    }

    pub(crate) fn get_dependencies(
        &'s self,
        filter: Filter,
    ) -> Box<dyn Iterator<Item = &'s DependencyWithItemNode<'s>> + 's> {
        match filter {
            Filter::All => Box::new(self.dependencies.iter()),
            Filter::Active => Box::new(self.dependencies.iter().filter(|x| x.is_active())),
            Filter::Finished => Box::new(self.dependencies.iter().filter(|x| !x.is_active())),
        }
    }

    pub(crate) fn has_dependencies(&self, filter: Filter) -> bool {
        self.item_node.has_dependencies(filter)
    }

    pub(crate) fn get_urgency_plan(&self) -> &Option<UrgencyPlanWithItemNode<'s>> {
        &self.urgency_plan
    }

    pub(crate) fn get_urgency_now(&self) -> Option<&SurrealUrgency> {
        self.item_node.get_urgency_now()
    }

    pub(crate) fn get_urgent_item_actions(&'s self) -> &'s [ActionWithItemNode<'s>] {
        &self.urgent_action_items
    }

    pub(crate) fn get_now(&self) -> &DateTime<Utc> {
        self.item_node.get_now()
    }

    pub(crate) fn is_ready_to_be_worked_on(&self) -> bool {
        self.item_node.is_ready_to_be_worked_on()
    }

    pub(crate) fn recursive_get_most_important_and_ready(
        &'s self,
        all_item_status: &'s HashMap<&'s RecordId, ItemStatus<'s>>,
    ) -> Option<&'s ItemStatus<'s>> {
        let a = self
            .recursive_get_most_important_both_ready_and_blocked(all_item_status, Vec::default());
        a.ready
    }

    pub(crate) fn recursive_get_most_important_both_ready_and_blocked(
        &'s self,
        all_item_status: &'s HashMap<&'s RecordId, ItemStatus<'s>>,
        mut visited: Vec<&'s ItemStatus<'s>>,
    ) -> MostImportantReadyAndBlocked<'s> {
        let mut would_be_most_important_but_not_ready = Vec::default();
        if self.has_children(Filter::Active) {
            visited.push(self);
            for child in self.get_children(Filter::Active) {
                let child = all_item_status
                    .get(child.get_surreal_record_id())
                    .expect("All items should be in the list");
                if visited.contains(&child) {
                    if self.is_ready_to_be_worked_on() {
                        return MostImportantReadyAndBlocked {
                            ready: Some(self),
                            blocked: would_be_most_important_but_not_ready,
                        };
                    } else {
                        would_be_most_important_but_not_ready.push(self);
                    }
                } else {
                    let r = child.recursive_get_most_important_both_ready_and_blocked(
                        all_item_status,
                        visited.clone(),
                    );
                    would_be_most_important_but_not_ready.extend(r.blocked);
                    if r.ready.is_some() {
                        return MostImportantReadyAndBlocked {
                            ready: r.ready,
                            blocked: would_be_most_important_but_not_ready,
                        };
                    }
                }
            }

            MostImportantReadyAndBlocked {
                ready: None,
                blocked: would_be_most_important_but_not_ready,
            }
        } else if self.is_ready_to_be_worked_on() {
            MostImportantReadyAndBlocked {
                ready: Some(self),
                blocked: would_be_most_important_but_not_ready,
            }
        } else {
            would_be_most_important_but_not_ready.push(self);
            MostImportantReadyAndBlocked {
                ready: None,
                blocked: would_be_most_important_but_not_ready,
            }
        }
    }

    pub(crate) fn recursive_get_urgent_bullet_list(
        &'s self,
        all_item_status: &'s HashMap<&'s RecordId, ItemStatus<'s>>,
        mut visited: Vec<&'s ItemStatus<'s>>,
    ) -> Box<dyn Iterator<Item = ActionWithItemStatus<'s>> + 's> {
        visited.push(self);
        Box::new(
            self.get_urgent_item_actions()
                .iter()
                .map(|x| ActionWithItemStatus::new(x, all_item_status))
                .chain(self.get_children(Filter::Active).flat_map(move |child| {
                    let child = all_item_status
                        .get(child.get_surreal_record_id())
                        .expect("All items should be in the list");
                    if !visited.contains(&child) {
                        child.recursive_get_urgent_bullet_list(all_item_status, visited.clone())
                    } else {
                        Box::new(iter::empty::<ActionWithItemStatus<'s>>())
                    }
                })),
        )
    }

    pub(crate) fn get_created(&self) -> &DateTime<Utc> {
        self.get_item_node().get_created()
    }
}

fn calculate_dependencies<'s>(
    item_node: &'s ItemNode<'s>,
    all_nodes: &'s HashMap<&'s RecordId, ItemNode<'s>>,
) -> Vec<DependencyWithItemNode<'s>> {
    item_node
        .get_dependencies(Filter::All)
        .map(|x| match x {
            DependencyWithItem::AfterDateTime { after, is_active } => {
                DependencyWithItemNode::AfterDateTime {
                    after: *after,
                    is_active: *is_active,
                }
            }
            DependencyWithItem::AfterItem(item) => DependencyWithItemNode::AfterItem(
                all_nodes
                    .get(item.get_surreal_record_id())
                    .expect("Item came from here and should be here"),
            ),
            DependencyWithItem::AfterChildItem(item) => DependencyWithItemNode::AfterChildItem(
                all_nodes
                    .get(item.get_surreal_record_id())
                    .expect("Item came from here and should be here"),
            ),
            DependencyWithItem::DuringItem(item) => DependencyWithItemNode::DuringItem(
                all_nodes
                    .get(item.get_surreal_record_id())
                    .expect("Item came from here and should be here"),
            ),
            DependencyWithItem::UntilScheduled { after, is_active } => {
                DependencyWithItemNode::UntilScheduled {
                    after: *after,
                    is_active: *is_active,
                }
            }
            DependencyWithItem::WaitingToBeInterrupted => {
                DependencyWithItemNode::WaitingToBeInterrupted
            }
            DependencyWithItem::AfterEvent(event) => DependencyWithItemNode::AfterEvent(event),
        })
        .collect()
}

fn calculate_parents<'s>(
    item_node: &'s ItemNode<'s>,
    all_nodes: &'s HashMap<&'s RecordId, ItemNode<'s>>,
) -> Vec<&'s ItemNode<'s>> {
    item_node
        .get_parents(Filter::All)
        .map(|x| {
            all_nodes
                .get(x.get_surreal_record_id())
                .expect("Item came from here and should be here")
        })
        .collect()
}

fn calculate_children<'s>(
    item_node: &'s ItemNode<'s>,
    all_nodes: &'s HashMap<&'s RecordId, ItemNode<'s>>,
) -> Vec<&'s ItemNode<'s>> {
    item_node
        .get_children(Filter::All)
        .map(|x| {
            all_nodes
                .get(x.get_surreal_record_id())
                .expect("Item came from here and should be here")
        })
        .collect()
}

fn calculate_urgent_action_items<'a>(
    actions: &[ActionWithItem<'_>],
    all_nodes: &'a HashMap<&'a RecordId, ItemNode<'a>>,
) -> Vec<ActionWithItemNode<'a>> {
    actions
        .iter()
        .map(|action| ActionWithItemNode::new(action, all_nodes))
        .collect()
}

#[cfg(test)]
mod tests {
    use chrono::{Days, Utc};
    use tokio::sync::mpsc;

    use crate::{
        base_data::BaseData,
        calculated_data::CalculatedData,
        data_storage::surrealdb_layer::{
            data_layer_commands::{
                DataLayerCommands, SurrealDbConnectionConfig, data_storage_start_and_run,
            },
            surreal_item::SurrealDependency,
            surreal_tables::SurrealTables,
        },
        new_item::{NewDependency, NewItemBuilder},
        node::Filter,
    };

    fn mem_config() -> SurrealDbConnectionConfig {
        SurrealDbConnectionConfig {
            endpoint: "mem://".to_string(),
            namespace: "TaskOnPurpose".to_string(),
            database: "test".to_string(),
            auth: None,
        }
    }

    #[tokio::test]
    async fn item_with_a_child_has_that_child_as_a_dependency() {
        // Arrange
        let (sender, receiver) = mpsc::channel(1);
        let data_storage_join_handle =
            tokio::spawn(async move { data_storage_start_and_run(receiver, mem_config()).await });

        sender
            .send(DataLayerCommands::NewItem(
                NewItemBuilder::default()
                    .summary("Parent Item with a child")
                    .build()
                    .expect("valid new item"),
            ))
            .await
            .unwrap();

        let surreal_tables = SurrealTables::new(&sender).await.unwrap();
        let parent_item = surreal_tables
            .surreal_items
            .iter()
            .find(|x| x.summary == "Parent Item with a child")
            .expect("Just added this very item");

        sender
            .send(DataLayerCommands::ParentItemWithANewChildItem {
                child: NewItemBuilder::default()
                    .summary("Child Item")
                    .build()
                    .expect("Valid item"),
                parent: parent_item.id.as_ref().expect("Is in DB").clone(),
                higher_importance_than_this: None,
            })
            .await
            .expect("Unit test setup/arrange");

        let now = Utc::now();
        let surreal_tables = SurrealTables::new(&sender).await.unwrap();
        let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);

        //Act
        let calculated_data = CalculatedData::new_from_base_data(base_data);
        let items_highest_lap_count = calculated_data.get_items_status();
        let (_, item) = items_highest_lap_count
            .iter()
            .find(|(_, x)| x.get_summary() == "Parent Item with a child")
            .expect("Just added this very item");

        //Assert
        assert!(item.has_dependencies(Filter::Active));

        drop(sender);
        data_storage_join_handle.await.unwrap();
    }

    #[tokio::test]
    async fn item_with_another_item_as_a_dependency_has_a_dependency() {
        // Arrange
        let (sender, receiver) = mpsc::channel(1);
        let data_storage_join_handle =
            tokio::spawn(async move { data_storage_start_and_run(receiver, mem_config()).await });

        sender
            .send(DataLayerCommands::NewItem(
                NewItemBuilder::default()
                    .summary("Item to be covered")
                    .build()
                    .expect("valid new item"),
            ))
            .await
            .unwrap();

        let surreal_tables = SurrealTables::new(&sender).await.unwrap();
        let parent_item = surreal_tables
            .surreal_items
            .iter()
            .find(|x| x.summary == "Item to be covered")
            .expect("Just added this very item");

        sender
            .send(DataLayerCommands::CoverItemWithANewItem {
                cover_this: parent_item.id.as_ref().expect("Is in DB").clone(),
                cover_with: NewItemBuilder::default()
                    .summary("Covering Item")
                    .build()
                    .expect("Valid item"),
            })
            .await
            .expect("Unit test setup/arrange");

        let now = Utc::now();
        let surreal_tables = SurrealTables::new(&sender).await.unwrap();
        let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);

        //Act
        let calculated_data = CalculatedData::new_from_base_data(base_data);
        let items_highest_lap_count = calculated_data.get_items_status();
        let (_, item) = items_highest_lap_count
            .iter()
            .find(|(_, x)| x.get_summary() == "Item to be covered")
            .expect("Just added this very item");

        //Assert
        assert!(item.has_dependencies(Filter::Active));

        drop(sender);
        data_storage_join_handle.await.unwrap();
    }

    #[tokio::test]
    async fn item_that_needs_to_wait_until_tomorrow_has_a_dependency() {
        // Arrange
        let (sender, receiver) = mpsc::channel(1);
        let data_storage_join_handle =
            tokio::spawn(async move { data_storage_start_and_run(receiver, mem_config()).await });

        sender
            .send(DataLayerCommands::NewItem(
                NewItemBuilder::default()
                    .summary("Item that needs to wait until tomorrow")
                    .dependencies(vec![NewDependency::Existing(
                        SurrealDependency::AfterDateTime(
                            Utc::now()
                                .checked_add_days(Days::new(1))
                                .expect("Far from overflowing")
                                .into(),
                        ),
                    )])
                    .build()
                    .expect("valid new item"),
            ))
            .await
            .unwrap();

        let now = Utc::now();

        let surreal_tables = SurrealTables::new(&sender).await.unwrap();
        let base_data = BaseData::new_from_surreal_tables(surreal_tables, now);

        //Act
        let calculated_data = CalculatedData::new_from_base_data(base_data);
        let items_highest_lap_count = calculated_data.get_items_status();
        let (_, item) = items_highest_lap_count.iter().next().unwrap();

        //Assert
        assert!(item.has_dependencies(Filter::Active));

        drop(sender);
        data_storage_join_handle.await.unwrap();
    }
}
