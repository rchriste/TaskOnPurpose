use std::time::Duration;

use ahash::HashSet;
use chrono::{DateTime, TimeDelta, Utc};
use surrealdb::RecordId;

use crate::{
    data_storage::surrealdb_layer::surreal_time_spent::SurrealTimeSpent,
    node::why_in_scope_and_action_with_item_status::WhyInScope,
};

use super::item::Item;

#[derive(PartialEq, Eq)]
pub(crate) struct TimeSpent<'s> {
    surreal_time_spent: &'s SurrealTimeSpent,
    why_in_scope: HashSet<WhyInScope>,
    when_started: DateTime<Utc>,
    when_stopped: DateTime<Utc>,
    duration: Duration,
    worked_towards: HashSet<RecordId>,
}

impl<'s> TimeSpent<'s> {
    pub(crate) fn new(surreal_time_spent: &'s SurrealTimeSpent) -> TimeSpent<'s> {
        let when_started: DateTime<Utc> = surreal_time_spent.when_started.clone().into();
        let when_stopped: DateTime<Utc> = surreal_time_spent.when_stopped.clone().into();
        let duration = match when_stopped.signed_duration_since(when_started).to_std() {
            Ok(duration) => duration,
            Err(_) => match when_started.signed_duration_since(when_stopped).to_std() {
                Ok(duration) => duration,
                Err(err) => {
                    println!("when_started: {:?}", when_started);
                    println!("when_stopped: {:?}", when_stopped);
                    println!("Error: {:?}", err);
                    panic!("Error in TimeSpent::new");
                }
            },
        };
        let worked_towards = surreal_time_spent
            .working_on
            .iter()
            .map(|action| action.get_record_id().clone())
            .collect();

        let why_in_scope = surreal_time_spent
            .why_in_scope
            .iter()
            .map(|x| x.clone().into())
            .collect::<HashSet<WhyInScope>>();

        TimeSpent {
            surreal_time_spent,
            why_in_scope,
            when_started,
            when_stopped,
            duration,
            worked_towards,
        }
    }

    pub(crate) fn get_started_at(&self) -> &DateTime<Utc> {
        &self.surreal_time_spent.when_started
    }

    pub(crate) fn get_time_delta(&self) -> TimeDelta {
        let when_started: DateTime<Utc> = self.surreal_time_spent.when_started.clone().into();
        self.surreal_time_spent
            .when_stopped
            .signed_duration_since(when_started)
    }

    pub(crate) fn get_duration(&self) -> &Duration {
        &self.duration
    }

    pub(crate) fn is_within(&self, start: &DateTime<Utc>, end: &DateTime<Utc>) -> bool {
        self.when_started >= *start && self.when_stopped <= *end
    }

    pub(crate) fn did_work_towards_any(&self, items: &[&Item<'_>]) -> bool {
        self.surreal_time_spent.working_on.iter().any(|action| {
            items
                .iter()
                .any(|item| item.get_surreal_record_id() == action.get_record_id())
        })
    }

    pub(crate) fn worked_towards(&self) -> &HashSet<RecordId> {
        &self.worked_towards
    }

    pub(crate) fn is_urgent(&self) -> bool {
        self.why_in_scope.contains(&WhyInScope::Urgency)
    }

    pub(crate) fn is_important(&self) -> bool {
        self.why_in_scope.contains(&WhyInScope::Importance)
    }

    pub(crate) fn is_menu_navigation(&self) -> bool {
        self.why_in_scope.contains(&WhyInScope::MenuNavigation)
    }
}
