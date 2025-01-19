use ahash::HashMap;
use chrono::{DateTime, Utc};
use surrealdb::{opt::RecordId, sql::Datetime};

use crate::{
    data_storage::surrealdb_layer::surreal_in_the_moment_priority::{
        SurrealInTheMomentPriority, SurrealPriorityKind,
    },
    node::{
        IsTriggered,
        action_with_item_status::ActionWithItemStatus,
        item_node::{ItemNode, TriggerWithItem},
        item_status::{ItemStatus, TriggerWithItemNode},
        mode_node::ModeNode
    },
};

use super::{item::Item, time_spent::TimeSpent};

pub(crate) struct InTheMomentPriorityWithItemAction<'s> {
    surreal_in_the_moment_priority: &'s SurrealInTheMomentPriority,
    in_effect_until: Vec<TriggerWithItemNode<'s>>,
    choice: ActionWithItemStatus<'s>,
    not_chosen: Vec<ActionWithItemStatus<'s>>,
    created: DateTime<Utc>,
}

impl<'s> InTheMomentPriorityWithItemAction<'s> {
    pub(crate) fn new(
        surreal_in_the_moment_priority: &'s SurrealInTheMomentPriority,
        now_sql: &Datetime,
        all_items: &'s HashMap<&'s RecordId, Item<'s>>,
        all_nodes: &'s HashMap<&'s RecordId, ItemNode<'s>>,
        items_status: &'s HashMap<&'s RecordId, ItemStatus<'s>>,
        all_modes: &'s [ModeNode<'s>],
        time_spent_log: &[TimeSpent<'_>],
    ) -> InTheMomentPriorityWithItemAction<'s> {
        let in_effect_until = surreal_in_the_moment_priority
            .in_effect_until
            .iter()
            .map(|trigger| {
                let trigger = TriggerWithItem::new(trigger, now_sql, all_items, time_spent_log);
                TriggerWithItemNode::new(&trigger, all_nodes)
            })
            .collect();
        let choice = ActionWithItemStatus::from_surreal_action(
            &surreal_in_the_moment_priority.choice,
            items_status,
            all_modes,
        );
        let not_chosen = surreal_in_the_moment_priority
            .not_chosen
            .iter()
            .map(|action| {
                ActionWithItemStatus::from_surreal_action(action, items_status, all_modes)
            })
            .collect();
        let created = surreal_in_the_moment_priority.created.clone().into();

        InTheMomentPriorityWithItemAction {
            surreal_in_the_moment_priority,
            in_effect_until,
            choice,
            not_chosen,
            created,
        }
    }

    pub(crate) fn get_choice(&self) -> &ActionWithItemStatus<'s> {
        &self.choice
    }

    pub(crate) fn get_not_chosen(&self) -> &[ActionWithItemStatus<'s>] {
        &self.not_chosen
    }

    pub(crate) fn get_kind(&self) -> &SurrealPriorityKind {
        &self.surreal_in_the_moment_priority.kind
    }

    pub(crate) fn is_active(&self) -> bool {
        !self.in_effect_until.iter().any(|x| x.is_triggered())
    }

    pub(crate) fn get_created(&self) -> &DateTime<Utc> {
        &self.created
    }
}
