//! The mesh half of [`Room`]: the target, the links between participants, the
//! subscriptions and uplinks that follow from them.

use super::room::Participant;
use super::{Effects, MediaPath, Pair, Room, Route, SessionId, Track, TrackId};
use std::collections::BTreeSet;

impl Room {
    /// The path the room as a whole wants.
    #[must_use]
    pub fn target(&self) -> MediaPath {
        self.target.path()
    }

    /// True after a direct link failed, until the next join or leave.
    #[must_use]
    pub fn is_pinned(&self) -> bool {
        self.target.is_pinned()
    }

    /// The state of the link between two participants, if both are in the room.
    #[must_use]
    pub fn route(&self, a: &SessionId, b: &SessionId) -> Option<Route> {
        self.links.route(a, b)
    }

    /// The epoch of the link between two participants, if both are in the room.
    #[must_use]
    pub fn epoch(&self, a: &SessionId, b: &SessionId) -> Option<u64> {
        self.links.epoch(a, b)
    }

    /// Whether a message may be relayed between two participants: they share this
    /// room and a direct connection exists (or is being set up or torn down).
    #[must_use]
    pub fn may_relay(&self, from: &SessionId, to: &SessionId) -> bool {
        self.links.route(from, to).is_some_and(Route::has_direct_link)
    }

    /// `reporter` plays `peer`'s media over the path of route `epoch`. A stale or
    /// foreign report changes nothing.
    pub fn route_live(&mut self, reporter: &SessionId, peer: &SessionId, epoch: u64) -> Effects {
        let events = self.links.report(reporter, peer, epoch);
        self.settle(events)
    }

    /// The `ToDirect` timeout armed for `epoch` fired. If the link is still waiting for
    /// its reports, the direct path failed. A stale epoch or a vanished link changes
    /// nothing.
    pub fn link_timeout(&mut self, pair: &Pair, epoch: u64) -> Effects {
        if !self.links.timed_out(pair, epoch) {
            return Effects::default();
        }
        self.fail_direct()
    }

    /// `reporter` says its direct connection to `peer` failed. Counts only while a direct
    /// connection exists (`ToDirect`, `Direct` or `ToForwarded`); otherwise ignored.
    pub fn link_failed(&mut self, reporter: &SessionId, peer: &SessionId) -> Effects {
        if !self.may_relay(reporter, peer) {
            return Effects::default();
        }
        self.fail_direct()
    }

    /// A direct link failed: pin the room to forwarding and move every link towards it.
    fn fail_direct(&mut self) -> Effects {
        self.target.pin_forwarded();
        let events = self.links.apply_target(self.target.path());
        self.settle(events)
    }

    /// Re-evaluates the target after a join or a leave and moves the links towards it.
    pub(super) fn retarget(&mut self) -> Effects {
        let path = self.target.evaluate(self.participants.len());
        let events = self.links.apply_target(path);
        self.settle(events)
    }

    /// Whether `subscriber` is ready and receives `publisher`'s tracks through the SFU.
    pub(super) fn receives_through_sfu(
        &self,
        publisher: &SessionId,
        subscriber: &SessionId,
    ) -> bool {
        publisher != subscriber
            && self.link_is_not_direct(publisher, subscriber)
            && self.participants.get(subscriber).is_some_and(|p| p.negotiation.is_ready())
    }

    pub(super) fn link_is_not_direct(&self, a: &SessionId, b: &SessionId) -> bool {
        self.links.route(a, b) != Some(Route::Direct)
    }

    /// Turns link events into effects and restores the subscription invariant and the
    /// uplink rule, which both follow from the routes.
    fn settle(&mut self, events: Vec<super::LinkEvent>) -> Effects {
        let mut effects = Effects::default();
        effects.absorb(events);
        self.sync_subscriptions(&mut effects);
        self.sync_uplinks(&mut effects);
        effects
    }

    fn sync_subscriptions(&mut self, effects: &mut Effects) {
        let changes: Vec<(SessionId, Vec<Track>, Vec<TrackId>)> = self
            .participants
            .iter()
            .filter(|(_, p)| p.negotiation.is_ready())
            .map(|(id, p)| {
                let (add, remove) = self.subscription_diff(id, p);
                (id.clone(), add, remove)
            })
            .filter(|(_, add, remove)| !add.is_empty() || !remove.is_empty())
            .collect();
        for (id, add, remove) in changes {
            if let Some(participant) = self.participants.get_mut(&id) {
                participant.subscriptions.extend(add.iter().map(|t| t.id.clone()));
                for track in &remove {
                    participant.subscriptions.remove(track);
                }
            }
            if !add.is_empty() {
                effects.subscribe.push((id.clone(), add));
            }
            if !remove.is_empty() {
                effects.unsubscribe.push((id, remove));
            }
        }
    }

    /// What a ready participant has to start and stop receiving to satisfy the invariant.
    fn subscription_diff(
        &self,
        id: &SessionId,
        participant: &Participant,
    ) -> (Vec<Track>, Vec<TrackId>) {
        let wanted: Vec<&Track> = self
            .participants
            .iter()
            .filter(|(other, _)| *other != id && self.link_is_not_direct(other, id))
            .flat_map(|(_, other)| other.published.iter())
            .collect();
        let wanted_ids: BTreeSet<&TrackId> = wanted.iter().map(|track| &track.id).collect();
        let add = wanted
            .iter()
            .filter(|track| !participant.subscriptions.contains(&track.id))
            .map(|track| (*track).clone())
            .collect();
        let remove = participant
            .subscriptions
            .iter()
            .filter(|track| !wanted_ids.contains(track))
            .cloned()
            .collect();
        (add, remove)
    }

    fn sync_uplinks(&mut self, effects: &mut Effects) {
        let paused = self.links.paused_uplinks();
        for (id, participant) in &mut self.participants {
            let active = !paused.contains(id);
            if participant.uplink_active != active {
                participant.uplink_active = active;
                effects.uplinks.push((id.clone(), active));
            }
        }
    }
}
