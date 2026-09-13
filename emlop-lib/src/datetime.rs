use crate::types::{ArgError, ArgParse};
use anyhow::{Error, bail, ensure};
use regex::Regex;
use std::str::FromStr;
use time::{Date, Duration, OffsetDateTime, UtcOffset, format_description::FormatItem,
           macros::format_description, parsing::Parsed};

#[cfg_attr(test, derive(PartialEq, Debug))]
#[derive(Clone, Copy)]
/// Represents a position to read from/to in `emerge.log`
///
/// See [get_hist()](fn.get_hist.html) and the [ArgParse] impl.
pub enum HistBound {
    /// Unbounded
    None,
    /// Bound by unix timestamp
    Unix(i64),
    /// Bound by time of nth first/last emerge run
    Run(usize),
}

/// Parse HistBound from various formats
///
/// * A plain unix timestamp
/// * An absolute date (rfc3339-like, interpreted with the given [UtcOffset], see `parse_date_yyyymmdd()`)
/// * A relative date (see `parse_date_ago()`)
/// * An emerge command number (see `parse_command_num()`)
impl ArgParse<String, UtcOffset> for HistBound {
    fn parse(val: &String, offset: UtcOffset, src: &'static str) -> Result<Self, ArgError> {
        let s = val.trim();
        let et = match i64::from_str(s) {
            Ok(i) => return Ok(Self::Unix(i)),
            Err(e) => e,
        };
        let ea = match parse_date_yyyymmdd(s, offset) {
            Ok(i) => return Ok(Self::Unix(i)),
            Err(e) => e,
        };
        let ec = match parse_command_num(s) {
            Ok(i) => return Ok(Self::Run(i)),
            Err(e) => e,
        };
        match parse_date_ago(s, OffsetDateTime::now_utc()) {
            Ok(i) => Ok(Self::Unix(i)),
            Err(er) => {
                let m = format!("Not a unix timestamp ({et}), absolute date ({ea}), relative date ({er}), or command ({ec})");
                Err(ArgError::new(val, src).msg(m))
            },
        }
    }
}

/// Parse a command index (parse as 1-based, return as 0-based)
fn parse_command_num(s: &str) -> Result<usize, Error> {
    use atoi::FromRadix10;
    let (num, pos) = usize::from_radix_10(s.as_bytes());
    match s[pos..].trim() {
        "c" | "command" | "commands" if num > 0 => Ok(num - 1),
        "c" | "command" if pos == 0 => Ok(0),
        _ => bail!("bad span {:?}", &s[pos..]),
    }
}

/// Parse a number of day/years/hours/etc in the past, relative to current time
fn parse_date_ago(s: &str, mut now: OffsetDateTime) -> Result<i64, Error> {
    ensure!(s.chars().all(|c| c.is_ascii_alphanumeric() || c == ' ' || c == ','), "bad char");
    ensure!(s.chars().any(|c| c.is_ascii_alphabetic()), "empty");

    // Handle case where only a span is given
    if let Ok(now) = parse_date_span(1, s.trim(), now) {
        return Ok(now.unix_timestamp());
    }

    // The regex gives us a list of positive integers and strings. We expect to always have a
    // number, followed by a known string.
    let re = Regex::new("([0-9]+|[a-z]+)").expect("Bad date span regex");
    let mut tokens = re.find_iter(s);
    while let Some(t) = tokens.next() {
        let num: i32 = t.as_str().parse().map_err(|_| Error::msg("not a number"))?;
        now = parse_date_span(num, tokens.next().map(|m| m.as_str()).unwrap_or(""), now)?;
    }
    Ok(now.unix_timestamp())
}

fn parse_date_span(num: i32, span: &str, now: OffsetDateTime) -> Result<OffsetDateTime, Error> {
    Ok(match span {
        "y" | "year" | "years" => {
            let year = now.year() - num;
            let month = now.month();
            let d = Date::from_calendar_date(year, month, now.day().min(month.length(year)))?;
            now.replace_date(d)
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
            let d = Date::from_calendar_date(year, month, now.day().min(month.length(year)))?;
            now.replace_date(d)
        },
        "w" | "week" | "weeks" => now - num * Duration::WEEK,
        "d" | "day" | "days" => now - num * Duration::DAY,
        "h" | "hour" | "hours" => now - num * Duration::HOUR,
        "min" | "mins" | "minute" | "minutes" => now - num * Duration::MINUTE,
        "s" | "sec" | "secs" | "second" | "seconds" => now - num * Duration::SECOND,
        o => bail!("bad span {:?}", o),
    })
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
                             .with_offset_minute_signed(offset.minutes_past_hour())
                             .unwrap()
                             .with_offset_second_signed(offset.seconds_past_minute())
                             .unwrap();
    const FMT: &[FormatItem<'_>] = format_description!(
                                                       version = 2,
                                                       "[year]-[month]-[day]\
        [optional [[first [T][ ]][hour]:[minute][optional [:[second]]]]]\
        [optional [[first [Z][[offset_hour]:[offset_minute]]]]]"
    );
    let rest = p.parse_items(s.as_bytes(), FMT)?;
    ensure!(rest.is_empty(), "junk at end");
    Ok(OffsetDateTime::try_from(p)?.unix_timestamp())
}

/// Unix timestamp (i64) wrapper that `Display`s as `YYYY-MM-DDTHH:MM:SSZ`
pub struct FmtUtc(pub i64);
impl std::fmt::Display for FmtUtc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match time::Timestamp::from_seconds(self.0) {
            Ok(t) => write!(f, "{}T{:02}:{:02}:{:02}Z", t.date(), t.hour(), t.minute(), t.second()),
            Err(_) => write!(f, "#{}#", self.0),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use time::format_description::well_known::Rfc3339;

    fn parse_rfc(s: &str) -> OffsetDateTime {
        OffsetDateTime::parse(s, &Rfc3339).expect(s)
    }
    fn parse_fromto(s: &str, o: UtcOffset) -> Result<HistBound, ArgError> {
        HistBound::parse(&String::from(s), o, "")
    }
    fn parse_ago(ago: &str, now: &str) -> String {
        parse_date_ago(ago, parse_rfc(now)).map(|i| format!("{}", FmtUtc(i))).unwrap()
    }
    fn ts(t: OffsetDateTime) -> i64 {
        t.unix_timestamp()
    }

    #[test]
    fn date() {
        let tb_unix = |rfc| HistBound::Unix(ts(parse_rfc(rfc)));
        let utc = UtcOffset::UTC;

        // Absolute dates
        assert_eq!(Ok(tb_unix("2018-04-03T00:00:00Z")), parse_fromto(" 1522713600 ", utc));
        assert_eq!(Ok(tb_unix("2018-04-03T00:00:00Z")), parse_fromto(" 2018-04-03 ", utc));
        assert_eq!(Ok(tb_unix("2018-04-03T01:02:00Z")), parse_fromto("2018-04-03 01:02", utc));
        assert_eq!(Ok(tb_unix("2018-04-03T01:02:03Z")), parse_fromto("2018-04-03 01:02:03", utc));
        assert_eq!(Ok(tb_unix("2018-04-03T01:02:03Z")), parse_fromto("2018-04-03T01:02:03", utc));
        assert_eq!(Ok(tb_unix("2018-04-03T01:02:03Z")), parse_fromto("2018-04-03T01:02:03Z", utc));
        assert_eq!(Ok(tb_unix("2018-04-03T00:02:03Z")),
                   parse_fromto("2018-04-03T01:02:03+01:00", utc));

        // Different timezone (not calling `get_utcoffset()` because tests are threaded, which makes
        // `UtcOffset::current_local_offset()` error out)
        for secs in [3600, -7200, 5400, -5400, 42, -42] {
            let offset = UtcOffset::from_whole_seconds(secs.try_into().unwrap()).unwrap();
            let tb = HistBound::Unix(ts(parse_rfc("2018-04-03T00:00:00Z")) - secs);
            assert_eq!(Ok(tb), parse_fromto("2018-04-03T00:00", offset));
        }

        // Relative dates
        assert_eq!("2025-05-03T11:57:56Z",
                   parse_ago("1 hour, 3 days  45sec", "2025-05-06T12:58:41Z"));
        assert_eq!("2025-04-01T12:58:41Z", parse_ago("5 weeks", "2025-05-06T12:58:41Z"));
        assert_eq!("2025-05-04T11:58:41Z", parse_ago("2d1h", "2025-05-06T12:58:41Z"));
        assert_eq!("2025-04-29T12:58:41Z", parse_ago("w", "2025-05-06T12:58:41Z"));
        assert_eq!("2025-02-28T01:02:03Z", parse_ago("2m", "2025-04-29T01:02:03Z"));
        assert_eq!("2024-02-29T01:02:03Z", parse_ago("2m", "2024-04-29T01:02:03Z"));
        assert_eq!("2023-02-28T01:02:03Z", parse_ago("y", "2024-02-29T01:02:03Z"));

        // Failure cases
        assert!(parse_fromto("", utc).is_err());
        assert!(parse_fromto(" ", utc).is_err());
        assert!(parse_fromto(",", utc).is_err());
        assert!(parse_fromto("junk2018-04-03T01:01:01", utc).is_err());
        assert!(parse_fromto("2018-04-03T01:01:01junk", utc).is_err());
        assert!(parse_fromto("2018-02-29T01:01:01", utc).is_err());
        assert!(parse_fromto("152271000o", utc).is_err());
        assert!(parse_fromto("1 day 3 centuries", utc).is_err());
        assert!(parse_fromto("a while ago", utc).is_err());
    }

    #[test]
    fn command_num() {
        assert_eq!(parse_command_num("1c").unwrap(), 0);
        assert_eq!(parse_command_num("5c").unwrap(), 4);
        assert_eq!(parse_command_num("c").unwrap(), 0);
        assert_eq!(parse_command_num("1 commands ").unwrap(), 0);
        assert!(parse_command_num("").is_err());
        assert!(parse_command_num("0c").is_err());
        assert!(parse_command_num("0").is_err());
        assert!(parse_command_num("1cool").is_err());
        assert!(parse_command_num("-1c").is_err());
    }
}
