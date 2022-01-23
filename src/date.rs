use anyhow::{bail, Error};
use clap::ArgMatches;
use log::{debug, warn};
use regex::Regex;
use std::{convert::TryFrom, str::FromStr};
use time::{format_description::{modifier::*, Component, FormatItem::*},
           parsing::Parsed,
           Date, Duration, OffsetDateTime, UtcOffset};

/// Get the UtcOffset to parse/display datetimes with.
/// Needs to be called before starting extra threads.
pub fn get_utcoffset(matches: &ArgMatches) -> UtcOffset {
    if matches.is_present("utc") {
        UtcOffset::UTC
    } else {
        UtcOffset::current_local_offset().unwrap_or_else(|e| {
                                             warn!("Falling back to UTC: {}", e);
                                             UtcOffset::UTC
                                         })
    }
}

/// Parse datetime in various formats, returning unix timestamp
pub fn parse_date(s: &str, offset: UtcOffset) -> Result<i64, String> {
    let s = s.trim();
    i64::from_str(s).or_else(|e| {
                        debug!("{}: bad timestamp: {}", s, e);
                        parse_date_yyyymmdd(s, offset)
                    })
                    .or_else(|e| {
                        debug!("{}: bad absolute date: {}", s, e);
                        parse_date_ago(s)
                    })
                    .map_err(|e| {
                        debug!("{}: bad relative date: {}", s, e);
                        format!("Couldn't parse {:#?}, check examples in --help", s)
                    })
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
fn parse_date_yyyymmdd(s: &str, offset: UtcOffset) -> Result<i64, Error> {
    let mut p = Parsed::new().with_hour_24(0)
                             .unwrap()
                             .with_minute(0)
                             .unwrap()
                             .with_second(0)
                             .unwrap()
                             .with_offset_hour(offset.whole_hours())
                             .unwrap()
                             .with_offset_minute(offset.minutes_past_hour().abs() as u8)
                             .unwrap()
                             .with_offset_second(offset.seconds_past_minute().abs() as u8)
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
    use std::convert::TryInto;
    use time::format_description::well_known::Rfc3339;

    #[test]
    fn date() {
        let then =
            OffsetDateTime::parse("2018-04-03T00:00:00Z", &Rfc3339).unwrap().unix_timestamp();
        let now = epoch_now();
        let (day, hour, min) = (60 * 60 * 24, 60 * 60, 60);
        let tz_utc = UtcOffset::UTC;

        // Absolute dates
        assert_eq!(Ok(then), parse_date(" 1522713600 ", tz_utc));
        assert_eq!(Ok(then), parse_date(" 2018-04-03 ", tz_utc));
        assert_eq!(Ok(then + hour + min), parse_date("2018-04-03 01:01", tz_utc));
        assert_eq!(Ok(then + hour + min + 1), parse_date("2018-04-03 01:01:01", tz_utc));
        assert_eq!(Ok(then + hour + min + 1), parse_date("2018-04-03T01:01:01", tz_utc));

        // Different timezone (not calling `get_utcoffset()` because tests are threaded, which makes
        // `UtcOffset::current_local_offset()` error out)
        for secs in [hour, -1 * hour, 90 * min, -90 * min] {
            let offset = dbg!(UtcOffset::from_whole_seconds(secs.try_into().unwrap()).unwrap());
            assert_eq!(Ok(then - secs), parse_date("2018-04-03T00:00", offset));
        }

        // Relative dates
        assert_eq!(Ok(now - hour - 3 * day - 45), parse_date("1 hour, 3 days  45sec", tz_utc));
        assert_eq!(Ok(now - 5 * 7 * day), parse_date("5 weeks", tz_utc));

        // Failure cases
        assert!(parse_date("", tz_utc).is_err());
        assert!(parse_date("junk2018-04-03T01:01:01", tz_utc).is_err());
        assert!(parse_date("2018-04-03T01:01:01junk", tz_utc).is_err());
        assert!(parse_date("152271000o", tz_utc).is_err());
        assert!(parse_date("1 day 3 centuries", tz_utc).is_err());
        assert!(parse_date("a while ago", tz_utc).is_err());
    }
}
