use crate::data_storage::surrealdb_layer::surreal_item::{SurrealScheduled, SurrealUrgency};

pub(crate) mod action_with_item_status;
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
    Crises,
    Scheduled,
    DefinitelyUrgent,
    MaybeUrgent,
    NotUrgent,
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
            Some(Some(SurrealUrgency::Scheduled(_, scheduled))) => Some(scheduled),
            Some(Some(SurrealUrgency::CrisesUrgent(_)))
            | Some(Some(SurrealUrgency::DefinitelyUrgent(_)))
            | Some(Some(SurrealUrgency::MaybeUrgent(_)))
            | None
            | Some(None) => None,
        }
    }

    /// The outside Option is for if the Urgency has been set. If None is returned then the user
    /// has not set the urgency. The inside Option is for if there is urgency. If Some(None) is returned
    /// that means that the user has explicitly set that there is no urgency.
    fn get_urgency_now(&self) -> Option<&Option<SurrealUrgency>>;
}
