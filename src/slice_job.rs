//! Slicing in the background, without a window: the slice runs on its own thread so the window keeps answering, and the window asks from time to time how it is going. The slicer
//! does not report its steps, so the bar is an estimate from the time the last slice took; it never says it is done before the slice is, and it is full only when the slice is over.

use crate::preview::PreviewState;
use p3dp_core::Mesh;
use p3dp_profiles::filament::{FilamentProfile, Nozzle};
use p3dp_profiles::PrinterProfile;
use p3dp_slicer::Placement;
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::time::Instant;

/// The time a slice is expected to take when there is no slice before it to go by, in seconds.
pub const FIRST_GUESS_S: f64 = 4.0;
/// The most the bar shows while the slice is still running.
pub const RUNNING_CAP: f32 = 0.95;

/// How much of the bar to fill after `elapsed_s` seconds of a slice that is expected to take `expected_s`: it rises fast at first and slows down, and stays under
/// [`RUNNING_CAP`].
pub fn estimate_progress(elapsed_s: f64, expected_s: f64) -> f32 {
    if !elapsed_s.is_finite() || elapsed_s <= 0.0 {
        return 0.0;
    }
    let expected = if expected_s.is_finite() && expected_s > 0.05 {
        expected_s
    } else {
        FIRST_GUESS_S
    };
    (f64::from(RUNNING_CAP) * (1.0 - (-elapsed_s / expected).exp())) as f32
}

/// What a finished slice gives back: the state of the preview with the new plan (or with the reason it failed) and how long it took.
#[derive(Debug, Clone)]
pub struct Done {
    pub preview: PreviewState,
    pub ok: bool,
    pub seconds: f64,
}

/// Where a slice stands.
#[derive(Debug)]
pub enum Poll {
    /// Still running: the share of the bar and the seconds so far.
    Running {
        fraction: f32,
        elapsed_s: f64,
    },
    Finished(Box<Done>),
    /// The thread ended without an answer (it panicked); the old plan is still there.
    Lost,
}

/// A slice running on its own thread.
pub struct SliceJob {
    rx: Receiver<Done>,
    started: Instant,
    expected_s: f64,
}

impl SliceJob {
    /// Starts slicing a mesh that is already on the bed (the places of the parts are the bed's). The preview that is given is copied: the one in the window is not touched until
    /// the answer is taken.
    pub fn spawn(
        preview: &PreviewState,
        mesh: Mesh,
        printer: PrinterProfile,
        filament: Option<(FilamentProfile, Nozzle)>,
        expected_s: f64,
    ) -> SliceJob {
        let (tx, rx) = channel();
        let mut state = preview.clone();
        state.config.placement = Placement::Origin;
        std::thread::spawn(move || {
            let t = Instant::now();
            let ok = state.slice(&mesh, &printer, filament.as_ref().map(|(f, n)| (f, *n)));
            let _ = tx.send(Done {
                preview: state,
                ok,
                seconds: t.elapsed().as_secs_f64(),
            });
        });
        SliceJob {
            rx,
            started: Instant::now(),
            expected_s,
        }
    }

    pub fn poll(&self) -> Poll {
        match self.rx.try_recv() {
            Ok(done) => Poll::Finished(Box::new(done)),
            Err(TryRecvError::Empty) => {
                let elapsed_s = self.started.elapsed().as_secs_f64();
                Poll::Running {
                    fraction: estimate_progress(elapsed_s, self.expected_s),
                    elapsed_s,
                }
            }
            Err(TryRecvError::Disconnected) => Poll::Lost,
        }
    }

    /// Waits for the answer (for a caller with no window, as the picture mode).
    pub fn wait(self) -> Poll {
        match self.rx.recv() {
            Ok(done) => Poll::Finished(Box::new(done)),
            Err(_) => Poll::Lost,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p3dp_core::mesh::cube;
    use p3dp_profiles::Catalog;

    fn printer() -> PrinterProfile {
        Catalog::builtin()
            .unwrap()
            .find("prusa-mk4s")
            .unwrap()
            .clone()
    }

    #[test]
    fn the_bar_rises_slows_down_and_never_reaches_full_while_running() {
        assert_eq!(estimate_progress(0.0, 3.0), 0.0);
        assert_eq!(estimate_progress(-1.0, 3.0), 0.0);
        assert_eq!(estimate_progress(f64::NAN, 3.0), 0.0);
        let mut last = 0.0;
        for i in 1..200 {
            let f = estimate_progress(f64::from(i) * 0.5, 3.0);
            assert!(f >= last && f < 1.0 && f <= RUNNING_CAP, "{i}: {f}");
            last = f;
        }
        assert!(last > 0.94, "{last}");
        // a nonsense expectation falls back to the first guess
        assert_eq!(
            estimate_progress(2.0, f64::NAN),
            estimate_progress(2.0, FIRST_GUESS_S)
        );
        assert_eq!(
            estimate_progress(2.0, 0.0),
            estimate_progress(2.0, FIRST_GUESS_S)
        );
        // a longer slice fills slower
        assert!(estimate_progress(2.0, 10.0) < estimate_progress(2.0, 2.0));
    }

    #[test]
    fn a_slice_in_the_background_comes_back_with_the_plan_and_leaves_the_window_state_alone() {
        let preview = PreviewState::default();
        let job = SliceJob::spawn(&preview, cube(20.0), printer(), None, 1.0);
        let Poll::Finished(done) = job.wait() else {
            panic!("no answer");
        };
        assert!(done.ok, "{:?}", done.preview.error);
        assert!(done.preview.layer_count() > 10);
        assert!(done.seconds >= 0.0);
        assert!(
            preview.plan.is_none(),
            "the copy was sliced, not the original"
        );
    }

    #[test]
    fn a_slice_that_fails_says_why_and_a_poll_sees_it_running_then_finished() {
        let preview = PreviewState::default();
        let job = SliceJob::spawn(&preview, Mesh::new(), printer(), None, 1.0);
        let mut result = None;
        for _ in 0..2000 {
            match job.poll() {
                Poll::Running { fraction, .. } => {
                    assert!((0.0..=RUNNING_CAP).contains(&fraction));
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                other => {
                    result = Some(other);
                    break;
                }
            }
        }
        let Some(Poll::Finished(done)) = result else {
            panic!("it did not finish: {result:?}");
        };
        assert!(!done.ok);
        assert!(done.preview.error.is_some());
        assert!(matches!(job.poll(), Poll::Lost), "the answer is taken once");
    }
}
