//! What the application has to do after the room changed.

use super::{LinkEvent, MediaPath, Pair, SessionId, Track, TrackId};

/// Tell `to` to play `peer`'s media over `path`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteNotice {
    pub to: SessionId,
    pub peer: SessionId,
    pub path: MediaPath,
    pub epoch: u64,
}

/// Tell `to` to close its direct connection to `peer`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkClose {
    pub to: SessionId,
    pub peer: SessionId,
}

/// A `ToDirect` timeout to arm: when it fires, the application reports it with the
/// same pair and epoch (see `Room::link_timeout`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkTimeout {
    pub pair: Pair,
    pub epoch: u64,
}

/// Everything a change to a room asks of the application, in one value: the
/// subscription changes (the application renegotiates each affected session once),
/// the messages for clients and the timeouts to arm.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Effects {
    /// `(subscriber, tracks it starts receiving through the SFU)`.
    pub subscribe: Vec<(SessionId, Vec<Track>)>,
    /// `(subscriber, tracks it stops receiving through the SFU)`.
    pub unsubscribe: Vec<(SessionId, Vec<TrackId>)>,
    pub routes: Vec<RouteNotice>,
    pub link_closes: Vec<LinkClose>,
    /// `(session, whether its uplink is active)`.
    pub uplinks: Vec<(SessionId, bool)>,
    pub timeouts: Vec<LinkTimeout>,
}

impl Effects {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// Sessions whose subscriptions change, each once.
    #[must_use]
    pub fn renegotiate(&self) -> Vec<SessionId> {
        let mut sessions: Vec<SessionId> = self
            .subscribe
            .iter()
            .map(|(id, _)| id.clone())
            .chain(self.unsubscribe.iter().map(|(id, _)| id.clone()))
            .collect();
        sessions.sort();
        sessions.dedup();
        sessions
    }

    pub(super) fn absorb(&mut self, events: Vec<LinkEvent>) {
        for event in events {
            match event {
                LinkEvent::Moving { pair, path, epoch } => {
                    for (to, peer) in pair.facing() {
                        self.routes.push(RouteNotice {
                            to: to.clone(),
                            peer: peer.clone(),
                            path,
                            epoch,
                        });
                    }
                }
                LinkEvent::TimeoutArmed { pair, epoch } => {
                    self.timeouts.push(LinkTimeout { pair, epoch });
                }
                LinkEvent::Closed { pair } => {
                    for (to, peer) in pair.facing() {
                        self.link_closes.push(LinkClose { to: to.clone(), peer: peer.clone() });
                    }
                }
            }
        }
    }
}
