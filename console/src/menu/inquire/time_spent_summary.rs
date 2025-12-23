use crate::{
    display::display_duration::DisplayDuration, node::item_status::ItemStatus,
    systems::do_now_list::DoNowList,
};

pub(crate) fn print_time_spent(menu_for: &ItemStatus<'_>, do_now_list: &DoNowList) {
    print!("Time Spent: ");
    let items = vec![menu_for.get_item()];
    let now = do_now_list.get_now();
    let time_spent = do_now_list
        .get_time_spent_log()
        .iter()
        .filter(|x| x.did_work_towards_any(&items))
        .collect::<Vec<_>>();
    if time_spent.is_empty() {
        println!("None");
    } else {
        println!();
        let a_day_ago = *now - chrono::Duration::days(1);
        let last_day = time_spent
            .iter()
            .filter(|x| x.is_within(&a_day_ago, now))
            .fold((chrono::Duration::default(), 0), |acc, x| {
                (acc.0 + x.get_time_delta(), acc.1 + 1)
            });
        let a_week_ago = *now - chrono::Duration::weeks(1);
        let last_week = time_spent
            .iter()
            .filter(|x| x.is_within(&a_week_ago, now))
            .fold((chrono::Duration::default(), 0), |acc, x| {
                (acc.0 + x.get_time_delta(), acc.1 + 1)
            });
        let a_month_ago = *now - chrono::Duration::weeks(4);
        let last_month = time_spent
            .iter()
            .filter(|x| x.is_within(&a_month_ago, now))
            .fold((chrono::Duration::default(), 0), |acc, x| {
                (acc.0 + x.get_time_delta(), acc.1 + 1)
            });
        let total = time_spent
            .iter()
            .fold((chrono::Duration::default(), 0), |acc, x| {
                (acc.0 + x.get_time_delta(), acc.1 + 1)
            });

        if last_day.1 != total.1 {
            print!("    Last Day: ");
            if last_day.1 == 0 {
                println!("None");
            } else {
                println!(
                    "{} times for {}",
                    last_day.1,
                    DisplayDuration::new(&last_day.0.to_std().expect("Can convert"))
                );
            }
        }

        if last_week.1 != last_day.1 {
            print!("    Last Week: ");
            if last_week.1 == 0 {
                println!("None");
            } else {
                println!(
                    "{} times for {}",
                    last_week.1,
                    DisplayDuration::new(&last_week.0.to_std().expect("Can convert"))
                );
            }
        }

        if last_month.1 != last_week.1 {
            print!("    Last Month: ");
            if last_month.1 == 0 {
                println!("None");
            } else {
                println!(
                    "{} times for {}",
                    last_month.1,
                    DisplayDuration::new(&last_month.0.to_std().expect("Can convert"))
                );
            }
        }

        println!(
            "    TOTAL: {} times for {}",
            total.1,
            DisplayDuration::new(&total.0.to_std().expect("Can convert"))
        );
        println!();
    }
}
