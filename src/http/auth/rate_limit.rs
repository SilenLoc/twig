use std::{
    collections::{HashMap, VecDeque},
    net::{IpAddr, Ipv4Addr},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use actix_web::HttpRequest;

const RATE_WINDOW: Duration = Duration::from_mins(1);
const MAX_LOGIN_ATTEMPTS_PER_WINDOW: usize = 10;
const MAX_INVITE_ATTEMPTS_PER_WINDOW: usize = 10;
const MAX_TRACKED_PEERS: usize = 4096;

#[derive(Clone, Default)]
pub struct AuthRateLimiter {
    login: Arc<Mutex<HashMap<IpAddr, VecDeque<Instant>>>>,
    invite: Arc<Mutex<HashMap<IpAddr, VecDeque<Instant>>>>,
}

impl AuthRateLimiter {
    pub fn allow_login(&self, peer: IpAddr, now: Instant) -> bool {
        allow_request(&self.login, peer, now, MAX_LOGIN_ATTEMPTS_PER_WINDOW)
    }

    pub fn allow_invite(&self, peer: IpAddr, now: Instant) -> bool {
        allow_request(&self.invite, peer, now, MAX_INVITE_ATTEMPTS_PER_WINDOW)
    }

    pub fn peer_ip(request: &HttpRequest) -> IpAddr {
        request
            .peer_addr()
            .map_or(IpAddr::V4(Ipv4Addr::LOCALHOST), |address| address.ip())
    }
}

fn allow_request(
    buckets: &Mutex<HashMap<IpAddr, VecDeque<Instant>>>,
    peer: IpAddr,
    now: Instant,
    limit: usize,
) -> bool {
    let Ok(mut buckets) = buckets.lock() else {
        return false;
    };
    buckets.retain(|_, events| {
        prune_events(events, now);
        !events.is_empty()
    });
    if !buckets.contains_key(&peer) && buckets.len() >= MAX_TRACKED_PEERS {
        return false;
    }
    let events = buckets.entry(peer).or_default();
    prune_events(events, now);
    if events.len() >= limit {
        return false;
    }
    events.push_back(now);
    true
}

fn prune_events(events: &mut VecDeque<Instant>, now: Instant) {
    while events
        .front()
        .is_some_and(|event| now.saturating_duration_since(*event) >= RATE_WINDOW)
    {
        events.pop_front();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_and_invite_attempts_are_limited_independently_per_peer() {
        let limiter = AuthRateLimiter::default();
        let peer = IpAddr::V4(Ipv4Addr::LOCALHOST);
        let now = Instant::now();

        for _ in 0..MAX_LOGIN_ATTEMPTS_PER_WINDOW {
            assert!(limiter.allow_login(peer, now));
        }
        assert!(!limiter.allow_login(peer, now));
        assert!(limiter.allow_invite(peer, now));

        assert!(limiter.allow_login(peer, now + RATE_WINDOW));
        for _ in 1..MAX_INVITE_ATTEMPTS_PER_WINDOW {
            assert!(limiter.allow_invite(peer, now));
        }
        assert!(!limiter.allow_invite(peer, now));
    }
}
