use zenoh_2825_repro::{parse_result, verdict, Outcome, Verdict};

fn outcome(frozen: bool, declared: u64, canary_delivered: bool) -> Outcome {
    Outcome {
        frozen,
        declared,
        canary_delivered,
        elapsed_secs: 1.0,
    }
}

#[test]
fn should_parse_a_frozen_result_line() {
    let line = "RESULT frozen=true declared=65536 canary_delivered=false elapsed_secs=296.4";

    let parsed = parse_result(line);

    assert_eq!(
        parsed,
        Some(Outcome {
            frozen: true,
            declared: 65536,
            canary_delivered: false,
            elapsed_secs: 296.4,
        })
    );
}

#[test]
fn should_parse_a_completed_result_line() {
    let line = "RESULT frozen=false declared=100000 canary_delivered=true elapsed_secs=0.4";

    let parsed = parse_result(line);

    assert_eq!(
        parsed,
        Some(Outcome {
            frozen: false,
            declared: 100_000,
            canary_delivered: true,
            elapsed_secs: 0.4,
        })
    );
}

#[test]
fn should_ignore_progress_lines() {
    let line = "churn n=  5000 batch=   20.8ms declare p50=     1.4us";

    let parsed = parse_result(line);

    assert_eq!(parsed, None);
}

#[test]
fn should_ignore_a_result_line_missing_a_field() {
    let line = "RESULT frozen=true declared=65536";

    let parsed = parse_result(line);

    assert_eq!(parsed, None);
}

#[test]
fn should_report_reproduced_and_fixed_when_before_freezes_and_after_completes() {
    let before = outcome(true, 65_536, false);
    let after = outcome(false, 100_000, true);

    let result = verdict(&before, &after, 100_000);

    assert_eq!(result, Verdict::ReproducedAndFixed);
}

#[test]
fn should_report_not_reproduced_when_before_completes() {
    let before = outcome(false, 50_000, true);
    let after = outcome(false, 50_000, true);

    let result = verdict(&before, &after, 50_000);

    assert_eq!(result, Verdict::NotReproduced);
}

#[test]
fn should_report_not_fixed_when_after_freezes() {
    let before = outcome(true, 65_536, false);
    let after = outcome(true, 65_536, false);

    let result = verdict(&before, &after, 100_000);

    assert_eq!(result, Verdict::NotFixed);
}

#[test]
fn should_report_not_fixed_when_after_loses_the_canary() {
    let before = outcome(true, 65_536, false);
    let after = outcome(false, 100_000, false);

    let result = verdict(&before, &after, 100_000);

    assert_eq!(result, Verdict::NotFixed);
}

#[test]
fn should_report_not_fixed_when_after_stops_short() {
    let before = outcome(true, 65_536, false);
    let after = outcome(false, 70_000, true);

    let result = verdict(&before, &after, 100_000);

    assert_eq!(result, Verdict::NotFixed);
}
