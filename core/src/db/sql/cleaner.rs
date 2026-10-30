use std::time::Duration;

use anyhow::Result;
use aws_sdk_s3::types::ObjectIdentifier;
use jiff::{Timestamp, ToSpan};

use crate::{db::sql::{UploadRecord, UploadStatus}, state::app::AppState};

struct CleanRecordsResult {
    affected_rows: u32
}

static MAX_DELETE_OBJECTS: usize = 500;

pub async fn cleaner_task_supervisor(mut state: AppState) {
    loop {
        match clean_records(&mut state).await {
            Ok(r) => tracing::info!("cleaned up {} expired records", r.affected_rows),
            Err(e) => tracing::error!("failed running cleanup: {}", e)
        };

        tokio::time::sleep(Duration::try_from(jiff::Span::new().minutes(15)).unwrap()).await;
    }
}

async fn clean_records(state: &mut AppState) -> Result<CleanRecordsResult> {
    tracing::info!("running auto-cleanup task");

    let mut res = CleanRecordsResult {
        affected_rows: 0
    };

    loop {
        let expired = UploadRecord::filter(
            UploadRecord::fields().created_at().le(Timestamp::now().checked_sub(1.hours())?)
        )
            .filter(UploadRecord::fields().status().eq(UploadStatus::Pending))
            .order_by(UploadRecord::fields().created_at().asc())
            .limit(MAX_DELETE_OBJECTS)
            .select(UploadRecord::fields().id())
            .exec(&mut state.sql)
            .await?;

        let mut target_s3_identifiers: Vec<ObjectIdentifier> = Vec::with_capacity(expired.len() * 2);
        for id in &expired {
            target_s3_identifiers.push(ObjectIdentifier::builder().key(format!("uploads/{}", id)).build()?);
            target_s3_identifiers.push(ObjectIdentifier::builder().key(format!("pending/{}", id)).build()?);
        }

        // TODO: extract this into an unit test?
        assert_eq!(target_s3_identifiers.len(), expired.len() * 2);

        if expired.len() <= 0 { break; }
        
        let response = state.s3
            .delete_objects()
            .bucket(&state.config.s3_bucket)
            .delete(
                aws_sdk_s3::types::Delete::builder()
                    .set_objects(Some(target_s3_identifiers))
                    .build()?
            )
            .send()
            .await?;

        let errors = response.errors();
        
        if errors.len() > 0 {
            let errors_strings = errors.iter().map(|e| format!("{:?}", e)).collect::<Vec<String>>();
            anyhow::bail!(format!("failed deleting all S3 objects\nerrors: {}", errors_strings.join("\n\n")));
        }

        UploadRecord::filter(UploadRecord::fields().id().in_list(expired.as_slice())).delete().exec(&mut state.sql).await?;
        res.affected_rows += expired.len() as u32;

        if expired.len() < MAX_DELETE_OBJECTS { break; }
    }

    Ok(res)
}