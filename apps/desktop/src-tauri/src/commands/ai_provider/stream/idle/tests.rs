//! Idle-timeout behaviour with PAUSED tokio time (#1353): a stream that keeps
//! producing outlives the old wall deadline, a silent one fails at the idle
//! bound, and the absolute ceiling still fires on a trickle.

use std::collections::VecDeque;
use std::time::Duration;

use super::*;

const IDLE: Duration = Duration::from_secs(300);

fn limits() -> StreamLimits {
    StreamLimits::new(IDLE)
}

/// One scripted step: wait `after`, then yield the result.
type Step = (Duration, Result<Option<Vec<u8>>, SourceError>);

struct Script(VecDeque<Step>);

#[async_trait]
impl ChunkSource for Script {
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, SourceError> {
        match self.0.pop_front() {
            Some((after, res)) => {
                tokio::time::sleep(after).await;
                res
            }
            // Past the script the source goes permanently silent.
            None => std::future::pending().await,
        }
    }
}

fn chunk(after: u64, text: &str) -> Step {
    (
        Duration::from_secs(after),
        Ok(Some(text.as_bytes().to_vec())),
    )
}

/// One text piece per newline-terminated line.
fn lines(buf: &mut String) -> Vec<StreamPiece> {
    let mut out = Vec::new();
    while let Some(i) = buf.find('\n') {
        let line: String = buf.drain(..=i).collect();
        let line = line.trim_end();
        out.push(if line == "END" {
            StreamPiece::done("")
        } else {
            StreamPiece::text(line)
        });
    }
    out
}

#[tokio::test(start_paused = true)]
async fn a_stream_that_keeps_producing_outlives_the_old_wall_deadline() {
    // 5 chunks 200 s apart = 1000 s total: past the 300 s wall that used to kill
    // it, never silent for IDLE, and under the 1200 s ceiling.
    let steps = (0..5).map(|i| chunk(200, &format!("c{i}\n"))).chain([chunk(
        1, "END
",
    )]);
    let mut src = Script(steps.collect());
    let (text, _) = collect(&mut src, lines, limits(), "Test", false)
        .await
        .unwrap();
    assert_eq!(text, "c0c1c2c3c4");
}

#[tokio::test(start_paused = true)]
async fn a_stream_silent_for_the_idle_bound_fails_with_a_timeout() {
    let mut src = Script([chunk(10, "a\n"), chunk(IDLE.as_secs() + 1, "late\n")].into());
    let err = collect(&mut src, lines, limits(), "Test", false)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, AppError::Timeout(m) if m.contains("no data received for 300s")),
        "got {err:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn the_absolute_ceiling_fires_on_a_stream_that_never_goes_idle() {
    // A chunk every 70 s forever (70 does not divide 1200, so no timer tie):
    // never idle, but must stop at the ceiling.
    let steps: VecDeque<Step> = (0..100).map(|i| chunk(70, &format!("c{i}\n"))).collect();
    let mut src = Script(steps);
    let err = collect(&mut src, lines, limits(), "Test", false)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, AppError::Timeout(m) if m.contains("limit") && m.contains("1200s")),
        "got {err:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn thinking_deltas_are_not_part_of_the_answer_and_count_as_activity() {
    fn mixed(buf: &mut String) -> Vec<StreamPiece> {
        let mut out = Vec::new();
        while let Some(i) = buf.find('\n') {
            let line: String = buf.drain(..=i).collect();
            let line = line.trim_end();
            out.push(match line.strip_prefix("T:") {
                Some(t) => StreamPiece::thinking(t),
                None if line == "END" => StreamPiece::done(""),
                None => StreamPiece::text(line),
            });
        }
        out
    }
    // 4 x 250 s of pure thinking (1000 s > the 300 s wall), then the answer.
    let steps = (0..4)
        .map(|_| chunk(250, "T:hm\n"))
        .chain([chunk(1, "{}\nEND\n")]);
    let mut src = Script(steps.collect());
    let (text, _) = collect(&mut src, mixed, limits(), "Test", false)
        .await
        .unwrap();
    assert_eq!(text, "{}");
}

#[tokio::test(start_paused = true)]
async fn a_reqwest_timeout_mid_body_is_reported_as_the_ceiling() {
    let mut src = Script([(Duration::ZERO, Err(SourceError::Timeout))].into());
    let err = collect(&mut src, lines, limits(), "Test", false)
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::Timeout(_)), "got {err:?}");
}

#[tokio::test(start_paused = true)]
async fn a_transport_error_stays_a_network_error() {
    let mut src = Script([(Duration::ZERO, Err(SourceError::Other("boom".into())))].into());
    let err = collect(&mut src, lines, limits(), "Test", false)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, AppError::Network(m) if m.contains("boom")),
        "got {err:?}"
    );
}

#[test]
fn the_ceiling_is_the_idle_bound_times_the_shared_factor() {
    assert_eq!(limits().ceiling, IDLE * 4);
}
