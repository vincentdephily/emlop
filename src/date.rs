use crate::Styles;
use anyhow::{bail, Error};
use log::{debug, warn};
use regex::Regex;
use std::{convert::{TryFrom, TryInto},
          str::FromStr,
          time::{SystemTime, UNIX_EPOCH}};
use time::{format_description::{modifier::*, Component, FormatItem::*},
           macros::format_description,
           parsing::Parsed,
           Date, Duration, OffsetDateTime, UtcOffset};

/// Get the UtcOffset to parse/display datetimes with.
/// Needs to be called before starting extra threads.
pub fn get_offset(utc: bool) -> UtcOffset {
    if utc {
        UtcOffset::UTC
    } else {
        UtcOffset::current_local_offset().unwrap_or_else(|e| {
                                             warn!("Falling back to UTC: {}", e);
                                             UtcOffset::UTC
                                         })
    }
}

/// Format standardized utc dates
pub fn fmt_utctime(ts: i64) -> String {
    let fmt = format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]Z");
    OffsetDateTime::from_unix_timestamp(ts).unwrap().format(&fmt).unwrap()
}

/// Format dates according to user preferences
pub fn fmt_time(ts: i64, style: &Styles) -> String {
    let fmt = format_description!("[year]-[month]-[day] [hour]:[minute]:[second] [offset_hour sign:mandatory]:[offset_minute]");
    let offset_secs = Duration::new(style.date_offset.whole_seconds().try_into().unwrap(), 0);
    OffsetDateTime::from_unix_timestamp(ts).unwrap()
                                           .replace_offset(style.date_offset)
                                           .checked_add(offset_secs)
                                           .unwrap()
                                           .format(&fmt)
                                           .unwrap()
}

pub fn epoch_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64
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

#[derive(Debug, Clone, Copy)]
pub enum Timespan {
    Year,
    Month,
    Week,
    Day,
}
pub fn parse_timespan(s: &str, _arg: ()) -> Result<Timespan, String> {
    match s {
        "y" => Ok(Timespan::Year),
        "m" => Ok(Timespan::Month),
        "w" => Ok(Timespan::Week),
        "d" => Ok(Timespan::Day),
        _ => Err("Valid values are y(ear), m(onth), w(eek), d(ay)".into()),
    }
}
/// Given a unix timestamp, truncate it to midnight and advance by the given number of years/months/days.
/// We avoid DST issues by switching to 12:00.
/// See https://github.com/chronotope/chrono/issues/290
pub fn timespan_next(ts: i64, add: Timespan) -> i64 {
    use chrono::{Datelike, Duration, Local, TimeZone, Timelike, Weekday};

    let mut d = Local.timestamp(ts, 0).with_minute(0).unwrap().with_second(0).unwrap();
    match add {
        Timespan::Year => {
            d = d.with_day(1).unwrap().with_month(1).unwrap().with_year(d.year() + 1).unwrap()
        },
        Timespan::Month => {
            d = d.with_day(1)
                 .unwrap()
                 .with_month0((d.month0() + 1) % 12)
                 .unwrap()
                 .with_year(if d.month() == 12 { d.year() + 1 } else { d.year() })
                 .unwrap()
        },
        Timespan::Week => {
            let till_monday = match d.weekday() {
                Weekday::Mon => 7,
                Weekday::Tue => 6,
                Weekday::Wed => 5,
                Weekday::Thu => 4,
                Weekday::Fri => 3,
                Weekday::Sat => 2,
                Weekday::Sun => 1,
            };
            d = d.with_hour(12).unwrap() + Duration::days(till_monday)
        },
        Timespan::Day => d = d.with_hour(12).unwrap() + Duration::days(1),
    }
    let res = d.with_hour(0).unwrap().timestamp();
    debug!("{} + {:?} = {}", fmt_utctime(ts), add, fmt_utctime(res));
    res
}
pub fn timespan_header(ts: i64, timespan: Timespan) -> String {
    use chrono::{Local, TimeZone};

    match timespan {
        Timespan::Year => format!("{}", Local.timestamp(ts, 0).format("%Y ")),
        Timespan::Month => format!("{}", Local.timestamp(ts, 0).format("%Y-%m ")),
        Timespan::Week => format!("{}", Local.timestamp(ts, 0).format("%Y-%W ")),
        Timespan::Day => format!("{}", Local.timestamp(ts, 0).format("%Y-%m-%d ")),
    }
}


#[cfg(test)]
mod test {
    use super::*;
    use std::convert::TryInto;
    use time::{format_description::well_known::Rfc3339, Weekday};

    fn parse_3339(s: &str) -> OffsetDateTime {
        OffsetDateTime::parse(s, &Rfc3339).unwrap()
    }
    fn parse_unix(epoch: i64) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(epoch).unwrap()
    }

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

    // FIXME: try different timezones, in particular Australia/Melbourne
    #[test]
    fn timespan_next_() {
        for t in &[// input                   year       month      week       day
                   "2019-01-01T00:00:00+00:00 2020-01-01 2019-02-01 2019-01-07 2019-01-02",
                   "2019-01-01T23:59:59+00:00 2020-01-01 2019-02-01 2019-01-07 2019-01-02",
                   "2019-01-30T00:00:00+00:00 2020-01-01 2019-02-01 2019-02-04 2019-01-31",
                   "2019-01-31T00:00:00+00:00 2020-01-01 2019-02-01 2019-02-04 2019-02-01",
                   "2019-12-31T00:00:00+00:00 2020-01-01 2020-01-01 2020-01-06 2020-01-01",
                   "2020-02-28T12:34:00+00:00 2021-01-01 2020-03-01 2020-03-02 2020-02-29"]
        {
            let v: Vec<&str> = t.split_whitespace().collect();
            let i = parse_3339(v[0]).unix_timestamp();
            let y = parse_3339(&format!("{}T00:00:00+00:00", v[1]));
            let m = parse_3339(&format!("{}T00:00:00+00:00", v[2]));
            let w = parse_3339(&format!("{}T00:00:00+00:00", v[3]));
            let d = parse_3339(&format!("{}T00:00:00+00:00", v[4]));
            assert_eq!(y, parse_unix(timespan_next(i, Timespan::Year)), "year {}", v[0]);
            assert_eq!(m, parse_unix(timespan_next(i, Timespan::Month)), "month {}", v[0]);
            assert_eq!(w, parse_unix(timespan_next(i, Timespan::Week)), "week {}", v[0]);
            assert_eq!(Weekday::Monday, parse_unix(timespan_next(i, Timespan::Week)).weekday());
            assert_eq!(d, parse_unix(timespan_next(i, Timespan::Day)), "day {}", v[0]);
        }
    }
}
