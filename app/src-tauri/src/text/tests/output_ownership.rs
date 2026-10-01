use super::*;

#[test]
fn delayed_approval_cannot_insert_into_new_or_unknown_session() {
    assert_eq!(validate_output_epoch(Some(7), Some(7)), Ok(()));
    for (expected, current) in [
        (Some(7), Some(8)),
        (Some(7), None),
        (None, Some(7)),
        (None, None),
    ] {
        assert!(validate_output_epoch(expected, current).is_err());
    }
}

#[test]
fn output_lease_tracks_real_pipeline_ownership_without_starting_capture() {
    let pipeline = crate::pipeline::SharedPipeline::new(crate::pipeline::PipelineConfig::default());
    let lease = OutputLease {
        epoch: pipeline.session_epoch(),
        pipeline: Some(pipeline.clone()),
    };
    assert_eq!(lease.ensure_current(), Ok(()));
    let recovery = pipeline.begin_recovery().unwrap();
    assert!(!recovery.is_cancelled());
    assert!(lease.ensure_current().is_err());
    let current = OutputLease {
        epoch: pipeline.session_epoch(),
        pipeline: Some(pipeline.clone()),
    };
    assert_eq!(current.ensure_current(), Ok(()));
    let unknown = OutputLease {
        epoch: None,
        pipeline: Some(pipeline),
    };
    assert!(unknown.ensure_current().is_err());
    // Explicit output commands may run before the pipeline is managed. They
    // don't claim a recording session and still use the input controller.
    assert_eq!(
        OutputLease {
            epoch: None,
            pipeline: None
        }
        .ensure_current(),
        Ok(())
    );
}
