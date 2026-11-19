//! The rules for trying again, the same as the client of the server: a connection that could not be made is tried again for any method (nothing was sent); a time-out
//! or a 502, 503 or 504 only for the methods that can be sent twice (never for a POST, which could do the thing twice).

use crate::backoff::Backoff;
use crate::error::LinkError;
use crate::transport::{HttpRequest, HttpResponse, HttpTransport};

/// What waits between tries. The tests give one that only counts.
pub trait Sleeper {
    fn sleep_ms(&mut self, ms: u64);
}

/// A sleeper that really waits.
#[derive(Debug, Default)]
pub struct ThreadSleeper;

impl Sleeper for ThreadSleeper {
    fn sleep_ms(&mut self, ms: u64) {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
}

/// A sleeper that adds up what it was asked to wait.
#[derive(Debug, Default)]
pub struct CountingSleeper {
    pub total_ms: u64,
    pub calls: u32,
}

impl Sleeper for CountingSleeper {
    fn sleep_ms(&mut self, ms: u64) {
        self.total_ms += ms;
        self.calls += 1;
    }
}

/// A transport that tries again.
pub struct Retrying<T: HttpTransport, S: Sleeper> {
    inner: T,
    sleeper: S,
    backoff: Backoff,
    retries: u32,
    /// How many tries the last `send` needed.
    pub last_attempts: u32,
}

impl<T: HttpTransport, S: Sleeper> Retrying<T, S> {
    /// `retries` more tries after the first (0 to 5).
    pub fn new(inner: T, sleeper: S, retries: u32, seed: u64) -> Retrying<T, S> {
        Retrying {
            inner,
            sleeper,
            backoff: Backoff::new(200, 2000, seed),
            retries: retries.min(5),
            last_attempts: 0,
        }
    }

    pub fn inner(&self) -> &T {
        &self.inner
    }

    pub fn sleeper(&self) -> &S {
        &self.sleeper
    }
}

impl<T: HttpTransport, S: Sleeper> HttpTransport for Retrying<T, S> {
    fn send(&mut self, req: &HttpRequest) -> Result<HttpResponse, LinkError> {
        let mut attempt = 0u32;
        loop {
            self.last_attempts = attempt + 1;
            let result = self.inner.send(req);
            let again = match &result {
                Err(LinkError::NoConnection) => true,
                Err(LinkError::Timeout) => req.method.is_idempotent(),
                Ok(r) if matches!(r.status, 502..=504) => req.method.is_idempotent(),
                _ => false,
            };
            if !again || attempt >= self.retries {
                return result;
            }
            let ms = self.backoff.delay_ms(attempt);
            self.sleeper.sleep_ms(ms);
            attempt += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::Method;
    use std::collections::VecDeque;

    struct Script(VecDeque<Result<HttpResponse, LinkError>>, u32);

    impl HttpTransport for Script {
        fn send(&mut self, _: &HttpRequest) -> Result<HttpResponse, LinkError> {
            self.1 += 1;
            self.0
                .pop_front()
                .unwrap_or_else(|| Ok(HttpResponse::json(200, "{}")))
        }
    }

    fn script(v: Vec<Result<HttpResponse, LinkError>>) -> Script {
        Script(v.into(), 0)
    }

    fn get() -> HttpRequest {
        HttpRequest::new(Method::Get, "/x")
    }

    fn post() -> HttpRequest {
        HttpRequest::new(Method::Post, "/x")
    }

    #[test]
    fn a_connection_that_failed_is_tried_again_for_any_method() {
        let mut r = Retrying::new(
            script(vec![
                Err(LinkError::NoConnection),
                Err(LinkError::NoConnection),
            ]),
            CountingSleeper::default(),
            2,
            1,
        );
        assert!(r.send(&post()).is_ok());
        assert_eq!(r.last_attempts, 3);
        assert_eq!(r.sleeper().calls, 2);
    }

    #[test]
    fn a_timeout_is_not_tried_again_for_a_post() {
        let mut r = Retrying::new(
            script(vec![Err(LinkError::Timeout)]),
            CountingSleeper::default(),
            3,
            1,
        );
        assert_eq!(r.send(&post()), Err(LinkError::Timeout));
        assert_eq!(r.last_attempts, 1);
        let mut g = Retrying::new(
            script(vec![Err(LinkError::Timeout)]),
            CountingSleeper::default(),
            3,
            1,
        );
        assert!(g.send(&get()).is_ok());
        assert_eq!(g.last_attempts, 2);
    }

    #[test]
    fn busy_servers_are_waited_for_only_for_safe_methods() {
        let busy = || Ok(HttpResponse::json(503, "busy"));
        let mut g = Retrying::new(
            script(vec![busy(), busy()]),
            CountingSleeper::default(),
            2,
            1,
        );
        assert_eq!(g.send(&get()).unwrap().status, 200);
        let mut p = Retrying::new(script(vec![busy()]), CountingSleeper::default(), 2, 1);
        assert_eq!(p.send(&post()).unwrap().status, 503);
        assert_eq!(p.inner().1, 1);
    }

    #[test]
    fn the_tries_run_out_and_the_pauses_are_bounded() {
        let many: Vec<_> = (0..20).map(|_| Err(LinkError::NoConnection)).collect();
        let mut r = Retrying::new(script(many), CountingSleeper::default(), 99, 5);
        assert_eq!(r.send(&get()), Err(LinkError::NoConnection));
        assert_eq!(r.last_attempts, 6);
        assert!(r.sleeper().total_ms <= 5 * 2000);
    }

    #[test]
    fn a_refusal_is_not_retried() {
        let mut r = Retrying::new(
            script(vec![Ok(HttpResponse::json(401, "no"))]),
            CountingSleeper::default(),
            3,
            1,
        );
        assert_eq!(r.send(&get()).unwrap().status, 401);
        assert_eq!(r.last_attempts, 1);
        assert!(LinkError::Timeout.is_transient() && !LinkError::Auth.is_transient());
    }
}
