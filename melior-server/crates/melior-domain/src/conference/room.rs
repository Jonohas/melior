use super::{ChannelId, ClientOffer, DomainError, Negotiation, SessionId, Track, TrackId};
use std::collections::{BTreeMap, BTreeSet};

/// A participant inside a [`Room`].
#[derive(Debug)]
struct Participant {
    /// Name shown to the other participants.
    username: String,
    negotiation: Negotiation,
    published: Vec<Track>,
    subscriptions: BTreeSet<TrackId>,
}

/// Who a participant is, as the others see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipantInfo {
    pub session_id: SessionId,
    pub username: String,
}

/// What changed for the rest of the room when a participant left.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Departure {
    /// Tracks each remaining participant no longer receives.
    pub unsubscribed: Vec<(SessionId, Vec<TrackId>)>,
    /// Everyone still in the room.
    pub remaining: Vec<SessionId>,
}

/// The room aggregate: who is in a channel, what they publish and who receives what.
///
/// Invariant: a participant whose media connection is established ("ready") is
/// subscribed to every track published by the other participants, and never to its own.
/// The methods return the subscription changes the caller has to carry out on the
/// media plane; the room itself performs no I/O.
#[derive(Debug)]
pub struct Room {
    id: ChannelId,
    participants: BTreeMap<SessionId, Participant>,
}

impl Room {
    #[must_use]
    pub fn new(id: ChannelId) -> Self {
        Self { id, participants: BTreeMap::new() }
    }

    #[must_use]
    pub fn id(&self) -> &ChannelId {
        &self.id
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.participants.is_empty()
    }

    #[must_use]
    pub fn contains(&self, id: &SessionId) -> bool {
        self.participants.contains_key(id)
    }

    /// Adds a participant. Returns who was already there.
    pub fn join(
        &mut self,
        id: SessionId,
        username: impl Into<String>,
    ) -> Result<Vec<ParticipantInfo>, DomainError> {
        if self.participants.contains_key(&id) {
            return Err(DomainError::AlreadyJoined(id));
        }
        let present = self.roster();
        self.participants.insert(
            id,
            Participant {
                username: username.into(),
                negotiation: Negotiation::default(),
                published: Vec::new(),
                subscriptions: BTreeSet::new(),
            },
        );
        Ok(present)
    }

    /// Who publishes a track, if it is still published.
    #[must_use]
    pub fn publisher_of(&self, track_id: &TrackId) -> Option<&SessionId> {
        self.participants
            .iter()
            .find(|(_, participant)| participant.published.iter().any(|t| &t.id == track_id))
            .map(|(id, _)| id)
    }

    /// Everyone in the room.
    #[must_use]
    pub fn roster(&self) -> Vec<ParticipantInfo> {
        self.participants
            .iter()
            .map(|(id, participant)| ParticipantInfo {
                session_id: id.clone(),
                username: participant.username.clone(),
            })
            .collect()
    }

    /// Removes a participant and everything it published.
    pub fn leave(&mut self, id: &SessionId) -> Result<Departure, DomainError> {
        let leaver = self
            .participants
            .remove(id)
            .ok_or_else(|| DomainError::UnknownParticipant(id.clone()))?;

        let mut departure = Departure::default();
        for (other_id, other) in &mut self.participants {
            let lost: Vec<TrackId> = leaver
                .published
                .iter()
                .filter(|track| other.subscriptions.remove(&track.id))
                .map(|track| track.id.clone())
                .collect();
            if !lost.is_empty() {
                departure.unsubscribed.push((other_id.clone(), lost));
            }
            departure.remaining.push(other_id.clone());
        }
        Ok(departure)
    }

    /// Adds a published track. Returns the participants that now have to receive it.
    pub fn publish(
        &mut self,
        publisher: &SessionId,
        track: Track,
    ) -> Result<Vec<SessionId>, DomainError> {
        if self.participants.values().any(|p| p.published.iter().any(|t| t.id == track.id)) {
            return Err(DomainError::DuplicateTrack(track.id));
        }
        self.participant_mut(publisher)?.published.push(track.clone());

        let mut subscribers = Vec::new();
        for (id, participant) in &mut self.participants {
            if id != publisher && participant.negotiation.is_ready() {
                participant.subscriptions.insert(track.id.clone());
                subscribers.push(id.clone());
            }
        }
        Ok(subscribers)
    }

    /// Removes a published track. Returns the participants that were receiving it.
    /// Unknown publishers or tracks are not an error: the track may already be gone
    /// because its publisher left.
    pub fn unpublish(&mut self, publisher: &SessionId, track_id: &TrackId) -> Vec<SessionId> {
        let Some(participant) = self.participants.get_mut(publisher) else {
            return Vec::new();
        };
        let before = participant.published.len();
        participant.published.retain(|t| &t.id != track_id);
        if participant.published.len() == before {
            return Vec::new();
        }

        self.participants
            .iter_mut()
            .filter_map(|(id, p)| p.subscriptions.remove(track_id).then(|| id.clone()))
            .collect()
    }

    pub fn negotiation(&self, id: &SessionId) -> Result<&Negotiation, DomainError> {
        self.participants
            .get(id)
            .map(|p| &p.negotiation)
            .ok_or_else(|| DomainError::UnknownParticipant(id.clone()))
    }

    pub fn negotiation_mut(&mut self, id: &SessionId) -> Result<&mut Negotiation, DomainError> {
        self.participant_mut(id).map(|p| &mut p.negotiation)
    }

    /// Records that a participant's offer was answered.
    pub fn client_offer_answered(&mut self, id: &SessionId) -> Result<ClientOffer, DomainError> {
        Ok(self.participant_mut(id)?.negotiation.offer_answered())
    }

    /// Records that a participant's media connection is up. The first time, the
    /// participant becomes ready and is subscribed to everything the others already
    /// publish; those tracks are returned.
    pub fn media_connected(&mut self, id: &SessionId) -> Result<Vec<Track>, DomainError> {
        if !self.participant_mut(id)?.negotiation.connected() {
            return Ok(Vec::new());
        }

        let existing: Vec<Track> = self
            .participants
            .iter()
            .filter(|(other, _)| *other != id)
            .flat_map(|(_, p)| p.published.iter().cloned())
            .collect();
        let participant = self.participant_mut(id)?;
        participant.subscriptions.extend(existing.iter().map(|t| t.id.clone()));
        Ok(existing)
    }

    /// Forgets a subscription the media plane could not establish.
    pub fn subscription_failed(&mut self, subscriber: &SessionId, track_id: &TrackId) {
        if let Some(participant) = self.participants.get_mut(subscriber) {
            participant.subscriptions.remove(track_id);
        }
    }

    pub fn subscriptions(&self, id: &SessionId) -> Result<Vec<TrackId>, DomainError> {
        self.participants
            .get(id)
            .map(|p| p.subscriptions.iter().cloned().collect())
            .ok_or_else(|| DomainError::UnknownParticipant(id.clone()))
    }

    fn participant_mut(&mut self, id: &SessionId) -> Result<&mut Participant, DomainError> {
        self.participants.get_mut(id).ok_or_else(|| DomainError::UnknownParticipant(id.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MediaKind;

    fn room() -> Room {
        Room::new(ChannelId::parse("general").unwrap())
    }

    fn video() -> Track {
        Track::new(TrackId::generate(), MediaKind::Video)
    }

    /// Joins a participant, completes its first offer and connects its media.
    fn join_ready(room: &mut Room) -> (SessionId, Vec<Track>) {
        let id = SessionId::generate();
        room.join(id.clone(), "someone").unwrap();
        assert_eq!(room.client_offer_answered(&id).unwrap(), ClientOffer::Initial);
        let existing = room.media_connected(&id).unwrap();
        (id, existing)
    }

    #[test]
    fn joining_twice_is_rejected() {
        let mut room = room();
        let id = SessionId::generate();
        room.join(id.clone(), "alice").unwrap();
        assert_eq!(room.join(id.clone(), "alice"), Err(DomainError::AlreadyJoined(id)));
    }

    #[test]
    fn joining_reports_who_is_already_there() {
        let mut room = room();
        let alice = SessionId::generate();
        assert!(room.join(alice.clone(), "alice").unwrap().is_empty());

        let present = room.join(SessionId::generate(), "bob").unwrap();
        assert_eq!(present, vec![ParticipantInfo { session_id: alice, username: "alice".into() }]);
        assert_eq!(room.roster().len(), 2);
    }

    #[test]
    fn published_track_goes_to_ready_participants_but_not_the_publisher() {
        let mut room = room();
        let (alice, _) = join_ready(&mut room);
        let (bob, _) = join_ready(&mut room);
        let connecting = SessionId::generate();
        room.join(connecting.clone(), "connecting").unwrap();

        let track = video();
        let subscribers = room.publish(&alice, track.clone()).unwrap();

        assert_eq!(subscribers, vec![bob.clone()]);
        assert_eq!(room.subscriptions(&bob).unwrap(), vec![track.id.clone()]);
        assert!(room.subscriptions(&alice).unwrap().is_empty());
        assert!(room.subscriptions(&connecting).unwrap().is_empty());
    }

    #[test]
    fn late_joiner_receives_every_existing_track() {
        let mut room = room();
        let (alice, _) = join_ready(&mut room);
        let (bob, _) = join_ready(&mut room);
        let a = video();
        let b = video();
        room.publish(&alice, a.clone()).unwrap();
        room.publish(&bob, b.clone()).unwrap();

        let (carol, existing) = join_ready(&mut room);

        let mut expected = vec![a.id.clone(), b.id.clone()];
        expected.sort();
        let mut got: Vec<TrackId> = existing.into_iter().map(|t| t.id).collect();
        got.sort();
        assert_eq!(got, expected);
        assert_eq!(room.subscriptions(&carol).unwrap(), expected);
    }

    #[test]
    fn a_participant_that_is_still_connecting_receives_tracks_once_connected() {
        let mut room = room();
        let (alice, _) = join_ready(&mut room);
        let bob = SessionId::generate();
        room.join(bob.clone(), "bob").unwrap();
        room.client_offer_answered(&bob).unwrap();

        let track = video();
        assert!(room.publish(&alice, track.clone()).unwrap().is_empty());
        assert_eq!(room.media_connected(&bob).unwrap(), vec![track]);
    }

    #[test]
    fn reconnecting_or_reoffering_does_not_resubscribe() {
        let mut room = room();
        let (alice, _) = join_ready(&mut room);
        let (bob, _) = join_ready(&mut room);
        room.publish(&alice, video()).unwrap();

        assert_eq!(room.client_offer_answered(&bob).unwrap(), ClientOffer::Renegotiation);
        assert!(room.media_connected(&bob).unwrap().is_empty());
    }

    #[test]
    fn leaving_unsubscribes_the_others_and_frees_the_room() {
        let mut room = room();
        let (alice, _) = join_ready(&mut room);
        let (bob, _) = join_ready(&mut room);
        let track = video();
        room.publish(&alice, track.clone()).unwrap();

        let departure = room.leave(&alice).unwrap();

        assert_eq!(
            departure,
            Departure {
                unsubscribed: vec![(bob.clone(), vec![track.id.clone()])],
                remaining: vec![bob.clone()],
            }
        );
        assert!(room.subscriptions(&bob).unwrap().is_empty());
        assert_eq!(room.leave(&alice), Err(DomainError::UnknownParticipant(alice)));

        room.leave(&bob).unwrap();
        assert!(room.is_empty());
    }

    #[test]
    fn unpublish_reports_subscribers_and_is_idempotent() {
        let mut room = room();
        let (alice, _) = join_ready(&mut room);
        let (bob, _) = join_ready(&mut room);
        let track = video();
        room.publish(&alice, track.clone()).unwrap();

        assert_eq!(room.unpublish(&alice, &track.id), vec![bob]);
        assert!(room.unpublish(&alice, &track.id).is_empty());
        assert!(room.unpublish(&SessionId::generate(), &track.id).is_empty());
    }

    #[test]
    fn a_track_is_attributed_to_its_publisher_while_it_is_published() {
        let mut room = room();
        let (alice, _) = join_ready(&mut room);
        let (_bob, _) = join_ready(&mut room);
        let track = video();
        assert_eq!(room.publisher_of(&track.id), None);

        room.publish(&alice, track.clone()).unwrap();
        assert_eq!(room.publisher_of(&track.id), Some(&alice));

        room.unpublish(&alice, &track.id);
        assert_eq!(room.publisher_of(&track.id), None);
    }

    #[test]
    fn a_track_id_can_only_be_published_once() {
        let mut room = room();
        let (alice, _) = join_ready(&mut room);
        let (bob, _) = join_ready(&mut room);
        let track = video();
        room.publish(&alice, track.clone()).unwrap();

        assert_eq!(room.publish(&bob, track.clone()), Err(DomainError::DuplicateTrack(track.id)));
    }
}
