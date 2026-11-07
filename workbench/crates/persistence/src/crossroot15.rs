//! Pure, conservative checks across a legacy log and available legacy reports.
//!
//! Inputs must already have passed their respective schema and semantic validators.
//! This does not establish that the report inventory is complete, validate filesystem
//! paths or hashes, reconcile interrupted writes, or authorize migration.

use domain::store::{log::LogFile, report::ReportFile};

/// Counts of references for which an available counterpart was not supplied.
/// These are observations, not proof that files or records are missing on disk.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Unresolved {
    pub manifests_without_report: usize,
    pub reports_without_manifest: usize,
    pub manifests_without_run: usize,
    pub reports_without_run: usize,
    pub run_report_ids_without_manifest: usize,
}

/// No identifier or document content is included in errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossRootError {
    DuplicateRun,
    DuplicateManifest,
    DuplicateReport,
    ConflictingReportAssociation,
}

/// Compare associations only where both sides are supplied. Historic repository,
/// workflow and approval IDs are deliberately not resolved against current roots.
pub fn validate(
    log: Option<&LogFile>,
    reports: &[ReportFile],
) -> Result<Unresolved, CrossRootError> {
    let mut unresolved = Unresolved::default();
    for (index, report) in reports.iter().enumerate() {
        if reports[..index]
            .iter()
            .any(|other| other.report_id == report.report_id)
        {
            return Err(CrossRootError::DuplicateReport);
        }
    }
    let Some(log) = log else {
        unresolved.reports_without_manifest = reports.len();
        unresolved.reports_without_run = reports.len();
        return Ok(unresolved);
    };
    for (index, run) in log.runs.iter().enumerate() {
        if log.runs[..index]
            .iter()
            .any(|other| other.run_id == run.run_id)
        {
            return Err(CrossRootError::DuplicateRun);
        }
        for report_id in &run.report_ids {
            match log
                .reports
                .iter()
                .find(|manifest| manifest.report_id == *report_id)
            {
                Some(manifest) if manifest.run_id != run.run_id => {
                    return Err(CrossRootError::ConflictingReportAssociation);
                }
                Some(_) => {}
                None => unresolved.run_report_ids_without_manifest += 1,
            }
            if reports
                .iter()
                .any(|report| report.report_id == *report_id && report.run_id != run.run_id)
            {
                return Err(CrossRootError::ConflictingReportAssociation);
            }
        }
    }
    for (index, manifest) in log.reports.iter().enumerate() {
        if log.reports[..index]
            .iter()
            .any(|other| other.report_id == manifest.report_id)
        {
            return Err(CrossRootError::DuplicateManifest);
        }
        if !log.runs.iter().any(|run| run.run_id == manifest.run_id) {
            unresolved.manifests_without_run += 1;
        }
        match reports
            .iter()
            .find(|report| report.report_id == manifest.report_id)
        {
            Some(report)
                if report.run_id != manifest.run_id || report.repo_id != manifest.repo_id =>
            {
                return Err(CrossRootError::ConflictingReportAssociation);
            }
            Some(_) => {}
            None => unresolved.manifests_without_report += 1,
        }
    }
    for report in reports {
        if !log
            .reports
            .iter()
            .any(|manifest| manifest.report_id == report.report_id)
        {
            unresolved.reports_without_manifest += 1;
        }
        if !log.runs.iter().any(|run| run.run_id == report.run_id) {
            unresolved.reports_without_run += 1;
        }
    }
    Ok(unresolved)
}
