use ahash::HashMap;
use chrono::{DateTime, Utc};
use surrealdb::{RecordId, sql::Datetime};

use crate::{
    data_storage::surrealdb_layer::{
        SurrealTrigger,
        surreal_in_the_moment_priority::{
            SurrealAction, SurrealInTheMomentPriority, SurrealPriorityKind,
        },
    },
    node::{
        IsTriggered,
        action_with_item_status::ActionWithItemStatus,
        item_node::{ItemNode, TriggerWithItem},
        item_status::{ItemStatus, TriggerWithItemNode},
    },
};

use super::{item::Item, time_spent::TimeSpent};

#[derive(Clone, Debug)]
pub(crate) struct InTheMomentPriority<'s> {
    surreal_in_the_moment_priority: &'s SurrealInTheMomentPriority,
    created: DateTime<Utc>,
}

impl<'s> InTheMomentPriority<'s> {
    pub(crate) fn new(surreal_in_the_moment_priority: &'s SurrealInTheMomentPriority) -> Self {
        Self {
            surreal_in_the_moment_priority,
            created: surreal_in_the_moment_priority.created.clone().into(),
        }
    }

    pub(crate) fn get_choice(&self) -> &SurrealAction {
        &self.surreal_in_the_moment_priority.choice
    }

    pub(crate) fn get_surreal_priority_kind(&self) -> &SurrealPriorityKind {
        &self.surreal_in_the_moment_priority.kind
    }

    pub(crate) fn get_in_effect_until(&self) -> &[SurrealTrigger] {
        &self.surreal_in_the_moment_priority.in_effect_until
    }

    pub(crate) fn get_created(&self) -> &DateTime<Utc> {
        &self.created
    }

    pub(crate) fn get_for_mode(&self) -> Option<&RecordId> {
        self.surreal_in_the_moment_priority.for_mode.as_ref()
    }
}

#[derive(Clone, Debug)]
pub(crate) enum PriorityKind<'s> {
    HighestPriority {
        not_chosen: Vec<ActionWithItemStatus<'s>>,
    },
    LowestPriority {
        not_chosen: Vec<ActionWithItemStatus<'s>>,
    },
    NotInMode,
}

impl<'s> PriorityKind<'s> {
    fn new_highest_priority(not_chosen: Vec<ActionWithItemStatus<'s>>) -> PriorityKind<'s> {
        PriorityKind::HighestPriority { not_chosen }
    }

    fn new_lowest_priority(not_chosen: Vec<ActionWithItemStatus<'s>>) -> PriorityKind<'s> {
        PriorityKind::LowestPriority { not_chosen }
    }

    fn new_not_in_mode() -> PriorityKind<'s> {
        PriorityKind::NotInMode
    }
}

pub(crate) struct InTheMomentPriorityWithItemAction<'s> {
    in_the_moment_priority: &'s InTheMomentPriority<'s>,
    in_effect_until: Vec<TriggerWithItemNode<'s>>,
    choice: ActionWithItemStatus<'s>,
    kind: PriorityKind<'s>,
    created: DateTime<Utc>,
}

impl<'s> InTheMomentPriorityWithItemAction<'s> {
    pub(crate) fn new(
        in_the_moment_priority: &'s InTheMomentPriority<'s>,
        now_sql: &Datetime,
        all_items: &'s HashMap<&'s RecordId, Item<'s>>,
        all_nodes: &'s HashMap<&'s RecordId, ItemNode<'s>>,
        items_status: &'s HashMap<&'s RecordId, ItemStatus<'s>>,
        time_spent_log: &[TimeSpent<'_>],
    ) -> InTheMomentPriorityWithItemAction<'s> {
        let in_effect_until = in_the_moment_priority
            .get_in_effect_until()
            .iter()
            .map(|trigger| {
                let trigger = TriggerWithItem::new(trigger, now_sql, all_items, time_spent_log);
                TriggerWithItemNode::new(&trigger, all_nodes)
            })
            .collect();
        let choice = ActionWithItemStatus::from_surreal_action(
            in_the_moment_priority.get_choice(),
            items_status,
        );
        let kind = match in_the_moment_priority.get_surreal_priority_kind() {
            SurrealPriorityKind::HighestPriority { not_chosen } => {
                let not_chosen = not_chosen
                    .iter()
                    .map(|action| ActionWithItemStatus::from_surreal_action(action, items_status))
                    .collect();
                PriorityKind::new_highest_priority(not_chosen)
            }
            SurrealPriorityKind::LowestPriority { not_chosen } => {
                let not_chosen = not_chosen
                    .iter()
                    .map(|action| ActionWithItemStatus::from_surreal_action(action, items_status))
                    .collect();
                PriorityKind::new_lowest_priority(not_chosen)
            }
            SurrealPriorityKind::NotInMode => PriorityKind::new_not_in_mode(),
        };
        let created = in_the_moment_priority.get_created().clone();

        InTheMomentPriorityWithItemAction {
            in_the_moment_priority,
            in_effect_until,
            choice,
            kind,
            created,
        }
    }

    pub(crate) fn get_choice(&self) -> &ActionWithItemStatus<'s> {
        &self.choice
    }

    pub(crate) fn get_priority_kind(&self) -> &PriorityKind<'s> {
        &self.kind
    }

    pub(crate) fn is_active(&self) -> bool {
        !self.in_effect_until.iter().any(|x| x.is_triggered())
    }

    pub(crate) fn get_created(&self) -> &DateTime<Utc> {
        &self.created
    }

    pub(crate) fn get_for_mode(&self) -> Option<&RecordId> {
        self.in_the_moment_priority.get_for_mode()
    }

    pub(crate) fn is_for_current_mode(&self, current_mode_id: Option<&RecordId>) -> bool {
        match (self.get_for_mode(), current_mode_id) {
            (None, _) => true,
            (Some(_), None) => false,
            (Some(for_mode), Some(current_mode_id)) => for_mode == current_mode_id,
        }
    }
}
