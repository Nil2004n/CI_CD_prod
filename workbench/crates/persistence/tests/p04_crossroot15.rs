use domain::ids::{RepoId, RunId};
use domain::store::{log::LogFile, report::ReportFile};
use persistence::crossroot15::{CrossRootError, validate};

fn fixtures() -> Result<(LogFile, ReportFile), serde_json::Error> {
    let log = serde_json::from_str(include_str!("fixtures/store/log.max.json"))?;
    let report = serde_json::from_str(include_str!("fixtures/store/report.max.json"))?;
    Ok((log, report))
}

#[test]
fn available_match_and_absent_report_are_distinguished() -> Result<(), Box<dyn std::error::Error>> {
    let (log, report) = fixtures()?;
    let unresolved = validate(Some(&log), &[report]).map_err(|e| format!("{e:?}"))?;
    assert_eq!(unresolved.manifests_without_report, 1);
    assert_eq!(unresolved.reports_without_manifest, 0);
    assert_eq!(unresolved.manifests_without_run, 0);
    Ok(())
}

#[test]
fn mismatched_run_or_repository_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let (log, report) = fixtures()?;
    let mut wrong_repo = report.clone();
    wrong_repo.repo_id = RepoId::parse("00000000-0000-4000-8000-000000000099")?;
    assert_eq!(
        validate(Some(&log), &[wrong_repo]),
        Err(CrossRootError::ConflictingReportAssociation)
    );
    let mut wrong_run = report;
    wrong_run.run_id = RunId::parse("00000000-0000-4000-8000-000000000099")?;
    assert_eq!(
        validate(Some(&log), &[wrong_run]),
        Err(CrossRootError::ConflictingReportAssociation)
    );
    Ok(())
}

#[test]
fn duplicate_identifiers_and_conflicting_run_listing_are_rejected()
-> Result<(), Box<dyn std::error::Error>> {
    let (mut log, report) = fixtures()?;
    assert_eq!(
        validate(Some(&log), &[report.clone(), report]),
        Err(CrossRootError::DuplicateReport)
    );
    log.runs[0].report_ids.push(log.reports[0].report_id);
    assert_eq!(
        validate(Some(&log), &[]),
        Err(CrossRootError::ConflictingReportAssociation)
    );
    log.runs[0].report_ids.clear();
    log.reports.push(log.reports[0].clone());
    assert_eq!(
        validate(Some(&log), &[]),
        Err(CrossRootError::DuplicateManifest)
    );
    log.reports.pop();
    log.runs.push(log.runs[0].clone());
    assert_eq!(validate(Some(&log), &[]), Err(CrossRootError::DuplicateRun));
    Ok(())
}

#[test]
fn historical_report_does_not_require_current_registry_or_log()
-> Result<(), Box<dyn std::error::Error>> {
    let (_, report) = fixtures()?;
    let unresolved = validate(None, &[report]).map_err(|e| format!("{e:?}"))?;
    assert_eq!(unresolved.reports_without_manifest, 1);
    assert_eq!(unresolved.reports_without_run, 1);
    Ok(())
}
