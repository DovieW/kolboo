use super::*;
use futures_util::{SinkExt, StreamExt};

#[tokio::test]
async fn overlapping_final_ranges_fail_the_actual_session() {
    let (write, read, mut peer) = crate::stt::streaming_test_support::peers().await;
    let (audio, input) = mpsc::channel(1);
    let (partials, _output) = mpsc::channel(8);
    let task = tokio::spawn(run_streaming_task(
        write, read, input, partials, 16000, None,
    ));
    audio.send(vec![0.1; 1600]).await.unwrap();
    assert!(matches!(
        peer.next().await.unwrap().unwrap(),
        Message::Binary(_)
    ));
    for start in [0.0, 0.05] {
        peer.send(Message::Text(json!({"type":"Results","start":start,"duration":0.1,"is_final":true,"channel":{"alternatives":[{"transcript":"synthetic"}]}}).to_string().into())).await.unwrap();
    }
    assert!(task
        .await
        .unwrap()
        .unwrap_err()
        .to_string()
        .contains("overlapping"));
}

#[tokio::test]
async fn streaming_delivers_all_audio_and_commits_final_ranges_only_once() {
    let (write, read, mut peer) = crate::stt::streaming_test_support::peers().await;
    let (audio, input) = mpsc::channel(4);
    let (partials, mut output) = mpsc::channel(32);
    let store = RequestLogStore::new();
    store.start_request("deepgram".into(), None);
    let task = tokio::spawn(run_streaming_task(
        write,
        read,
        input,
        partials,
        16000,
        Some(store.clone()),
    ));
    audio.send(vec![0.1; 1601]).await.unwrap();
    drop(audio);
    let mut received = Vec::new();
    loop {
        let message = peer.next().await.unwrap().unwrap();
        match message {
            Message::Binary(bytes) => received.extend_from_slice(&bytes),
            Message::Text(text)
                if serde_json::from_str::<JsonValue>(&text).unwrap()["type"] == "CloseStream" =>
            {
                break
            }
            _ => {}
        }
    }
    assert_eq!(received, f32_to_pcm_s16le(&vec![0.1; 1601]));
    for event in [
        json!({"type":"Metadata"}),
        json!({"type":"SpeechStarted"}),
        json!({"type":"UtteranceEnd"}),
        json!({"type":"future-event"}),
        json!({"type":"Results","start":0,"duration":0.1,"is_final":false,"channel":{"alternatives":[{"transcript":"draft"}]}}),
        json!({"type":"Results","start":0,"duration":0.1,"is_final":true,"channel":{"alternatives":[{"transcript":"hello"}]}}),
        json!({"type":"Results","start":0,"duration":0.1,"is_final":true,"channel":{"alternatives":[{"transcript":"hello"}]}}),
    ] {
        peer.send(Message::Text(event.to_string().into()))
            .await
            .unwrap();
    }
    peer.send(Message::Binary(vec![0].into())).await.unwrap();
    peer.close(None).await.unwrap();
    assert_eq!(task.await.unwrap().unwrap(), "hello");
    let mut committed = Vec::new();
    while let Some(partial) = output.recv().await {
        if let Some(text) = partial.committed_text {
            committed.push(text);
        }
    }
    assert_eq!(committed, ["hello"]);
    assert_eq!(
        store.get_logs(None)[0].raw_transcript.as_deref(),
        Some("hello")
    );
}

#[tokio::test]
async fn malformed_or_unfinished_streams_fail_without_exposing_provider_content() {
    for event in [
        "private-not-json".to_string(),
        json!({"type":"Results"}).to_string(),
        json!({"type":"Error","message":"private-content"}).to_string(),
    ] {
        let (write, read, mut peer) = crate::stt::streaming_test_support::peers().await;
        let (_audio, input) = mpsc::channel(1);
        let (partials, _output) = mpsc::channel(1);
        let task = tokio::spawn(run_streaming_task(
            write, read, input, partials, 16000, None,
        ));
        peer.send(Message::Text(event.into())).await.unwrap();
        let error = task.await.unwrap().unwrap_err().to_string();
        assert!(!error.contains("private"));
    }
    {
        let (write, read, mut peer) = crate::stt::streaming_test_support::peers().await;
        peer.close(None).await.unwrap();
        let (_audio, input) = mpsc::channel(1);
        let (partials, _output) = mpsc::channel(1);
        assert!(
            run_streaming_task(write, read, input, partials, 16000, None)
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn active_and_finalization_timeouts_cannot_accept_accumulated_prefixes() {
    for finalized in [false, true] {
        let (write, read, mut peer) = crate::stt::streaming_test_support::peers().await;
        let (audio, input) = mpsc::channel(1);
        let (partials, _output) = mpsc::channel(1);
        let task = tokio::spawn(run_streaming_task(
            write, read, input, partials, 16000, None,
        ));
        if finalized {
            drop(audio);
            loop {
                if let Message::Text(text) = peer.next().await.unwrap().unwrap() {
                    if serde_json::from_str::<JsonValue>(&text).unwrap()["type"] == "CloseStream" {
                        break;
                    }
                }
            }
        }
        tokio::time::pause();
        tokio::task::yield_now().await;
        tokio::time::advance(
            if finalized {
                POST_FINALIZE_TIMEOUT
            } else {
                DEFAULT_WS_TIMEOUT
            } + Duration::from_secs(1),
        )
        .await;
        assert!(matches!(task.await.unwrap(), Err(SttError::Timeout)));
        tokio::time::resume();
    }
}
