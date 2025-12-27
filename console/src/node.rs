use crate::data_storage::surrealdb_layer::surreal_item::{SurrealScheduled, SurrealUrgency};

pub(crate) mod action_with_item_status;
pub(crate) mod event_node;
pub(crate) mod item_node;
pub(crate) mod item_status;
pub(crate) mod item_status_why_in_mode;
pub(crate) mod mode_node;
pub(crate) mod urgency_level_item_with_item_status;
pub(crate) mod why_in_scope_and_action_with_item_status;

#[derive(Clone, Copy)]
pub(crate) enum Filter {
    All,
    Active,
    Finished,
}

pub(crate) enum Urgency {
    MoreUrgentThanAnythingIncludingScheduled,
    ScheduledAnyMode,
    MoreUrgentThanMode,
    InTheModeScheduled,
    InTheModeDefinitelyUrgent,
    InTheModeMaybeUrgent,
    InTheModeByImportance,
}

pub(crate) trait IsTriggered {
    fn is_triggered(&self) -> bool;
}

pub(crate) trait IsActive {
    fn is_active(&self) -> bool;
}

pub(crate) trait GetUrgencyNow {
    fn is_scheduled_now(&self) -> bool {
        self.get_scheduled_now().is_some()
    }

    fn get_scheduled_now(&self) -> Option<&SurrealScheduled> {
        match self.get_urgency_now() {
            Some(SurrealUrgency::ScheduledAnyMode(scheduled))
            | Some(SurrealUrgency::InTheModeScheduled(scheduled)) => Some(scheduled),
            Some(SurrealUrgency::MoreUrgentThanAnythingIncludingScheduled)
            | Some(SurrealUrgency::MoreUrgentThanMode)
            | Some(SurrealUrgency::InTheModeDefinitelyUrgent)
            | Some(SurrealUrgency::InTheModeMaybeUrgent)
            | Some(SurrealUrgency::InTheModeByImportance)
            | None => None,
        }
    }

    fn get_urgency_now(&self) -> Option<&SurrealUrgency>;
}
