use std::time::Duration;

use anyhow::Result;
use aws_sdk_s3::types::ObjectIdentifier;
use jiff::{Timestamp, ToSpan};

use crate::{
    db::sql::{UploadRecord, UploadStatus, upload::get_s3_key},
    state::app::AppState,
};

struct CleanRecordsResult {
    affected_rows: u32,
}

static MAX_DELETE_OBJECTS: usize = 500;

pub async fn cleaner_task_supervisor(mut state: AppState) {
    const CLEANUP_INTERVAL: Duration = Duration::from_mins(5);

    tracing::info!(
        interval_secs = CLEANUP_INTERVAL.as_secs(),
        "Expired upload cleanup scheduler started"
    );
    loop {
        if state.ct.is_cancelled() {
            break;
        }

        match clean_records(&mut state).await {
            Ok(r) => tracing::debug!(
                deleted_records = r.affected_rows,
                "Expired upload cleanup pass completed"
            ),
            Err(e) => {
                tracing::error!(error = %e, "Expired upload cleanup failed; retrying on next interval")
            }
        };

        tokio::select! {
            biased;
            _ = state.ct.cancelled() => break,
            _ = tokio::time::sleep(CLEANUP_INTERVAL) => {}
        }
    }
}

#[tracing::instrument(name = "uploads.cleanup", skip_all)]
async fn clean_records(state: &mut AppState) -> Result<CleanRecordsResult> {
    tracing::debug!("Scanning for expired pending uploads");

    let mut res = CleanRecordsResult { affected_rows: 0 };

    loop {
        // Finish each S3/SQL deletion batch before honoring shutdown.
        if state.ct.is_cancelled() {
            break;
        }

        let expired = UploadRecord::filter(
            UploadRecord::fields()
                .created_at()
                .le(Timestamp::now().checked_sub(1.hours())?),
        )
        .filter(UploadRecord::fields().status().eq(UploadStatus::Pending))
        .order_by(UploadRecord::fields().created_at().asc())
        .limit(MAX_DELETE_OBJECTS)
        .select((
            UploadRecord::fields().id(),
            UploadRecord::fields().mime_type(),
        ))
        .exec(&mut state.sql)
        .await?;

        let mut target_s3_identifiers: Vec<ObjectIdentifier> =
            Vec::with_capacity(expired.len() * 2);
        for (id, mime) in &expired {
            target_s3_identifiers.push(
                aws_sdk_s3::types::ObjectIdentifier::builder()
                    .key(get_s3_key(id, mime, &UploadStatus::Pending))
                    .build()?,
            );
        }

        if expired.is_empty() {
            break;
        }

        tracing::debug!(
            batch_size = expired.len(),
            "Deleting expired pending objects from storage"
        );
        let response = state
            .s3
            .delete_objects()
            .bucket(&state.config.s3_bucket)
            .delete(
                aws_sdk_s3::types::Delete::builder()
                    .set_objects(Some(target_s3_identifiers))
                    .build()?,
            )
            .send()
            .await?;

        let errors = response.errors();

        if !errors.is_empty() {
            let errors_strings = errors
                .iter()
                .map(|e| format!("{:?}", e))
                .collect::<Vec<String>>();
            anyhow::bail!(format!(
                "failed deleting all S3 objects\nerrors: {}",
                errors_strings.join("\n\n")
            ));
        }

        let ids: Vec<_> = expired.iter().map(|(id, _)| *id).collect();
        UploadRecord::filter(UploadRecord::fields().id().in_list(ids.as_slice()))
            .delete()
            .exec(&mut state.sql)
            .await?;

        res.affected_rows += expired.len() as u32;
        tracing::info!(
            deleted_records = expired.len(),
            "Expired pending objects and database records deleted"
        );

        if expired.len() < MAX_DELETE_OBJECTS {
            break;
        }
    }

    Ok(res)
}
