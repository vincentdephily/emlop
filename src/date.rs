use anyhow::{bail, Error};
use regex::Regex;
use std::{convert::TryFrom, str::FromStr};
use time::{format_description::{modifier::*, Component, FormatItem::*},
           parsing::Parsed,
           Date, Duration, OffsetDateTime};

/// Parse datetime in various formats, returning unix timestamp
// TODO: debug-log individual parsing errors
pub fn parse_date(s: &str) -> Result<i64, String> {
    let s = s.trim();
    i64::from_str(s).or_else(|_| parse_date_yyyymmdd(s))
                    .or_else(|_| parse_date_ago(s))
                    .map_err(|_| format!("Couldn't parse {:#?}, check examples in --help", s))
}

/// Parse a number of day/years/hours/etc in the past, relative to current time
fn parse_date_ago(s: &str) -> Result<i64, Error> {
    if !s.chars().all(|c| c.is_alphanumeric() || c == ' ' || c == ',') {
        bail!("Illegal char");
    }
    let mut now = OffsetDateTime::now_utc();
    let re = Regex::new("([0-9]+|[a-z]+)").expect("Bad date span regex");
    let mut tokens = re.find_iter(s);
    let mut at_least_one = false;

    // The regex gives us a list of positive integers and strings. We expect to always have a
    // number, followed by a known string.
    while let Some(t) = tokens.next() {
        at_least_one = true;
        let num: i32 = t.as_str().parse()?;
        match tokens.next().map(|m| m.as_str()).unwrap_or("") {
            "y" | "year" | "years" => {
                let d = Date::from_calendar_date(now.year() - num, now.month(), now.day())?;
                now = now.replace_date(d);
            },
            "m" | "month" | "months" => {
                let mut month = now.month();
                let mut year = now.year();
                for _ in 0..num {
                    month = month.previous();
                    if month == time::Month::December {
                        year -= 1;
                    }
                }
                let d = Date::from_calendar_date(year, month, now.day())?;
                now = now.replace_date(d);
            },
            "w" | "week" | "weeks" => now -= num * Duration::WEEK,
            "d" | "day" | "days" => now -= num * Duration::DAY,
            "h" | "hour" | "hours" => now -= num * Duration::HOUR,
            "min" | "mins" | "minute" | "minutes" => now -= num * Duration::MINUTE,
            "s" | "sec" | "secs" | "second" | "seconds" => now -= num * Duration::SECOND,
            o => bail!("bad span {:?}", o),
        };
    }

    if !at_least_one {
        bail!("No token found");
    }
    Ok(now.unix_timestamp())
}

/// Parse rfc3339-like format with added flexibility
// TODO: Default to local time offset, and add a utc option
fn parse_date_yyyymmdd(s: &str) -> Result<i64, Error> {
    let mut p = Parsed::new().with_hour_24(0)
                             .unwrap()
                             .with_minute(0)
                             .unwrap()
                             .with_second(0)
                             .unwrap()
                             .with_offset_hour(0)
                             .unwrap();
    // See <https://github.com/time-rs/time/issues/428>
    let rest = p.parse_items(s.as_bytes(), &[
        Component(Component::Year(Year::default())),
        Literal(b"-"),
        Component(Component::Month(Month::default())),
        Literal(b"-"),
        Component(Component::Day(Day::default())),
        Optional(&Compound(&[
            First(&[
                Literal(b"T"),
                Literal(b" ")
            ]),
            Component(Component::Hour(Hour::default())),
            Literal(b":"),
            Component(Component::Minute(Minute::default())),
            Optional(&Compound(&[
                Literal(b":"),
                Component(Component::Second(Second::default()))
            ]))
        ]))
    ])?;
    if !rest.is_empty() {
        bail!("Junk at end")
    }
    Ok(OffsetDateTime::try_from(p)?.unix_timestamp())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::epoch_now;
    use time::format_description::well_known::Rfc3339;

    #[test]
    fn date() {
        let then =
            OffsetDateTime::parse("2018-04-03T00:00:00Z", &Rfc3339).unwrap().unix_timestamp();
        let now = epoch_now();
        let (day, hour, min) = (60 * 60 * 24, 60 * 60, 60);

        assert_eq!(Ok(then), parse_date(" 1522713600 "));
        assert_eq!(Ok(then), parse_date(" 2018-04-03 "));
        assert_eq!(Ok(then + hour + min), parse_date("2018-04-03 01:01"));
        assert_eq!(Ok(then + hour + min + 1), parse_date("2018-04-03 01:01:01"));
        assert_eq!(Ok(then + hour + min + 1), parse_date("2018-04-03T01:01:01"));

        assert_eq!(Ok(now - hour - 3 * day - 45), parse_date("1 hour, 3 days  45sec"));
        assert_eq!(Ok(now - 5 * 7 * day), parse_date("5 weeks"));

        assert!(parse_date("").is_err());
        assert!(parse_date("junk2018-04-03T01:01:01").is_err());
        assert!(parse_date("2018-04-03T01:01:01junk").is_err());
        assert!(parse_date("152271000o").is_err());
        assert!(parse_date("1 day 3 centuries").is_err());
        assert!(parse_date("a while ago").is_err());
    }
}
