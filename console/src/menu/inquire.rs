use fundu::{CustomDurationParser, CustomTimeUnit, SaturatingInto, TimeUnit};
use lazy_static::lazy_static;

use chrono::{DateTime, Datelike, Duration, Local, NaiveTime, TimeZone};
use crossterm::terminal;
use regex::{Regex, RegexBuilder};

pub(crate) mod back_menu;
pub(crate) mod do_now_list_menu;
pub(crate) mod item_children_summary;
pub(crate) mod select_higher_importance_than_this;
pub(crate) mod time_spent_summary;
pub(crate) mod update_item_summary;

/// Default page size for `inquire::Select` and `inquire::MultiSelect`.
///
/// We aim for roughly half of the visible terminal rows, but never less than 10.
/// If terminal size can't be detected, we fall back to 10.
#[must_use]
pub(crate) fn default_select_page_size() -> usize {
    const MIN_PAGE_SIZE: usize = 10;

    match terminal::size() {
        Ok((_cols, rows)) => {
            let half_rows = (rows as usize) / 2;
            std::cmp::max(MIN_PAGE_SIZE, half_rows)
        }
        Err(_) => MIN_PAGE_SIZE,
    }
}

#[must_use]
fn parse_exact_or_relative_datetime_help_string() -> &'static str {
    concat!(
        "Enter an exact time or a time relative to now. Examples:\n",
        "\"3:00pm\" or \"3pm\", for today at 3:00pm\n",
        "\"Today 3pm\" or \"Day 3pm\" for today at 3:00pm\n",
        "\"Tomorrow 3pm\", \"Next day 3pm\", or \"Next today 3pm\" for tomorrow at 3:00pm\n",
        "\"Next next day 3pm\" for the day after tomorrow at 3:00pm\n",
        "\"Mon 3:15pm\" for Monday of this week at 3:15pm\n",
        "\"next Mon 5pm\" for next week's Monday, and \"next next Mon 5pm\" for the Monday after that\n",
        "You can also say \"last Mon 5pm\" or even \"last last Mon 5pm\"\n",
        "Full dates also work like \"1/15/2025 4:15pm\" or \"2/13/2025 4pm\"\n",
        "Relative times also work like \"30m\" or \"30min\" for in thirty minutes from now, or\n",
        "  \"1h\", \"1hour\", for in an hour, or \"1d\", \"1day\", for in a day, or\n",
        "  \"1w\", \"1week\" for in a week; you can also say \"30m ago\" or \"-30m\" to give a time in the past\n"
    )
}

fn parse_exact_or_relative_datetime(input: &str) -> Option<DateTime<Local>> {
    lazy_static! {
        static ref relative_parser: CustomDurationParser<'static> = CustomDurationParser::builder()
            .allow_time_unit_delimiter()
            .number_is_optional()
            .allow_ago()
            .allow_negative()
            .allow_sign_delimiter()
            .time_units(&[
                CustomTimeUnit::with_default(
                    TimeUnit::Second,
                    &["s", "sec", "secs", "second", "seconds"]
                ),
                CustomTimeUnit::with_default(
                    TimeUnit::Minute,
                    &["m", "min", "mins", "minute", "minutes"]
                ),
                CustomTimeUnit::with_default(TimeUnit::Hour, &["h", "hour", "hours"]),
                CustomTimeUnit::with_default(TimeUnit::Day, &["d", "day", "days"]),
                CustomTimeUnit::with_default(TimeUnit::Week, &["w", "week", "weeks"]),
            ])
            .build();
    }
    match relative_parser.parse(input) {
        Ok(exact_start) => {
            if exact_start.is_positive() {
                Some(Local::now() + exact_start.saturating_into()) //I call Local::now rather than take it as an input to keep things the same as how dateparse::parse works as it uses local time for some of the parsing and does not accept now as an input
            } else {
                //std::time::Duration does not support negative durations so we need to handle this ourselves
                Some(Local::now() - exact_start.abs().saturating_into())
            }
        }
        Err(_) => match dateparser::parse_with(
            input,
            &Local,
            NaiveTime::from_hms_opt(0, 0, 0).expect("Valid time given"),
        ) {
            Ok(exact_start) => Some(exact_start.into()),
            Err(_e) => {
                lazy_static! {
                    // Allow repeating modifiers like "next next day" or "last last Mon".
                    static ref RE: Regex = RegexBuilder::new(r"^\s*((?:(?:last|next)\s+)*)?(Monday|Mon|Tuesday|Tue|Wed|Wednesday|Thu|Thur|Thurs|Thursday|Fri|Friday|Sat|Saturday|Sun|Sunday|Tomorrow|Day|Today)\s*(([0-9]{1,2})(:[0-9]{2}(:[0-9]{2})?)?\s*(am|pm)?)?\s*$").case_insensitive(true).build().expect("Regex is valid");
                }
                if RE.is_match(input) {
                    let captures = RE.captures(input).unwrap();
                    let modifier_prefix = captures.get(1).map(|m| m.as_str()).unwrap_or("");
                    let day_of_the_week = captures
                        .get(2)
                        .expect("is_match is true and this is required")
                        .as_str();
                    lazy_static! {
                        static ref MondayRE: Regex = RegexBuilder::new(r"^\s*(Monday|Mon)")
                            .case_insensitive(true)
                            .build()
                            .expect("Regex is valid");
                        static ref TuesdayRE: Regex = RegexBuilder::new(r"^\s*(Tuesday|Tue)")
                            .case_insensitive(true)
                            .build()
                            .expect("Regex is valid");
                        static ref WednesdayRE: Regex = RegexBuilder::new(r"^\s*(Wednesday|Wed)")
                            .case_insensitive(true)
                            .build()
                            .expect("Regex is valid");
                        static ref ThursdayRE: Regex =
                            RegexBuilder::new(r"^\s*(Thursday|Thu|Thur|Thurs)")
                                .case_insensitive(true)
                                .build()
                                .expect("Regex is valid");
                        static ref FridayRE: Regex = RegexBuilder::new(r"^\s*(Friday|Fri)")
                            .case_insensitive(true)
                            .build()
                            .expect("Regex is valid");
                        static ref SaturdayRE: Regex = RegexBuilder::new(r"^\s*(Saturday|Sat)")
                            .case_insensitive(true)
                            .build()
                            .expect("Regex is valid");
                        static ref SundayRE: Regex = RegexBuilder::new(r"^\s*(Sunday|Sun)")
                            .case_insensitive(true)
                            .build()
                            .expect("Regex is valid");
                        static ref TomorrowRE: Regex = RegexBuilder::new(r"^\s*Tomorrow")
                            .case_insensitive(true)
                            .build()
                            .expect("Regex is valid");
                        static ref TodayRE: Regex = RegexBuilder::new(r"^\s*(Day|Today)")
                            .case_insensitive(true)
                            .build()
                            .expect("Regex is valid");
                    }
                    let now = Local::now();
                    let days_since_sunday_plus_one: i64 = now.weekday().number_from_sunday().into();
                    let days_since_sunday = days_since_sunday_plus_one - 1;
                    let date = if MondayRE.is_match(day_of_the_week) {
                        now.date_naive() - Duration::days(days_since_sunday) + Duration::days(1)
                    } else if TuesdayRE.is_match(day_of_the_week) {
                        now.date_naive() - Duration::days(days_since_sunday) + Duration::days(2)
                    } else if WednesdayRE.is_match(day_of_the_week) {
                        now.date_naive() - Duration::days(days_since_sunday) + Duration::days(3)
                    } else if ThursdayRE.is_match(day_of_the_week) {
                        now.date_naive() - Duration::days(days_since_sunday) + Duration::days(4)
                    } else if FridayRE.is_match(day_of_the_week) {
                        now.date_naive() - Duration::days(days_since_sunday) + Duration::days(5)
                    } else if SaturdayRE.is_match(day_of_the_week) {
                        now.date_naive() - Duration::days(days_since_sunday) + Duration::days(6)
                    } else if SundayRE.is_match(day_of_the_week) {
                        now.date_naive() - Duration::days(days_since_sunday) + Duration::days(0)
                    } else if TomorrowRE.is_match(day_of_the_week) {
                        now.date_naive() + Duration::days(1)
                    } else if TodayRE.is_match(day_of_the_week) {
                        now.date_naive()
                    } else {
                        panic!(
                            "This should not be possible as the regex should only match if it is one of the days of the week"
                        )
                    };

                    // Treat "next"/"last" as modifiers on the base unit.
                    // - weekdays: each modifier step is +/- 1 week from the current-week anchor
                    // - day/today/tomorrow: each modifier step is +/- 1 day
                    let modifier_steps: i64 = modifier_prefix
                        .split_whitespace()
                        .map(|w| w.to_ascii_lowercase())
                        .map(|w| match w.as_str() {
                            "next" => 1,
                            "last" => -1,
                            _ => 0,
                        })
                        .sum();

                    let is_weekday = MondayRE.is_match(day_of_the_week)
                        || TuesdayRE.is_match(day_of_the_week)
                        || WednesdayRE.is_match(day_of_the_week)
                        || ThursdayRE.is_match(day_of_the_week)
                        || FridayRE.is_match(day_of_the_week)
                        || SaturdayRE.is_match(day_of_the_week)
                        || SundayRE.is_match(day_of_the_week);

                    let delta_days = if is_weekday { 7 } else { 1 };
                    let date = date + Duration::days(modifier_steps * delta_days);

                    if captures.get(3).is_none() {
                        Some(
                            Local
                                .from_local_datetime(
                                    &date.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap()),
                                )
                                .unwrap(),
                        )
                    } else {
                        let parse_this = if captures.get(4).is_some()
                            && captures.get(5).is_none()
                            && captures.get(6).is_none()
                            && captures.get(7).is_some()
                        {
                            //The time is given as a number and am/pm for example "5pm" however the Chrono library will give an error message and won't be able to parse this direct and we want to default to the start of the hour so add ":00" into it.
                            format!(
                                "{}:00{}",
                                captures
                                    .get(4)
                                    .expect("is_some is true and this is required")
                                    .as_str(),
                                captures
                                    .get(7)
                                    .expect("is_some is true and this is required")
                                    .as_str()
                            )
                        } else {
                            captures
                                .get(3)
                                .expect("is_some is true and this is required")
                                .as_str()
                                .to_string()
                        };
                        let local = &Local;
                        NaiveTime::parse_from_str(&parse_this, "%I:%M %P")
                            .or_else(|_| NaiveTime::parse_from_str(&parse_this, "%H:%M"))
                            .or_else(|_| NaiveTime::parse_from_str(&parse_this, "%H:%M:%S"))
                            .map(|time| local.from_local_datetime(&date.and_time(time)).unwrap())
                            .ok()
                    }
                } else {
                    None
                }
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Datelike, Days, Duration, Local, NaiveTime, TimeZone};

    use super::parse_exact_or_relative_datetime;

    #[test]
    fn test_parse_exact_or_relative_datetime_each_weekday_can_be_typed_in() {
        let now = Local::now();
        let days_since_sunday_plus_one: i64 = now.weekday().number_from_sunday().into();
        let days_since_sunday = days_since_sunday_plus_one - 1;
        let sunday = now.date_naive() - Duration::days(days_since_sunday) + Duration::days(0);
        let monday = now.date_naive() - Duration::days(days_since_sunday) + Duration::days(1);
        let tuesday = now.date_naive() - Duration::days(days_since_sunday) + Duration::days(2);
        let wednesday = now.date_naive() - Duration::days(days_since_sunday) + Duration::days(3);
        let thursday = now.date_naive() - Duration::days(days_since_sunday) + Duration::days(4);
        let friday = now.date_naive() - Duration::days(days_since_sunday) + Duration::days(5);
        let saturday = now.date_naive() - Duration::days(days_since_sunday) + Duration::days(6);

        assert_eq!(
            parse_exact_or_relative_datetime("Mon"),
            Some(
                Local
                    .from_local_datetime(
                        &monday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("Monday"),
            Some(
                Local
                    .from_local_datetime(
                        &monday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("MON"),
            Some(
                Local
                    .from_local_datetime(
                        &monday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("MONDAY"),
            Some(
                Local
                    .from_local_datetime(
                        &monday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("Mon 5pm"),
            Some(
                Local
                    .from_local_datetime(
                        &monday.and_time(NaiveTime::from_hms_opt(17, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("Mon 5:17pm"),
            Some(
                Local
                    .from_local_datetime(
                        &monday.and_time(NaiveTime::from_hms_opt(17, 17, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("Mon 17:17"),
            Some(
                Local
                    .from_local_datetime(
                        &monday.and_time(NaiveTime::from_hms_opt(17, 17, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("Mon 17:17:30"),
            Some(
                Local
                    .from_local_datetime(
                        &monday.and_time(NaiveTime::from_hms_opt(17, 17, 30).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("Tue"),
            Some(
                Local
                    .from_local_datetime(
                        &tuesday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("Tuesday"),
            Some(
                Local
                    .from_local_datetime(
                        &tuesday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("TUE"),
            Some(
                Local
                    .from_local_datetime(
                        &tuesday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("TUESDAY"),
            Some(
                Local
                    .from_local_datetime(
                        &tuesday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("Wed"),
            Some(
                Local
                    .from_local_datetime(
                        &wednesday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("Wednesday"),
            Some(
                Local
                    .from_local_datetime(
                        &wednesday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("WED"),
            Some(
                Local
                    .from_local_datetime(
                        &wednesday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("WEDNESDAY"),
            Some(
                Local
                    .from_local_datetime(
                        &wednesday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("Thu"),
            Some(
                Local
                    .from_local_datetime(
                        &thursday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("Thursday"),
            Some(
                Local
                    .from_local_datetime(
                        &thursday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("THU"),
            Some(
                Local
                    .from_local_datetime(
                        &thursday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("THURSDAY"),
            Some(
                Local
                    .from_local_datetime(
                        &thursday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("Fri"),
            Some(
                Local
                    .from_local_datetime(
                        &friday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("Friday"),
            Some(
                Local
                    .from_local_datetime(
                        &friday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("FRI"),
            Some(
                Local
                    .from_local_datetime(
                        &friday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("FRIDAY"),
            Some(
                Local
                    .from_local_datetime(
                        &friday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("Sat"),
            Some(
                Local
                    .from_local_datetime(
                        &saturday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("Saturday"),
            Some(
                Local
                    .from_local_datetime(
                        &saturday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("SAT"),
            Some(
                Local
                    .from_local_datetime(
                        &saturday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("SATURDAY"),
            Some(
                Local
                    .from_local_datetime(
                        &saturday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("Sun"),
            Some(
                Local
                    .from_local_datetime(
                        &sunday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("Sunday"),
            Some(
                Local
                    .from_local_datetime(
                        &sunday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("SUN"),
            Some(
                Local
                    .from_local_datetime(
                        &sunday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("SUNDAY"),
            Some(
                Local
                    .from_local_datetime(
                        &sunday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );
    }

    #[test]
    fn test_parse_exact_or_relative_datetime_each_next_weekday_can_be_typed_in() {
        let now = Local::now();
        let days_since_sunday_plus_one: i64 = now.weekday().number_from_sunday().into();
        let days_since_sunday = days_since_sunday_plus_one - 1;
        let next_sunday = now.date_naive() + Duration::days(7) - Duration::days(days_since_sunday)
            + Duration::days(0);
        let next_monday = now.date_naive() + Duration::days(7) - Duration::days(days_since_sunday)
            + Duration::days(1);
        let next_tuesday = now.date_naive() + Duration::days(7) - Duration::days(days_since_sunday)
            + Duration::days(2);
        let next_wednesday = now.date_naive() + Duration::days(7)
            - Duration::days(days_since_sunday)
            + Duration::days(3);
        let next_thursday = now.date_naive() + Duration::days(7)
            - Duration::days(days_since_sunday)
            + Duration::days(4);
        let next_friday = now.date_naive() + Duration::days(7) - Duration::days(days_since_sunday)
            + Duration::days(5);
        let next_saturday = now.date_naive() + Duration::days(7)
            - Duration::days(days_since_sunday)
            + Duration::days(6);

        assert_eq!(
            parse_exact_or_relative_datetime("next Mon"),
            Some(
                Local
                    .from_local_datetime(
                        &next_monday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next next Mon"),
            Some(
                Local
                    .from_local_datetime(
                        &(next_monday + Duration::days(7))
                            .and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("Next Monday"),
            Some(
                Local
                    .from_local_datetime(
                        &next_monday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("NEXT MON"),
            Some(
                Local
                    .from_local_datetime(
                        &next_monday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("NEXT MONDAY"),
            Some(
                Local
                    .from_local_datetime(
                        &next_monday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("Next Mon 5pm"),
            Some(
                Local
                    .from_local_datetime(
                        &next_monday.and_time(NaiveTime::from_hms_opt(17, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next Mon 5:17pm"),
            Some(
                Local
                    .from_local_datetime(
                        &next_monday.and_time(NaiveTime::from_hms_opt(17, 17, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next Mon 17:17"),
            Some(
                Local
                    .from_local_datetime(
                        &next_monday.and_time(NaiveTime::from_hms_opt(17, 17, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next Mon 17:17:30"),
            Some(
                Local
                    .from_local_datetime(
                        &next_monday.and_time(NaiveTime::from_hms_opt(17, 17, 30).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next Tue"),
            Some(
                Local
                    .from_local_datetime(
                        &next_tuesday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next Tuesday"),
            Some(
                Local
                    .from_local_datetime(
                        &next_tuesday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next TUE"),
            Some(
                Local
                    .from_local_datetime(
                        &next_tuesday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next TUESDAY"),
            Some(
                Local
                    .from_local_datetime(
                        &next_tuesday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next Wed"),
            Some(
                Local
                    .from_local_datetime(
                        &next_wednesday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next Wednesday"),
            Some(
                Local
                    .from_local_datetime(
                        &next_wednesday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next WED"),
            Some(
                Local
                    .from_local_datetime(
                        &next_wednesday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next WEDNESDAY"),
            Some(
                Local
                    .from_local_datetime(
                        &next_wednesday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next Thu"),
            Some(
                Local
                    .from_local_datetime(
                        &next_thursday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next Thursday"),
            Some(
                Local
                    .from_local_datetime(
                        &next_thursday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next THU"),
            Some(
                Local
                    .from_local_datetime(
                        &next_thursday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next THURSDAY"),
            Some(
                Local
                    .from_local_datetime(
                        &next_thursday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next Fri"),
            Some(
                Local
                    .from_local_datetime(
                        &next_friday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next Friday"),
            Some(
                Local
                    .from_local_datetime(
                        &next_friday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next FRI"),
            Some(
                Local
                    .from_local_datetime(
                        &next_friday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next FRIDAY"),
            Some(
                Local
                    .from_local_datetime(
                        &next_friday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next Sat"),
            Some(
                Local
                    .from_local_datetime(
                        &next_saturday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next Saturday"),
            Some(
                Local
                    .from_local_datetime(
                        &next_saturday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next SAT"),
            Some(
                Local
                    .from_local_datetime(
                        &next_saturday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next SATURDAY"),
            Some(
                Local
                    .from_local_datetime(
                        &next_saturday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next Sun"),
            Some(
                Local
                    .from_local_datetime(
                        &next_sunday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next Sunday"),
            Some(
                Local
                    .from_local_datetime(
                        &next_sunday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next SUN"),
            Some(
                Local
                    .from_local_datetime(
                        &next_sunday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next SUNDAY"),
            Some(
                Local
                    .from_local_datetime(
                        &next_sunday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );
    }

    #[test]
    fn test_parse_exact_or_relative_datetime_each_last_weekday_can_be_typed_in() {
        let now = Local::now();
        let days_since_sunday_plus_one: i64 = now.weekday().number_from_sunday().into();
        let days_since_sunday = days_since_sunday_plus_one - 1;
        let last_sunday = now.date_naive() - Duration::days(7) - Duration::days(days_since_sunday)
            + Duration::days(0);
        let last_monday = now.date_naive() - Duration::days(7) - Duration::days(days_since_sunday)
            + Duration::days(1);
        let last_tuesday = now.date_naive() - Duration::days(7) - Duration::days(days_since_sunday)
            + Duration::days(2);
        let last_wednesday =
            now.date_naive() - Duration::days(7) - Duration::days(days_since_sunday)
                + Duration::days(3);
        let last_thursday =
            now.date_naive() - Duration::days(7) - Duration::days(days_since_sunday)
                + Duration::days(4);
        let last_friday = now.date_naive() - Duration::days(7) - Duration::days(days_since_sunday)
            + Duration::days(5);
        let last_saturday =
            now.date_naive() - Duration::days(7) - Duration::days(days_since_sunday)
                + Duration::days(6);

        assert_eq!(
            parse_exact_or_relative_datetime("last Mon"),
            Some(
                Local
                    .from_local_datetime(
                        &last_monday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last last Mon"),
            Some(
                Local
                    .from_local_datetime(
                        &(last_monday - Duration::days(7))
                            .and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last Monday"),
            Some(
                Local
                    .from_local_datetime(
                        &last_monday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last MON"),
            Some(
                Local
                    .from_local_datetime(
                        &last_monday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last MONDAY"),
            Some(
                Local
                    .from_local_datetime(
                        &last_monday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last Mon 5pm"),
            Some(
                Local
                    .from_local_datetime(
                        &last_monday.and_time(NaiveTime::from_hms_opt(17, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last Mon 5:17pm"),
            Some(
                Local
                    .from_local_datetime(
                        &last_monday.and_time(NaiveTime::from_hms_opt(17, 17, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last Mon 17:17"),
            Some(
                Local
                    .from_local_datetime(
                        &last_monday.and_time(NaiveTime::from_hms_opt(17, 17, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last Mon 17:17:30"),
            Some(
                Local
                    .from_local_datetime(
                        &last_monday.and_time(NaiveTime::from_hms_opt(17, 17, 30).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last Tue"),
            Some(
                Local
                    .from_local_datetime(
                        &last_tuesday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last Tuesday"),
            Some(
                Local
                    .from_local_datetime(
                        &last_tuesday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last TUE"),
            Some(
                Local
                    .from_local_datetime(
                        &last_tuesday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last TUESDAY"),
            Some(
                Local
                    .from_local_datetime(
                        &last_tuesday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last Wed"),
            Some(
                Local
                    .from_local_datetime(
                        &last_wednesday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last Wednesday"),
            Some(
                Local
                    .from_local_datetime(
                        &last_wednesday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last WED"),
            Some(
                Local
                    .from_local_datetime(
                        &last_wednesday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last WEDNESDAY"),
            Some(
                Local
                    .from_local_datetime(
                        &last_wednesday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last Thu"),
            Some(
                Local
                    .from_local_datetime(
                        &last_thursday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last Thursday"),
            Some(
                Local
                    .from_local_datetime(
                        &last_thursday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last THU"),
            Some(
                Local
                    .from_local_datetime(
                        &last_thursday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last THURSDAY"),
            Some(
                Local
                    .from_local_datetime(
                        &last_thursday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last Fri"),
            Some(
                Local
                    .from_local_datetime(
                        &last_friday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last Friday"),
            Some(
                Local
                    .from_local_datetime(
                        &last_friday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last FRI"),
            Some(
                Local
                    .from_local_datetime(
                        &last_friday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last FRIDAY"),
            Some(
                Local
                    .from_local_datetime(
                        &last_friday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last Sat"),
            Some(
                Local
                    .from_local_datetime(
                        &last_saturday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last Saturday"),
            Some(
                Local
                    .from_local_datetime(
                        &last_saturday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last SAT"),
            Some(
                Local
                    .from_local_datetime(
                        &last_saturday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last SATURDAY"),
            Some(
                Local
                    .from_local_datetime(
                        &last_saturday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last Sun"),
            Some(
                Local
                    .from_local_datetime(
                        &last_sunday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last Sunday"),
            Some(
                Local
                    .from_local_datetime(
                        &last_sunday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last SUN"),
            Some(
                Local
                    .from_local_datetime(
                        &last_sunday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last SUNDAY"),
            Some(
                Local
                    .from_local_datetime(
                        &last_sunday.and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );
    }

    #[test]
    fn test_parse_exact_or_relative_datetime_time_only_is_today_at_that_time() {
        //Just giving the time will default to today at that time
        //Saying number and am/pm will default to the start of the hour

        assert_eq!(
            parse_exact_or_relative_datetime("3:00pm"),
            Some(
                Local
                    .from_local_datetime(
                        &Local::now()
                            .date_naive()
                            .and_time(NaiveTime::from_hms_opt(15, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("3pm"),
            Some(
                Local
                    .from_local_datetime(
                        &Local::now()
                            .date_naive()
                            .and_time(NaiveTime::from_hms_opt(15, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("9:00am"),
            Some(
                Local
                    .from_local_datetime(
                        &Local::now()
                            .date_naive()
                            .and_time(NaiveTime::from_hms_opt(9, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("9am"),
            Some(
                Local
                    .from_local_datetime(
                        &Local::now()
                            .date_naive()
                            .and_time(NaiveTime::from_hms_opt(9, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );
    }

    #[test]
    fn test_parse_exact_or_relative_datetime_tomorrow_or_next_day_plus_time_is_tomorrow_at_that_time()
     {
        assert_eq!(
            parse_exact_or_relative_datetime("Tomorrow"),
            Some(
                Local
                    .from_local_datetime(
                        &Local::now()
                            .date_naive()
                            .checked_add_days(Days::new(1))
                            .expect("Test failure")
                            .and_time(NaiveTime::from_hms_opt(0, 0, 0).expect("Test failure"))
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("Next day"),
            Some(
                Local
                    .from_local_datetime(
                        &Local::now()
                            .date_naive()
                            .checked_add_days(Days::new(1))
                            .expect("Test failure")
                            .and_time(NaiveTime::from_hms_opt(0, 0, 0).expect("Test failure"))
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("Tomorrow 3pm"),
            Some(
                Local
                    .from_local_datetime(
                        &Local::now()
                            .date_naive()
                            .checked_add_days(Days::new(1))
                            .expect("Test failure")
                            .and_time(NaiveTime::from_hms_opt(15, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("Next day 3pm"),
            Some(
                Local
                    .from_local_datetime(
                        &Local::now()
                            .date_naive()
                            .checked_add_days(Days::new(1))
                            .expect("Test failure")
                            .and_time(NaiveTime::from_hms_opt(15, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        // Test case-insensitive variants of "Next day"
        assert_eq!(
            parse_exact_or_relative_datetime("NEXT DAY 3pm"),
            Some(
                Local
                    .from_local_datetime(
                        &Local::now()
                            .date_naive()
                            .checked_add_days(Days::new(1))
                            .expect("Test failure")
                            .and_time(NaiveTime::from_hms_opt(15, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next day 3pm"),
            Some(
                Local
                    .from_local_datetime(
                        &Local::now()
                            .date_naive()
                            .checked_add_days(Days::new(1))
                            .expect("Test failure")
                            .and_time(NaiveTime::from_hms_opt(15, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next next day"),
            Some(
                Local
                    .from_local_datetime(
                        &Local::now()
                            .date_naive()
                            .checked_add_days(Days::new(2))
                            .expect("Test failure")
                            .and_time(NaiveTime::from_hms_opt(0, 0, 0).expect("Test failure"))
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next next day 3pm"),
            Some(
                Local
                    .from_local_datetime(
                        &Local::now()
                            .date_naive()
                            .checked_add_days(Days::new(2))
                            .expect("Test failure")
                            .and_time(NaiveTime::from_hms_opt(15, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last day"),
            Some(
                Local
                    .from_local_datetime(
                        &Local::now()
                            .date_naive()
                            .checked_sub_days(Days::new(1))
                            .expect("Test failure")
                            .and_time(NaiveTime::from_hms_opt(0, 0, 0).expect("Test failure"))
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last last day"),
            Some(
                Local
                    .from_local_datetime(
                        &Local::now()
                            .date_naive()
                            .checked_sub_days(Days::new(2))
                            .expect("Test failure")
                            .and_time(NaiveTime::from_hms_opt(0, 0, 0).expect("Test failure"))
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("last last day 3pm"),
            Some(
                Local
                    .from_local_datetime(
                        &Local::now()
                            .date_naive()
                            .checked_sub_days(Days::new(2))
                            .expect("Test failure")
                            .and_time(NaiveTime::from_hms_opt(15, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        // "Day" is the base unit; "next Day" is tomorrow.
        assert_eq!(
            parse_exact_or_relative_datetime("Day"),
            Some(
                Local
                    .from_local_datetime(
                        &Local::now()
                            .date_naive()
                            .and_time(NaiveTime::from_hms_opt(0, 0, 0).expect("Test failure"))
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next Day"),
            Some(
                Local
                    .from_local_datetime(
                        &Local::now()
                            .date_naive()
                            .checked_add_days(Days::new(1))
                            .expect("Test failure")
                            .and_time(NaiveTime::from_hms_opt(0, 0, 0).expect("Test failure"))
                    )
                    .unwrap()
            )
        );

        // "Today" is an alias for "Day"; "next today" is tomorrow.
        assert_eq!(
            parse_exact_or_relative_datetime("Today"),
            Some(
                Local
                    .from_local_datetime(
                        &Local::now()
                            .date_naive()
                            .and_time(NaiveTime::from_hms_opt(0, 0, 0).expect("Test failure"))
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("Today 3pm"),
            Some(
                Local
                    .from_local_datetime(
                        &Local::now()
                            .date_naive()
                            .and_time(NaiveTime::from_hms_opt(15, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next today"),
            Some(
                Local
                    .from_local_datetime(
                        &Local::now()
                            .date_naive()
                            .checked_add_days(Days::new(1))
                            .expect("Test failure")
                            .and_time(NaiveTime::from_hms_opt(0, 0, 0).expect("Test failure"))
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("next today 3pm"),
            Some(
                Local
                    .from_local_datetime(
                        &Local::now()
                            .date_naive()
                            .checked_add_days(Days::new(1))
                            .expect("Test failure")
                            .and_time(NaiveTime::from_hms_opt(15, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("Tomorrow 3:00pm"),
            Some(
                Local
                    .from_local_datetime(
                        &Local::now()
                            .date_naive()
                            .checked_add_days(Days::new(1))
                            .expect("Test failure")
                            .and_time(NaiveTime::from_hms_opt(15, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("Tomorrow 9am"),
            Some(
                Local
                    .from_local_datetime(
                        &Local::now()
                            .date_naive()
                            .checked_add_days(Days::new(1))
                            .expect("Test failure")
                            .and_time(NaiveTime::from_hms_opt(9, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("Tomorrow 9:00am"),
            Some(
                Local
                    .from_local_datetime(
                        &Local::now()
                            .date_naive()
                            .checked_add_days(Days::new(1))
                            .expect("Test failure")
                            .and_time(NaiveTime::from_hms_opt(9, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("Tomorrow 19:00"),
            Some(
                Local
                    .from_local_datetime(
                        &Local::now()
                            .date_naive()
                            .checked_add_days(Days::new(1))
                            .expect("Test failure")
                            .and_time(NaiveTime::from_hms_opt(19, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );
    }

    #[test]
    fn test_parse_exact_or_relative_datetime_last_day_patterns() {
        // Test "last day" returns yesterday
        assert_eq!(
            parse_exact_or_relative_datetime("last day"),
            Some(
                Local
                    .from_local_datetime(
                        &Local::now()
                            .date_naive()
                            .checked_sub_days(Days::new(1))
                            .expect("Test failure")
                            .and_time(NaiveTime::from_hms_opt(0, 0, 0).expect("Test failure"))
                    )
                    .unwrap()
            )
        );

        // Test "last last day" returns the day before yesterday
        assert_eq!(
            parse_exact_or_relative_datetime("last last day"),
            Some(
                Local
                    .from_local_datetime(
                        &Local::now()
                            .date_naive()
                            .checked_sub_days(Days::new(2))
                            .expect("Test failure")
                            .and_time(NaiveTime::from_hms_opt(0, 0, 0).expect("Test failure"))
                    )
                    .unwrap()
            )
        );

        // Test "last day" with time
        assert_eq!(
            parse_exact_or_relative_datetime("last day 3pm"),
            Some(
                Local
                    .from_local_datetime(
                        &Local::now()
                            .date_naive()
                            .checked_sub_days(Days::new(1))
                            .expect("Test failure")
                            .and_time(NaiveTime::from_hms_opt(15, 0, 0).unwrap())
                    )
                    .unwrap()
            )
        );

        // Test "last today" (another way to say yesterday)
        assert_eq!(
            parse_exact_or_relative_datetime("last today"),
            Some(
                Local
                    .from_local_datetime(
                        &Local::now()
                            .date_naive()
                            .checked_sub_days(Days::new(1))
                            .expect("Test failure")
                            .and_time(NaiveTime::from_hms_opt(0, 0, 0).expect("Test failure"))
                    )
                    .unwrap()
            )
        );

        // Test case-insensitive variants
        assert_eq!(
            parse_exact_or_relative_datetime("LAST DAY"),
            Some(
                Local
                    .from_local_datetime(
                        &Local::now()
                            .date_naive()
                            .checked_sub_days(Days::new(1))
                            .expect("Test failure")
                            .and_time(NaiveTime::from_hms_opt(0, 0, 0).expect("Test failure"))
                    )
                    .unwrap()
            )
        );
    }

    #[test]
    fn test_parse_exact_or_relative_datetime_when_the_times_are_relative() {
        let input_and_expected = vec![
            ("30s", Duration::seconds(30)),
            ("30 s", Duration::seconds(30)),
            ("30sec", Duration::seconds(30)),
            ("30 sec", Duration::seconds(30)),
            ("30seconds", Duration::seconds(30)),
            ("30 seconds", Duration::seconds(30)),
            ("-30s", Duration::seconds(-30)),
            ("30s ago", Duration::seconds(-30)),
            ("30m", Duration::minutes(30)),
            ("30 m", Duration::minutes(30)),
            ("30min", Duration::minutes(30)),
            ("30 min", Duration::minutes(30)),
            ("30minutes", Duration::minutes(30)),
            ("30 minutes", Duration::minutes(30)),
            ("30m ago", Duration::minutes(-30)),
            ("1h", Duration::hours(1)),
            ("1 h", Duration::hours(1)),
            ("1hour", Duration::hours(1)),
            ("1 hour", Duration::hours(1)),
            ("2hours", Duration::hours(2)),
            ("2 hours", Duration::hours(2)),
            ("2h ago", Duration::hours(-2)),
            ("1d", Duration::days(1)),
            ("1 d", Duration::days(1)),
            ("1day", Duration::days(1)),
            ("1 day", Duration::days(1)),
            ("2days", Duration::days(2)),
            ("2 days", Duration::days(2)),
            ("1w", Duration::weeks(1)),
            ("1 w", Duration::weeks(1)),
            ("1week", Duration::weeks(1)),
            ("1 week", Duration::weeks(1)),
            ("2weeks", Duration::weeks(2)),
            ("2 weeks", Duration::weeks(2)),
            ("1w ago", Duration::weeks(-1)),
        ];

        for (input, expected) in input_and_expected {
            println!("input={:?}", input);
            let dut = parse_exact_or_relative_datetime(input);
            let expected = Local::now() + expected;
            println!("dut={:?}", dut);
            println!("expected={:?}", expected);
            assert!(dut.expect("Should parse, Test failure") - expected < Duration::seconds(1));
        }
    }

    #[test]
    fn test_parse_exact_or_relative_datetime_just_a_date_is_the_start_of_that_day() {
        //Just giving a date will default to the start of that day

        assert_eq!(
            parse_exact_or_relative_datetime("12/15/2024"),
            Some(
                Local
                    .with_ymd_and_hms(2024, 12, 15, 0, 0, 0)
                    .earliest()
                    .unwrap()
            )
        );

        assert_eq!(
            parse_exact_or_relative_datetime("1/15/2025"),
            Some(
                Local
                    .with_ymd_and_hms(2025, 1, 15, 0, 0, 0)
                    .earliest()
                    .unwrap()
            )
        );
    }

    #[test]
    fn test_parse_exact_or_relative_datetime_writing_a_complete_datetime_with_a_full_time() {
        assert_eq!(
            parse_exact_or_relative_datetime("1/15/2025 3:00pm"),
            Some(
                Local
                    .with_ymd_and_hms(2025, 1, 15, 15, 0, 0)
                    .earliest()
                    .unwrap()
            )
        );
    }

    #[test]
    fn test_parse_exact_or_relative_datetime_writing_a_complete_datetime_with_a_short_time() {
        assert_eq!(
            parse_exact_or_relative_datetime("1/15/2025 3pm"),
            Some(
                Local
                    .with_ymd_and_hms(2025, 1, 15, 15, 0, 0)
                    .earliest()
                    .unwrap()
            )
        );
    }

    #[test]
    fn test_parse_exact_or_relative_datetime_an_invalid_date_returns_none() {
        assert_eq!(parse_exact_or_relative_datetime("invalid date"), None);
    }
}
