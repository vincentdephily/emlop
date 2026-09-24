use crate::{ArgError, ArgParse};

/// Type of msthematical average
#[derive(Clone, Copy)]
pub enum Average {
    /// Classic arithmetic "sum / count"
    Arith,
    /// Middle value (half of other values are above/below)
    Median,
    /// Arthmethic, weighted towards recent values (for N values, the weight goes linearly from 1 to N)
    WeightedArith,
    /// Median, weighted towards recent values (for N values, the weight goes linearly from 1 to N)
    WeightedMedian,
}
impl ArgParse<String, ()> for Average {
    fn parse(v: &String, _: (), s: &'static str) -> Result<Self, ArgError> {
        match v.as_str() {
            "a" | "arith" => Ok(Self::Arith),
            "m" | "median" => Ok(Self::Median),
            "wa" | "weighted-arith" => Ok(Self::WeightedArith),
            "wm" | "weighted-median" => Ok(Self::WeightedMedian),
            _ => Err(ArgError::new(v, s).pos("(a)rith (m)edian wa/weightedarith wm/weigtedmedian")),
        }
    }
}
impl Average {
    pub fn as_str(&self) -> &'static str {
        match &self {
            Self::Arith => "Arithetic mean",
            Self::Median => "Median",
            Self::WeightedArith => "Weighted arithmetic mean",
            Self::WeightedMedian => "Weighted median",
        }
    }
}

/// Container for durations (seconds) to predict the next value
///
/// Negative values are used for `count`, but ignored for `tot` and `pred()`.
#[derive(Default)]
pub struct Times {
    /// All positive values
    vals: Vec<i64>,
    /// Count of all values inserted, including negatives
    pub count: i64,
    /// Sum of all positive values inserted
    pub tot: i64,
}
impl Times {
    /// Digest new data point
    ///
    /// Data points should be inserted in chronological order.
    /// We don't store negative values but we still take them into account.
    pub fn insert(&mut self, t: i64) {
        self.count += 1;
        if t > 0 {
            self.vals.push(t);
            self.tot += t;
        }
    }
    /// Predict the next data point
    ///
    /// Only the last `lim` values are taken into account.
    pub fn pred(&self, lim: u16, avg: Average) -> Option<i64> {
        if self.vals.is_empty() {
            return None;
        }
        let start = self.vals.len().saturating_sub(lim as usize);
        let vals = &self.vals[start..];
        let pred = match avg {
            // Simple arithmetic mean
            Average::Arith => vals.iter().sum::<i64>() / vals.len() as i64,
            // Middle value (or avg of the middle two)
            Average::Median => {
                let mut s: Vec<i64> = vals.to_vec();
                s.sort_unstable();
                let l = vals.len();
                if l.is_multiple_of(2) { (s[(l / 2) - 1] + s[l / 2]) / 2 } else { s[l / 2] }
            },
            // Arithmically weighted arithmetic mean
            // Eg for 4 values the weights are 4,3,2,1 (most recent value first)
            Average::WeightedArith => {
                let (s, _, w) =
                    vals.iter().fold((0, 1, 0), |(s, n, w), v| (s + v * n, n + 1, w + n));
                s / w
            },
            // Arithmically weighted median
            // Eg 3,1,2 -> 3,1,1,2,2,2 -> 1,1,2,2,2,3 -> 2
            Average::WeightedMedian => {
                let mut s = Vec::with_capacity(((vals.len() + 1) * vals.len()) / 2);
                for (n, &v) in vals.iter().enumerate() {
                    s.resize(s.len() + n + 1, v);
                }
                s.sort_unstable();
                let l = s.len();
                if l.is_multiple_of(2) { (s[(l / 2) - 1] + s[l / 2]) / 2 } else { s[l / 2] }
            },
        };
        Some(pred)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn averages() {
        use Average::*;
        assert_eq!(None, Times::default().pred(10, Arith), "empty");
        for (a, m, wa, wm, lim, vals) in
            [(1, 1, 1, 1, 10, vec![1]),
             (12 / 2, 6, 21 / 3, 10, 10, vec![2, 10]),
             (12 / 2, 6, 14 / 3, 2, 10, vec![10, 2]),
             (15 / 3, 4, (1 + 20 + 12) / (1 + 2 + 3), 4, 10, vec![1, 10, 4]),
             (15 / 4, 2, (1 + 20 + 9 + 4) / (1 + 2 + 3 + 4), 2, 10, vec![1, 10, 3, 1]),
             (15 / 4, 2, (1 + 20 + 9 + 4) / (1 + 2 + 3 + 4), 2, 4, vec![999, 1, 10, 3, 1])]
        {
            let mut t = Times::default();
            for &v in vals.iter() {
                t.insert(v);
            }
            assert_eq!(Some(a), t.pred(lim, Arith), "arith {lim} {vals:?}");
            assert_eq!(Some(m), t.pred(lim, Median), "median {lim} {vals:?}");
            assert_eq!(Some(wa), t.pred(lim, WeightedArith), "weighted arith {lim} {vals:?}");
            assert_eq!(Some(wm), t.pred(lim, WeightedMedian), "weighted median {lim} {vals:?}");
        }
    }
}
