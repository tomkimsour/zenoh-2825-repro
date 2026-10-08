//! Parsing and judging the results of the zenoh#2825 scenario runs.

/// What one scenario run reported on its `RESULT` line.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    pub frozen: bool,
    pub declared: u64,
    pub canary_delivered: bool,
    pub elapsed_secs: f64,
}

/// The comparison of a run on unfixed zenoh with a run on fixed zenoh.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The unfixed run froze or lost data, the fixed run completed and routed data.
    ReproducedAndFixed,
    /// The unfixed run completed normally, so the bug did not show.
    NotReproduced,
    /// The bug showed, but the fixed run did not complete and route data.
    NotFixed,
}

/// Parses a line such as
/// `RESULT frozen=true declared=65536 canary_delivered=false elapsed_secs=296.4`.
pub fn parse_result(line: &str) -> Option<Outcome> {
    let fields = line.strip_prefix("RESULT ")?;
    let value = |key: &str| {
        fields
            .split_whitespace()
            .find_map(|field| field.strip_prefix(key)?.strip_prefix('='))
    };
    Some(Outcome {
        frozen: value("frozen")?.parse().ok()?,
        declared: value("declared")?.parse().ok()?,
        canary_delivered: value("canary_delivered")?.parse().ok()?,
        elapsed_secs: value("elapsed_secs")?.parse().ok()?,
    })
}

/// Judges whether `before` shows the bug and `after`, which was asked to declare
/// `expected` tokens, shows it fixed.
pub fn verdict(before: &Outcome, after: &Outcome, expected: u64) -> Verdict {
    let reproduced = before.frozen || !before.canary_delivered;
    let fixed = !after.frozen && after.canary_delivered && after.declared == expected;
    match (reproduced, fixed) {
        (false, _) => Verdict::NotReproduced,
        (true, false) => Verdict::NotFixed,
        (true, true) => Verdict::ReproducedAndFixed,
    }
}
