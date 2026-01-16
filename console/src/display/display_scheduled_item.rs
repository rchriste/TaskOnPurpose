use std::fmt::Display;

use better_term::Style;
use chrono::{DateTime, Local, Utc};

use crate::{
    data_storage::surrealdb_layer::surreal_item::SurrealScheduled,
    display::display_duration_one_unit::DisplayDurationOneUnit,
    systems::upcoming::scheduled_item::ScheduledItem,
};

pub(crate) struct DisplayScheduledItem<'s> {
    scheduled_item: &'s ScheduledItem<'s>,
    now_local: DateTime<Local>,
}

impl Display for DisplayScheduledItem<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        //scheduled_start in bold then +duration also in bold then start range
        //tab then name in italics
        match self
            .scheduled_item
            .get_scheduled_now()
            .expect("This should only be used for scheduled items")
        {
            SurrealScheduled::Exact {
                start: _start,
                duration,
            } => {
                let scheduled_start = self.scheduled_item.get_scheduled_start();
                let scheduled_start = scheduled_start.with_timezone(&Local);
                let now_local = self.get_now().with_timezone(&Local);
                writeln!(
                    f,
                    "{}={} lasting {}{}",
                    Style::default().bold(),
                    duration_or_time(&now_local, &scheduled_start),
                    DisplayDurationOneUnit::new(duration),
                    Style::default()
                )?;
            }
            SurrealScheduled::Range {
                start_range,
                duration,
            } => {
                let scheduled_start = self.scheduled_item.get_scheduled_start();
                let scheduled_start = scheduled_start.with_timezone(&Local);
                let start_end_of_range = start_range.1.with_timezone(&Local);
                let delay_up_to = start_end_of_range - scheduled_start;
                let delay_up_to = delay_up_to.to_std().unwrap();
                let delay_up_to = DisplayDurationOneUnit::new(&delay_up_to);
                writeln!(
                    f,
                    "{}~{} delay up to {} lasting {}{}",
                    Style::default().bold(),
                    duration_or_time(&self.now_local, &scheduled_start),
                    delay_up_to,
                    DisplayDurationOneUnit::new(duration),
                    Style::default()
                )?;
            }
        }
        writeln!(f, "\t{}", self.scheduled_item.get_summary())
    }
}

impl<'s> DisplayScheduledItem<'s> {
    pub(crate) fn new(scheduled_item: &'s ScheduledItem<'s>) -> Self {
        let now_local = scheduled_item.get_now().with_timezone(&Local);
        Self {
            scheduled_item,
            now_local,
        }
    }

    fn get_now(&self) -> &DateTime<Utc> {
        self.scheduled_item.get_now()
    }
}

fn duration_or_time(now: &DateTime<Local>, time: &DateTime<Local>) -> String {
    //Show duration if start is within two hours, otherwise show start time
    let duration = *time - now;
    if duration.num_minutes() < 1 {
        "Now".to_string()
    } else if duration.num_hours() < 2 {
        let duration = duration.to_std().unwrap();
        format!("{}", DisplayDurationOneUnit::new(&duration))
    } else {
        format!("{}", time.format("%a %d %b %Y %I:%M%P"))
    }
}
