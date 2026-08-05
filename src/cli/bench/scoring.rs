// Clean-room mirror of upstream SkateBench isCorrect (docs/skatebench-compat.md
// §2): negatives are checked first and override positives; matching is
// lowercase substring over the full response text.
pub fn is_correct(output: &str, answers: &[String], negative_answers: &[String]) -> bool {
    let result_lower = output.to_lowercase();
    if negative_answers
        .iter()
        .any(|negative| result_lower.contains(&negative.to_lowercase()))
    {
        return false;
    }
    answers
        .iter()
        .any(|answer| result_lower.contains(&answer.to_lowercase()))
}
