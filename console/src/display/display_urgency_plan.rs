use std::fmt::{Display, Formatter};

use chrono::{DateTime, Local, Utc};

use crate::{
    data_storage::surrealdb_layer::surreal_item::{SurrealScheduled, SurrealUrgency},
    display::display_duration_one_unit::DisplayDurationOneUnit,
    node::{
        Filter,
        item_status::{ItemsInScopeWithItemNode, TriggerWithItemNode, UrgencyPlanWithItemNode},
    },
};

use super::{
    DisplayStyle,
    display_item_node::{DisplayFormat, DisplayItemNode},
};

pub(crate) struct DisplayUrgencyPlan<'s> {
    urgency_plan: &'s Option<UrgencyPlanWithItemNode<'s>>,
    filter: Filter,
    display_format: DisplayFormat,
}

impl Display for DisplayUrgencyPlan<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.urgency_plan {
            Some(UrgencyPlanWithItemNode::StaysTheSame(urgency)) => {
                let display_urgency = DisplayUrgency::new(urgency, DisplayStyle::Full);
                write!(f, "Always: {}", display_urgency)
            }
            Some(UrgencyPlanWithItemNode::WillEscalate {
                initial,
                triggers,
                later,
            }) => {
                let display_initial = DisplayUrgency::new(initial, DisplayStyle::Full);
                let display_later = DisplayUrgency::new(later, DisplayStyle::Full);
                write!(
                    f,
                    "Starts at: {}, then escalates to: {}, Escalation triggers: ",
                    display_initial, display_later,
                )?;
                if triggers.is_empty() {
                    write!(f, "Immediately")?;
                } else {
                    for trigger in triggers.iter() {
                        match trigger {
                            TriggerWithItemNode::WallClockDateTime {
                                after,
                                is_triggered,
                            } => {
                                if *is_triggered {
                                    write!(f, "[Already happened] ")?;
                                }
                                let trigger: DateTime<Local> = after.with_timezone(&Local);
                                write!(f, "After: {} ", trigger.format("%I:%M %P"))?;
                            }
                            TriggerWithItemNode::LoggedInvocationCount {
                                starting: _starting,
                                count_needed,
                                current_count,
                                items_in_scope,
                            } => {
                                write!(
                                    f,
                                    "Working on other items currently at {} of {} times.",
                                    current_count, count_needed
                                )?;

                                match items_in_scope {
                                    ItemsInScopeWithItemNode::All => {
                                        write!(f, " Any item")?;
                                    }
                                    ItemsInScopeWithItemNode::Include(include) => {
                                        write!(f, " Item worked on must be one of: ")?;
                                        for (count, item) in include.iter().enumerate() {
                                            let display = DisplayItemNode::new(
                                                item,
                                                self.filter,
                                                self.display_format,
                                            );
                                            write!(
                                                f,
                                                "({} of {}) {}, ",
                                                count + 1,
                                                include.len(),
                                                display
                                            )?;
                                        }
                                    }
                                    ItemsInScopeWithItemNode::Exclude(exclude) => {
                                        write!(f, " Item worked on must not be one of: ")?;
                                        for (count, item) in exclude.iter().enumerate() {
                                            let display = DisplayItemNode::new(
                                                item,
                                                self.filter,
                                                self.display_format,
                                            );
                                            write!(
                                                f,
                                                "({} of {}) {}, ",
                                                count + 1,
                                                exclude.len(),
                                                display
                                            )?;
                                        }
                                    }
                                }
                            }
                            TriggerWithItemNode::LoggedAmountOfTime {
                                starting: _starting,
                                duration_needed,
                                current_duration,
                                items_in_scope,
                            } => {
                                let display_duration_needed =
                                    DisplayDurationOneUnit::new(duration_needed);
                                let display_current_duration =
                                    DisplayDurationOneUnit::new(current_duration);

                                write!(
                                    f,
                                    "Working on other items currently for {} of {}.",
                                    display_current_duration, display_duration_needed
                                )?;

                                match items_in_scope {
                                    ItemsInScopeWithItemNode::All => {
                                        write!(f, " Any item")?;
                                    }
                                    ItemsInScopeWithItemNode::Include(include) => {
                                        write!(f, " Item worked on must be one of: ")?;
                                        for (count, item) in include.iter().enumerate() {
                                            let display = DisplayItemNode::new(
                                                item,
                                                self.filter,
                                                self.display_format,
                                            );
                                            write!(
                                                f,
                                                "({} of {}) {}, ",
                                                count + 1,
                                                include.len(),
                                                display
                                            )?;
                                        }
                                    }
                                    ItemsInScopeWithItemNode::Exclude(exclude) => {
                                        write!(f, " Item worked on must not be one of: ")?;
                                        for (count, item) in exclude.iter().enumerate() {
                                            let display = DisplayItemNode::new(
                                                item,
                                                self.filter,
                                                self.display_format,
                                            );
                                            write!(
                                                f,
                                                "({} of {}) {}, ",
                                                count + 1,
                                                exclude.len(),
                                                display
                                            )?;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(())
            }
            None => write!(f, "Urgency plan not set"),
        }
    }
}

impl<'s> DisplayUrgencyPlan<'s> {
    pub(crate) fn new(
        urgency_plan: &'s Option<UrgencyPlanWithItemNode>,
        filter: Filter,
        display_format: DisplayFormat,
    ) -> Self {
        Self {
            urgency_plan,
            filter,
            display_format,
        }
    }
}

pub(crate) struct DisplayUrgency<'s> {
    urgency: &'s SurrealUrgency,
    style: DisplayStyle,
}

impl Display for DisplayUrgency<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.urgency {
            SurrealUrgency::MoreUrgentThanAnythingIncludingScheduled => {
                write!(f, "🚨")?;
                match self.style {
                    DisplayStyle::Abbreviated => Ok(()),
                    DisplayStyle::Full => {
                        write!(f, " More urgent than anything including scheduled")
                    }
                }
            }
            SurrealUrgency::MoreUrgentThanMode => {
                write!(f, "🔥")?;
                match self.style {
                    DisplayStyle::Abbreviated => Ok(()),
                    DisplayStyle::Full => write!(f, " More urgent than a mode"),
                }
            }
            SurrealUrgency::InTheModeByImportance => {
                write!(f, "🟢")?;
                match self.style {
                    DisplayStyle::Abbreviated => Ok(()),
                    DisplayStyle::Full => write!(f, " When in the mode, by importance"),
                }
            }
            SurrealUrgency::InTheModeDefinitelyUrgent => {
                write!(f, "🔴")?;
                match self.style {
                    DisplayStyle::Abbreviated => Ok(()),
                    DisplayStyle::Full => write!(f, " When in the mode, definitely urgent"),
                }
            }
            SurrealUrgency::InTheModeMaybeUrgent => {
                write!(f, "🟡")?;
                match self.style {
                    DisplayStyle::Abbreviated => Ok(()),
                    DisplayStyle::Full => write!(f, " When in the mode, maybe urgent"),
                }
            }
            SurrealUrgency::ScheduledAnyMode(scheduled) => {
                write!(f, "🗓️❗")?;
                match self.style {
                    DisplayStyle::Abbreviated => Ok(()),
                    DisplayStyle::Full => {
                        let display_scheduled = DisplayScheduled::new(scheduled);
                        write!(f, " Scheduled any mode: {}", display_scheduled)
                    }
                }
            }
            SurrealUrgency::InTheModeScheduled(scheduled) => {
                write!(f, "🗓️⭳")?;
                match self.style {
                    DisplayStyle::Abbreviated => Ok(()),
                    DisplayStyle::Full => {
                        let display_scheduled = DisplayScheduled::new(scheduled);
                        write!(f, " Scheduled when in the mode: {}", display_scheduled)
                    }
                }
            }
        }
    }
}

impl<'s> DisplayUrgency<'s> {
    pub(crate) fn new(urgency: &'s SurrealUrgency, style: DisplayStyle) -> Self {
        Self { urgency, style }
    }
}

struct DisplayScheduled<'s> {
    scheduled: &'s SurrealScheduled,
}

impl Display for DisplayScheduled<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.scheduled {
            SurrealScheduled::Exact { start, duration } => {
                let start: DateTime<Utc> = start.clone().into();
                let start: DateTime<Local> = start.into();
                write!(
                    f,
                    "Exact start: {} lasting {}",
                    start.format("%I:%M %P"),
                    DisplayDurationOneUnit::new(duration)
                )
            }
            SurrealScheduled::Range {
                start_range,
                duration,
            } => {
                let start_range: (DateTime<Utc>, DateTime<Utc>) =
                    (start_range.0.clone().into(), start_range.1.clone().into());
                let start_range: (DateTime<Local>, DateTime<Local>) =
                    (start_range.0.into(), start_range.1.into());
                write!(
                    f,
                    "Range start: {}-{} lasting {}",
                    start_range.0.format("%I:%M %P"),
                    start_range.1.format("%I:%M %P"),
                    DisplayDurationOneUnit::new(duration)
                )
            }
        }
    }
}

impl<'s> DisplayScheduled<'s> {
    pub(crate) fn new(scheduled: &'s SurrealScheduled) -> Self {
        Self { scheduled }
    }
}
