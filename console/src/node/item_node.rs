use std::{iter, time::Duration};

use ahash::HashMap;
use chrono::{DateTime, Utc};
use surrealdb::{RecordId, sql::Datetime};

use crate::{
    base_data::{Visited, event::Event, item::Item, time_spent::TimeSpent},
    calculated_data::parent_lookup::ParentLookup,
    data_storage::surrealdb_layer::{
        SurrealItemsInScope, SurrealTrigger,
        surreal_item::{
            SurrealDependency, SurrealItem, SurrealItemType, SurrealReviewGuidance,
            SurrealScheduled, SurrealUrgency, SurrealUrgencyPlan,
        },
    },
};

use super::{Filter, GetUrgencyNow, IsActive, IsTriggered};

#[derive(Clone, Debug, Eq)]
pub(crate) struct ItemNode<'s> {
    item: &'s Item<'s>,
    parents: Vec<GrowingItemNode<'s>>,
    children: Vec<ShrinkingItemNode<'s>>,
    dependencies: Vec<DependencyWithItem<'s>>,
    urgency_plan: Option<UrgencyPlanWithItem<'s>>,
    urgent_action_items: Vec<ActionWithItem<'s>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DependencyWithItem<'e> {
    AfterDateTime {
        after: DateTime<Utc>,
        is_active: bool,
    },
    UntilScheduled {
        after: DateTime<Utc>,
        is_active: bool,
    }, //Scheduled items should use this to state that they are not ready until the scheduled time
    AfterItem(&'e Item<'e>),
    AfterChildItem(&'e Item<'e>),
    DuringItem(&'e Item<'e>),
    AfterEvent(&'e Event<'e>),
    WaitingToBeInterrupted,
}

impl IsActive for DependencyWithItem<'_> {
    fn is_active(&self) -> bool {
        match self {
            DependencyWithItem::AfterDateTime { is_active, .. } => *is_active,
            DependencyWithItem::UntilScheduled { is_active, .. } => *is_active,
            DependencyWithItem::AfterItem(item) => item.is_active(),
            DependencyWithItem::AfterChildItem(item) => item.is_active(),
            DependencyWithItem::DuringItem(item) => item.is_active(),
            DependencyWithItem::AfterEvent(event) => event.is_active(),
            DependencyWithItem::WaitingToBeInterrupted => true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum UrgencyPlanWithItem<'e> {
    WillEscalate {
        initial: SurrealUrgency,
        triggers: Vec<TriggerWithItem<'e>>,
        later: SurrealUrgency,
    },
    StaysTheSame(SurrealUrgency),
}

impl GetUrgencyNow for UrgencyPlanWithItem<'_> {
    fn get_urgency_now(&self) -> Option<&SurrealUrgency> {
        match self {
            UrgencyPlanWithItem::WillEscalate {
                initial,
                triggers,
                later,
            } => {
                if triggers.is_triggered() {
                    Some(later)
                } else {
                    Some(initial)
                }
            }
            UrgencyPlanWithItem::StaysTheSame(urgency) => Some(urgency),
        }
    }
}

impl GetUrgencyNow for Option<UrgencyPlanWithItem<'_>> {
    fn get_urgency_now(&self) -> Option<&SurrealUrgency> {
        self.as_ref().and_then(|x| x.get_urgency_now())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TriggerWithItem<'e> {
    WallClockDateTime {
        after: DateTime<Utc>,
        is_triggered: bool,
    },
    LoggedInvocationCount {
        starting: DateTime<Utc>,
        count_needed: u32,
        current_count: u32,
        items_in_scope: ItemsInScopeWithItem<'e>,
    },
    LoggedAmountOfTime {
        starting: DateTime<Utc>,
        duration_needed: Duration,
        current_duration: Duration,
        items_in_scope: ItemsInScopeWithItem<'e>,
    },
}

impl IsTriggered for Vec<TriggerWithItem<'_>> {
    fn is_triggered(&self) -> bool {
        if self.is_empty() {
            true
        } else {
            self.iter().any(|x| x.is_triggered())
        }
    }
}

impl IsTriggered for TriggerWithItem<'_> {
    fn is_triggered(&self) -> bool {
        match self {
            TriggerWithItem::WallClockDateTime { is_triggered, .. } => *is_triggered,
            TriggerWithItem::LoggedInvocationCount {
                count_needed,
                current_count,
                ..
            } => count_needed <= current_count,
            TriggerWithItem::LoggedAmountOfTime {
                duration_needed,
                current_duration,
                ..
            } => duration_needed <= current_duration,
        }
    }
}

impl<'e> TriggerWithItem<'e> {
    pub(crate) fn new(
        surreal_trigger: &'e SurrealTrigger,
        now_sql: &Datetime,
        all_items: &'e HashMap<&'e RecordId, Item<'e>>,
        time_spent_log: &[TimeSpent<'_>],
    ) -> Self {
        match surreal_trigger {
            SurrealTrigger::WallClockDateTime(after) => TriggerWithItem::WallClockDateTime {
                after: after.clone().into(),
                is_triggered: now_sql >= after,
            },
            SurrealTrigger::LoggedInvocationCount {
                starting,
                count,
                items_in_scope,
            } => {
                let starting = starting.clone().into();
                let items_in_scope = ItemsInScopeWithItem::new(items_in_scope, all_items);
                let time_spent_on_this =
                    get_time_spent_on_this(&starting, &items_in_scope, time_spent_log);
                TriggerWithItem::LoggedInvocationCount {
                    starting,
                    count_needed: *count,
                    current_count: time_spent_on_this.count() as u32,
                    items_in_scope,
                }
            }
            SurrealTrigger::LoggedAmountOfTime {
                starting,
                duration,
                items_in_scope,
            } => {
                let starting = starting.clone().into();
                let items_in_scope = ItemsInScopeWithItem::new(items_in_scope, all_items);
                let time_spent_on_this =
                    get_time_spent_on_this(&starting, &items_in_scope, time_spent_log);
                TriggerWithItem::LoggedAmountOfTime {
                    starting,
                    duration_needed: (*duration).into(),
                    current_duration: time_spent_on_this.map(|x| x.get_duration()).sum(),
                    items_in_scope,
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ItemsInScopeWithItem<'e> {
    All,
    Include(Vec<&'e Item<'e>>),
    Exclude(Vec<&'e Item<'e>>),
}

impl<'e> ItemsInScopeWithItem<'e> {
    pub(crate) fn new(
        surreal_items_in_scope: &'e SurrealItemsInScope,
        all_items: &'e HashMap<&'e RecordId, Item<'e>>,
    ) -> Self {
        match surreal_items_in_scope {
            SurrealItemsInScope::All => ItemsInScopeWithItem::All,
            SurrealItemsInScope::Include(include) => {
                let include = include
                    .iter()
                    .map(|x| all_items.get(x).expect("All items should contain this"))
                    .collect::<Vec<_>>();
                ItemsInScopeWithItem::Include(include)
            }
            SurrealItemsInScope::Exclude(exclude) => {
                let exclude = exclude
                    .iter()
                    .map(|x| all_items.get(x).expect("All items should contain this"))
                    .collect();
                ItemsInScopeWithItem::Exclude(exclude)
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ActionWithItem<'e> {
    SetReadyAndUrgency(&'e Item<'e>),
    ParentBackToAMotivation(&'e Item<'e>),
    ItemNeedsAClassification(&'e Item<'e>),
    ReviewItem(&'e Item<'e>),
    PickItemReviewFrequency(&'e Item<'e>),
    MakeProgress(&'e Item<'e>),
}

impl ActionWithItem<'_> {}

impl<'a> From<&'a ItemNode<'a>> for &'a Item<'a> {
    fn from(value: &ItemNode<'a>) -> Self {
        value.item
    }
}

impl<'a> From<&'a ItemNode<'a>> for &'a SurrealItem {
    fn from(value: &'a ItemNode<'a>) -> Self {
        value.item.into()
    }
}

impl PartialEq for ItemNode<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.item == other.item
    }
}

impl<'s> ItemNode<'s> {
    pub(crate) fn new(
        item: &'s Item<'s>,
        all_items: &'s HashMap<&'s RecordId, Item<'s>>,
        parent_lookup: &'s ParentLookup<'s>,
        all_events: &'s HashMap<&'s RecordId, Event<'s>>,
        time_spent_log: &[TimeSpent],
    ) -> Self {
        let visited = Visited::new(item.get_surreal_record_id(), None);
        let parents = item.find_parents(parent_lookup, &visited);
        let parents = create_growing_nodes(parents, parent_lookup, &visited);
        let visited: Vec<&RecordId> = iter::once(item.get_surreal_record_id())
            .chain(parents.iter().flat_map(|x| {
                x.get_self_and_parents(Vec::default())
                    .into_iter()
                    .map(|x| x.get_surreal_record_id())
            }))
            .collect();
        let children = item.find_children(all_items, &visited);
        let children = create_shrinking_nodes(&children, all_items, visited);
        let urgency_plan = calculate_urgency_plan(item, all_items, time_spent_log);
        let dependencies =
            calculate_dependencies(item, &urgency_plan, all_items, all_events, &children);
        let urgent_action_items = if item.is_active() {
            calculate_urgent_action_items(item, &parents, &children, &urgency_plan, &dependencies)
        } else {
            //Perf Improvement: Finished items should not have any urgent action items
            Vec::default()
        };
        ItemNode {
            item,
            parents,
            children,
            dependencies,
            urgency_plan,
            urgent_action_items,
        }
    }

    pub(crate) fn is_finished(&self) -> bool {
        self.item.is_finished()
    }

    pub(crate) fn create_parent_chain(&'s self, filter: Filter) -> Vec<(u32, &'s Item<'s>)> {
        let mut result = Vec::default();
        for i in self.get_parents(filter) {
            result.push((1, i.item));
            let parents = i.create_growing_parents(filter, 2);
            result.extend(parents.iter());
        }
        result
    }

    pub(crate) fn get_children(
        &'s self,
        filter: Filter,
    ) -> Box<dyn Iterator<Item = &'s ShrinkingItemNode<'s>> + 's + Send> {
        Box::new(self.children.iter().filter(move |x| match filter {
            Filter::All => true,
            Filter::Active => !x.item.is_finished(),
            Filter::Finished => x.item.is_finished(),
        }))
    }

    pub(crate) fn get_item(&self) -> &'s Item<'s> {
        self.item
    }

    pub(crate) fn get_surreal_record_id(&self) -> &RecordId {
        self.item.get_surreal_record_id()
    }

    pub(crate) fn is_person_or_group(&self) -> bool {
        self.item.is_person_or_group()
    }

    pub(crate) fn has_parents(&self, filter: Filter) -> bool {
        has_parents(&self.parents, filter)
    }

    pub(crate) fn get_finished_at(&self) -> &Option<DateTime<Utc>> {
        self.item.get_finished_at()
    }

    pub(crate) fn get_parents(
        &'s self,
        filter: Filter,
    ) -> Box<dyn Iterator<Item = &'s GrowingItemNode<'s>> + 's + Send> {
        match filter {
            Filter::All => Box::new(self.parents.iter()),
            Filter::Active => Box::new(self.parents.iter().filter(|x| !x.item.is_finished())),
            Filter::Finished => Box::new(self.parents.iter().filter(|x| x.item.is_finished())),
        }
    }

    /// Get's larger items and all of their parents and the current item
    pub(crate) fn get_self_and_parents(&'s self, filter: Filter) -> Vec<&'s Item<'s>> {
        let mut items = Vec::default();
        for item in self.get_parents(filter) {
            items = item.get_self_and_parents(items);
        }
        items.push(self.item);
        items
    }

    pub(crate) fn is_core_work_or_neither(&self, filter: Filter) -> bool {
        self.get_self_and_parents(filter)
            .iter()
            .any(|x| x.is_type_motivation_kind_core_or_neither())
    }

    pub(crate) fn is_non_core_work_or_neither(&self, filter: Filter) -> bool {
        self.get_self_and_parents(filter)
            .iter()
            .any(|x| x.is_type_motivation_kind_non_core_or_neither())
    }

    pub(crate) fn get_type(&self) -> &SurrealItemType {
        self.item.get_type()
    }

    pub(crate) fn is_type_goal(&self) -> bool {
        self.item.is_type_goal()
    }

    pub(crate) fn is_type_motivation(&self) -> bool {
        self.item.is_type_motivation()
    }

    pub(crate) fn is_type_motivation_kind_core(&self) -> bool {
        self.item.is_type_motivation_kind_core()
    }

    pub(crate) fn is_type_motivation_kind_non_core(&self) -> bool {
        self.item.is_type_motivation_kind_non_core()
    }

    pub(crate) fn is_type_motivation_kind_neither(&self) -> bool {
        self.item.is_type_motivation_kind_neither()
    }

    pub(crate) fn has_children(&self, filter: Filter) -> bool {
        has_children(&self.children, filter)
    }

    pub(crate) fn get_summary(&self) -> &str {
        self.item.get_summary()
    }

    pub(crate) fn get_created(&self) -> &DateTime<Utc> {
        self.item.get_created()
    }

    pub(crate) fn is_active(&self) -> bool {
        !self.is_finished()
    }

    pub(crate) fn is_scheduled_now(&self) -> bool {
        self.urgency_plan.is_scheduled_now()
    }

    pub(crate) fn get_scheduled_now(&self) -> Option<&SurrealScheduled> {
        self.urgency_plan.get_scheduled_now()
    }

    pub(crate) fn get_urgency_now(&self) -> Option<&SurrealUrgency> {
        self.urgency_plan.get_urgency_now()
    }

    pub(crate) fn has_dependencies(&self, filter: Filter) -> bool {
        self.get_dependencies(filter).next().is_some()
    }

    pub(crate) fn get_dependencies(
        &'s self,
        filter: Filter,
    ) -> Box<dyn Iterator<Item = &'s DependencyWithItem<'s>> + 's> {
        get_dependencies(&self.dependencies, filter)
    }

    pub(crate) fn get_urgency_plan(&'s self) -> &'s Option<UrgencyPlanWithItem<'s>> {
        &self.urgency_plan
    }

    pub(crate) fn get_now(&self) -> &DateTime<Utc> {
        self.item.get_now()
    }

    pub(crate) fn is_ready_to_be_worked_on(&self) -> bool {
        self.get_dependencies(Filter::Active).next().is_none()
    }

    pub(crate) fn get_urgent_action_items(&'s self) -> &'s Vec<ActionWithItem<'s>> {
        &self.urgent_action_items
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GrowingItemNode<'s> {
    pub(crate) item: &'s Item<'s>,
    pub(crate) larger: Vec<GrowingItemNode<'s>>,
}

impl<'s> GrowingItemNode<'s> {
    pub(crate) fn create_growing_parents(
        &'s self,
        filter: Filter,
        levels_deep: u32,
    ) -> Vec<(u32, &'s Item<'s>)> {
        let mut result = Vec::default();
        for i in self.get_parents(filter) {
            result.push((levels_deep, i.item));
            let parents = i.create_growing_parents(filter, levels_deep + 1);
            result.extend(parents.iter());
        }
        result
    }

    pub(crate) fn get_parents(
        &'s self,
        filter: Filter,
    ) -> Box<dyn Iterator<Item = &'s GrowingItemNode<'s>> + 's + Send> {
        match filter {
            Filter::All => Box::new(self.larger.iter()),
            Filter::Active => Box::new(self.larger.iter().filter(|x| !x.item.is_finished())),
            Filter::Finished => Box::new(self.larger.iter().filter(|x| x.item.is_finished())),
        }
    }

    pub(crate) fn get_item(&self) -> &'s Item<'s> {
        self.item
    }

    pub(crate) fn get_surreal_record_id(&self) -> &RecordId {
        self.item.get_surreal_record_id()
    }

    pub(crate) fn get_self_and_parents(&self, mut items: Vec<&'s Item<'s>>) -> Vec<&'s Item<'s>> {
        for item in &self.larger {
            items = item.get_self_and_parents(items);
        }
        items.push(self.item);
        items
    }

    pub(crate) fn get_surreal_review_guidance(&self) -> &Option<SurrealReviewGuidance> {
        self.item.get_surreal_review_guidance()
    }

    pub(crate) fn is_finished(&self) -> bool {
        self.item.is_finished()
    }
}

pub(crate) trait ShouldChildrenHaveReviewFrequencySet {
    fn should_children_have_review_frequency_set<'a>(
        &'a self,
        visited: Vec<&'a GrowingItemNode<'a>>,
    ) -> bool;
}

impl ShouldChildrenHaveReviewFrequencySet for &[GrowingItemNode<'_>] {
    fn should_children_have_review_frequency_set(
        &self,
        visited: Vec<&GrowingItemNode<'_>>,
    ) -> bool {
        if self.is_empty() {
            true
        } else {
            self.iter().any(|x| match x.get_surreal_review_guidance() {
                Some(_) => x.should_children_have_review_frequency_set(visited.clone()),
                None => false,
            })
        }
    }
}

impl ShouldChildrenHaveReviewFrequencySet for Vec<GrowingItemNode<'_>> {
    fn should_children_have_review_frequency_set(
        &self,
        visited: Vec<&GrowingItemNode<'_>>,
    ) -> bool {
        self.as_slice()
            .should_children_have_review_frequency_set(visited)
    }
}

impl ShouldChildrenHaveReviewFrequencySet for &GrowingItemNode<'_> {
    fn should_children_have_review_frequency_set<'a>(
        &'a self,
        mut visited: Vec<&'a GrowingItemNode<'a>>,
    ) -> bool {
        if self.is_finished() || visited.contains(self) {
            //Skip Finished Items & Circular reference
            false
        } else {
            visited.push(self);
            match self.get_surreal_review_guidance() {
                Some(SurrealReviewGuidance::ReviewChildrenSeparately) => true,
                Some(SurrealReviewGuidance::AlwaysReviewChildrenWithThisItem) => false,
                None => self
                    .larger
                    .should_children_have_review_frequency_set(visited),
            }
        }
    }
}

pub(crate) fn create_growing_nodes<'a>(
    items: Vec<&'a Item<'a>>,
    parent_lookup: &'a ParentLookup<'a>,
    visited: &Visited<'a, '_>,
) -> Vec<GrowingItemNode<'a>> {
    items
        .iter()
        .filter_map(|x| {
            if !visited.contains(x.get_surreal_record_id()) {
                //TODO: Add a unit test for this circular reference in smaller and bigger
                let visited = Visited::new(x.get_surreal_record_id(), Some(visited));
                Some(create_growing_node(x, parent_lookup, &visited))
            } else {
                None
            }
        })
        .collect()
}

pub(crate) fn create_growing_node<'a>(
    item: &'a Item<'a>,
    parent_lookup: &'a ParentLookup<'a>,
    visited: &Visited<'a, '_>,
) -> GrowingItemNode<'a> {
    let parents = item.find_parents(parent_lookup, visited);
    let larger = create_growing_nodes(parents, parent_lookup, visited);
    GrowingItemNode { item, larger }
}

fn calculate_dependencies<'a>(
    item: &'a Item,
    urgency_plan: &Option<UrgencyPlanWithItem<'_>>,
    all_items: &'a HashMap<&'a RecordId, Item<'a>>,
    all_events: &'a HashMap<&'a RecordId, Event<'a>>,
    smaller: &[ShrinkingItemNode<'a>],
) -> Vec<DependencyWithItem<'a>> {
    //Making smaller of type ShrinkingItemNode gets rid of the circular reference as that is checked when creating the ShrinkingItemNode
    let mut result = item
        .get_surreal_dependencies()
        .iter()
        .map(|x| match x {
            SurrealDependency::AfterDateTime(after) => {
                let after = after.clone().into();
                DependencyWithItem::AfterDateTime {
                    after,
                    is_active: item.get_now() < &after,
                }
            }
            SurrealDependency::AfterItem(after) => {
                let item = all_items.get(after).expect("All items should contain this");
                DependencyWithItem::AfterItem(item)
            }
            SurrealDependency::DuringItem(during) => {
                let item = all_items
                    .get(during)
                    .expect("All items should contain this");
                DependencyWithItem::DuringItem(item)
            }
            SurrealDependency::AfterEvent(event) => {
                let event = all_events
                    .get(event)
                    .expect("All events should contain this");
                DependencyWithItem::AfterEvent(event)
            }
        })
        .collect::<Vec<_>>();

    //If item is scheduled then I need to add that as a dependency as well
    if urgency_plan.is_scheduled_now() {
        let scheduled = urgency_plan
            .get_scheduled_now()
            .expect("is_scheduled_now is true so this should be Some");
        let after = scheduled.get_earliest_start().clone().into();
        result.push(DependencyWithItem::UntilScheduled {
            after,
            is_active: &after > item.get_now(),
        });
    }

    //If item has smaller items then those need to be added a dependencies as well
    for smaller in smaller.iter() {
        result.push(DependencyWithItem::AfterChildItem(smaller.get_item()));
    }

    //Items that are reactive are always waiting for something to happen
    if item.is_responsibility_reactive() {
        result.push(DependencyWithItem::WaitingToBeInterrupted);
    }

    result
}

fn calculate_urgency_plan<'a>(
    item: &'a Item,
    all_items: &'a HashMap<&'a RecordId, Item>,
    time_spent_log: &[TimeSpent],
) -> Option<UrgencyPlanWithItem<'a>> {
    item.get_surreal_urgency_plan().as_ref().map(|x| match x {
        SurrealUrgencyPlan::WillEscalate {
            initial,
            triggers,
            later,
        } => UrgencyPlanWithItem::WillEscalate {
            initial: initial.clone(),
            triggers: triggers
                .iter()
                .map(|x| TriggerWithItem::new(x, item.get_now_sql(), all_items, time_spent_log))
                .collect(),
            later: later.clone(),
        },
        SurrealUrgencyPlan::StaysTheSame(urgency) => {
            UrgencyPlanWithItem::StaysTheSame(urgency.clone())
        }
    })
}

fn get_time_spent_on_this<'a>(
    after: &'a DateTime<Utc>,
    items_in_scope: &'a ItemsInScopeWithItem<'a>,
    time_spent_log: &'a [TimeSpent<'a>],
) -> Box<dyn Iterator<Item = &'a TimeSpent<'a>> + 'a> {
    match items_in_scope {
        ItemsInScopeWithItem::All => Box::new(
            time_spent_log
                .iter()
                .filter(|x| x.get_started_at() >= after),
        ),
        ItemsInScopeWithItem::Include(include) => Box::new(
            time_spent_log
                .iter()
                .filter(|x| x.get_started_at() >= after && x.did_work_towards_any(include)),
        ),
        ItemsInScopeWithItem::Exclude(exclude) => Box::new(
            time_spent_log
                .iter()
                .filter(|x| x.get_started_at() >= after && !x.did_work_towards_any(exclude)),
        ),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ShrinkingItemNode<'s> {
    pub(crate) item: &'s Item<'s>,
    pub(crate) smaller: Vec<ShrinkingItemNode<'s>>,
}

impl<'s> ShrinkingItemNode<'s> {
    pub(crate) fn get_item(&self) -> &'s Item<'s> {
        self.item
    }

    pub(crate) fn get_surreal_record_id(&self) -> &RecordId {
        self.item.get_surreal_record_id()
    }

    pub(crate) fn get_children(
        &'s self,
        filter: Filter,
    ) -> Box<dyn Iterator<Item = &'s ShrinkingItemNode<'s>> + 's + Send> {
        match filter {
            Filter::All => Box::new(self.smaller.iter()),
            Filter::Active => Box::new(self.smaller.iter().filter(|x| !x.item.is_finished())),
            Filter::Finished => Box::new(self.smaller.iter().filter(|x| x.item.is_finished())),
        }
    }
}

pub(crate) fn create_shrinking_nodes<'a>(
    items: &[&'a Item<'a>],
    possible_children: &'a HashMap<&'a RecordId, Item<'a>>,
    visited: Vec<&'a RecordId>,
) -> Vec<ShrinkingItemNode<'a>> {
    items
        .iter()
        .filter_map(|x| {
            if !visited.contains(&x.get_surreal_record_id()) {
                //TODO: Add a unit test for this circular reference in smaller and bigger
                let mut visited = visited.clone();
                visited.push(x.get_surreal_record_id());
                Some(create_shrinking_node(x, possible_children, visited))
            } else {
                None
            }
        })
        .collect()
}

pub(crate) fn create_shrinking_node<'a>(
    item: &'a Item<'a>,
    all_items: &'a HashMap<&'a RecordId, Item<'a>>,
    visited: Vec<&'a RecordId>,
) -> ShrinkingItemNode<'a> {
    let children = item.find_children(all_items, &visited);
    let smaller = create_shrinking_nodes(&children, all_items, visited);
    ShrinkingItemNode { item, smaller }
}

fn has_parents(parents: &[GrowingItemNode<'_>], filter: Filter) -> bool {
    match filter {
        Filter::All => !parents.is_empty(),
        Filter::Active => parents.iter().any(|x| !x.item.is_finished()),
        Filter::Finished => parents.iter().any(|x| x.item.is_finished()),
    }
}

fn has_children(children: &[ShrinkingItemNode<'_>], filter: Filter) -> bool {
    match filter {
        Filter::All => !children.is_empty(),
        Filter::Active => children.iter().any(|x| !x.item.is_finished()),
        Filter::Finished => children.iter().any(|x| x.item.is_finished()),
    }
}

fn get_dependencies<'a>(
    dependencies: &'a [DependencyWithItem],
    filter: Filter,
) -> Box<dyn Iterator<Item = &'a DependencyWithItem<'a>> + 'a> {
    match filter {
        Filter::All => Box::new(dependencies.iter()),
        Filter::Active => Box::new(dependencies.iter().filter(|x| x.is_active())),
        Filter::Finished => Box::new(dependencies.iter().filter(|x| !x.is_active())),
    }
}

fn has_dependencies(dependencies: &[DependencyWithItem], filter: Filter) -> bool {
    get_dependencies(dependencies, filter).next().is_some()
}

fn calculate_urgent_action_items<'a>(
    item: &'a Item,
    parents: &[GrowingItemNode],
    children: &[ShrinkingItemNode],
    urgency_plan: &Option<UrgencyPlanWithItem<'_>>,
    dependencies: &[DependencyWithItem],
) -> Vec<ActionWithItem<'a>> {
    let mut result = Vec::default();
    //Does it need a parent?
    if !has_parents(parents, Filter::Active) && !item.is_type_motivation() {
        result.push(ActionWithItem::ParentBackToAMotivation(item));
    }

    if item.is_type_motivation_kind_not_set() {
        result.push(ActionWithItem::ItemNeedsAClassification(item));
    }

    //Does it need to pick a review frequency?
    if !(item.has_review_frequency() && item.has_review_guidance())
        && parents.should_children_have_review_frequency_set(Default::default())
    {
        result.push(ActionWithItem::PickItemReviewFrequency(item));
    }

    //Does it need to be reviewed?
    if item.is_a_review_due() {
        result.push(ActionWithItem::ReviewItem(item));
    }

    match urgency_plan.get_urgency_now() {
        Some(SurrealUrgency::MoreUrgentThanAnythingIncludingScheduled)
        | Some(SurrealUrgency::InTheModeMaybeUrgent)
        | Some(SurrealUrgency::MoreUrgentThanMode)
        | Some(SurrealUrgency::InTheModeDefinitelyUrgent) => {
            if !has_dependencies(dependencies, Filter::Active) {
                result.push(ActionWithItem::MakeProgress(item));
            }
        }
        Some(SurrealUrgency::InTheModeScheduled(surreal_scheduled))
        | Some(SurrealUrgency::ScheduledAnyMode(surreal_scheduled)) => {
            if !has_dependencies(dependencies, Filter::Active) {
                match surreal_scheduled {
                    SurrealScheduled::Exact { start, .. } => {
                        if start <= item.get_now_sql() {
                            result.push(ActionWithItem::MakeProgress(item));
                        }
                    }
                    SurrealScheduled::Range { start_range, .. } => {
                        if &start_range.0 <= item.get_now_sql() {
                            result.push(ActionWithItem::MakeProgress(item));
                        }
                    }
                }
            }
        }
        Some(SurrealUrgency::InTheModeByImportance) => {
            //Do nothing, not urgent
        }
        None => {
            //Need to set an urgency plan
            if !has_children(children, Filter::Active) && !item.is_responsibility_reactive() {
                result.push(ActionWithItem::SetReadyAndUrgency(item));
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use crate::{
        base_data::item::ItemVecExtensions,
        calculated_data::parent_lookup::ParentLookup,
        data_storage::surrealdb_layer::{
            surreal_item::{
                SurrealDependency, SurrealItemBuilder, SurrealItemType, SurrealOrderedSubItem,
            },
            surreal_tables::SurrealTablesBuilder,
        },
        node::{Filter, item_node::ItemNode},
    };

    #[test]
    fn when_smaller_items_causes_a_circular_reference_create_growing_node_detects_this_and_terminates()
     {
        let surreal_items = vec![
            SurrealItemBuilder::default()
                .id(Some(("surreal_item", "1").into()))
                .summary("Main Item that covers something else")
                .item_type(SurrealItemType::Action)
                .smaller_items_in_priority_order(vec![SurrealOrderedSubItem::SubItem {
                    surreal_item_id: ("surreal_item", "3").into(),
                }])
                .build()
                .unwrap(),
            SurrealItemBuilder::default()
                .id(Some(("surreal_item", "2").into()))
                .summary("Item that is covered by main item and the item this covers")
                .item_type(SurrealItemType::Action)
                .smaller_items_in_priority_order(vec![SurrealOrderedSubItem::SubItem {
                    surreal_item_id: ("surreal_item", "1").into(),
                }])
                .build()
                .unwrap(),
            SurrealItemBuilder::default()
                .id(Some(("surreal_item", "3").into()))
                .summary("Item that is covers the item it is covered by, circular reference")
                .item_type(SurrealItemType::Action)
                .smaller_items_in_priority_order(vec![SurrealOrderedSubItem::SubItem {
                    surreal_item_id: ("surreal_item", "2").into(),
                }])
                .build()
                .unwrap(),
        ];
        let surreal_tables = SurrealTablesBuilder::default()
            .surreal_items(surreal_items)
            .build()
            .expect("no required fields");
        let all_time_spent = surreal_tables.make_time_spent_log().collect::<Vec<_>>();
        let now = Utc::now();
        let items = surreal_tables.make_items(&now);
        let parent_lookup = ParentLookup::new(&items);
        let active_items = items.filter_active_items();
        let events = surreal_tables.make_events();

        let to_dos = active_items
            .iter()
            .filter(|x| x.get_item_type() == &SurrealItemType::Action);
        let next_step_nodes = to_dos
            .map(|x| ItemNode::new(x, &items, &parent_lookup, &events, &all_time_spent))
            .filter(|x| !x.has_children(Filter::Active))
            .collect::<Vec<_>>();

        assert_eq!(next_step_nodes.len(), 3);
        assert_eq!(
            next_step_nodes
                .first()
                .unwrap()
                .create_parent_chain(Filter::Active)
                .len(),
            2
        );
    }

    #[test]
    fn when_you_cover_yourself_causing_a_circular_reference_create_growing_node_detects_this_and_terminates()
     {
        let surreal_items = vec![
            SurrealItemBuilder::default()
                .id(Some(("surreal_item", "1").into()))
                .summary("Main Item that covers something else")
                .item_type(SurrealItemType::Action)
                .dependencies(vec![SurrealDependency::AfterItem(
                    ("surreal_item", "1").into(),
                )])
                .build()
                .unwrap(),
            SurrealItemBuilder::default()
                .id(Some(("surreal_item", "2").into()))
                .summary("Item that is covered by main item and the item this covers")
                .item_type(SurrealItemType::Action)
                .dependencies(vec![SurrealDependency::AfterItem(
                    ("surreal_item", "2").into(),
                )])
                .build()
                .unwrap(),
            SurrealItemBuilder::default()
                .id(Some(("surreal_item", "3").into()))
                .summary("Item that is covers the item it is covered by, circular reference")
                .item_type(SurrealItemType::Action)
                .dependencies(vec![SurrealDependency::AfterItem(
                    ("surreal_item", "3").into(),
                )])
                .build()
                .unwrap(),
        ];
        let surreal_tables = SurrealTablesBuilder::default()
            .surreal_items(surreal_items)
            .build()
            .expect("no required fields");
        let all_time_spent = surreal_tables.make_time_spent_log().collect::<Vec<_>>();
        let now = Utc::now();
        let items = surreal_tables.make_items(&now);
        let parent_lookup = ParentLookup::new(&items);
        let active_items = items.filter_active_items();
        let events = surreal_tables.make_events();

        let to_dos = active_items
            .iter()
            .filter(|x| x.get_item_type() == &SurrealItemType::Action);
        let next_step_nodes = to_dos
            .map(|x| ItemNode::new(x, &items, &parent_lookup, &events, &all_time_spent))
            .filter(|x| !x.has_children(Filter::Active))
            .collect::<Vec<_>>();

        assert_eq!(
            next_step_nodes
                .iter()
                .filter(|x| !x.has_dependencies(Filter::Active))
                .count(),
            0
        );
    }
}
