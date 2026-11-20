use super::{
    ChannelId, ClientOffer, DomainError, Effects, LinkClose, Links, MediaKind, MeshPolicy,
    Negotiation, RoomTarget, SessionId, Track, TrackId, TrackSource,
};
use std::collections::{BTreeMap, BTreeSet};

/// A participant inside a [`Room`].
#[derive(Debug)]
pub(super) struct Participant {
    /// Name shown to the other participants.
    pub(super) username: String,
    pub(super) negotiation: Negotiation,
    pub(super) published: Vec<Track>,
    pub(super) subscriptions: BTreeSet<TrackId>,
    /// Whether the participant was last told to send on the SFU connection.
    pub(super) uplink_active: bool,
}

/// Who a participant is, as the others see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipantInfo {
    pub session_id: SessionId,
    pub username: String,
}

/// What a join changed.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Joined {
    /// Who was already there.
    pub present: Vec<ParticipantInfo>,
    pub effects: Effects,
}

/// What changed for the rest of the room when a participant left.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Departure {
    /// Everyone still in the room.
    pub remaining: Vec<SessionId>,
    /// Tracks the others no longer receive, link closes, route changes and uplinks.
    pub effects: Effects,
}

/// The room aggregate: who is in a channel, what they publish and who receives what.
///
/// Invariant: a participant R whose media connection is established ("ready") is
/// subscribed to a track of another participant P exactly when the link between P and
/// R is not `Direct`, and never to its own tracks. The methods return the subscription
/// changes the caller has to carry out on the media plane; the room itself performs
/// no I/O. The mesh rules (target, links, uplinks) live in `mesh`, `links` and
/// `room_mesh`.
#[derive(Debug)]
pub struct Room {
    id: ChannelId,
    pub(super) participants: BTreeMap<SessionId, Participant>,
    pub(super) links: Links,
    pub(super) target: RoomTarget,
}

impl Room {
    /// A room without mesh: everything is forwarded.
    #[must_use]
    pub fn new(id: ChannelId) -> Self {
        Self::with_mesh(id, MeshPolicy::OFF)
    }

    #[must_use]
    pub fn with_mesh(id: ChannelId, policy: MeshPolicy) -> Self {
        Self {
            id,
            participants: BTreeMap::new(),
            links: Links::default(),
            target: RoomTarget::new(policy),
        }
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

    /// Adds a participant, links it to everyone already there and re-evaluates the
    /// room's target.
    pub fn join(
        &mut self,
        id: SessionId,
        username: impl Into<String>,
    ) -> Result<Joined, DomainError> {
        if self.participants.contains_key(&id) {
            return Err(DomainError::AlreadyJoined(id));
        }
        let present = self.roster();
        for other in &present {
            self.links.add(&other.session_id, &id);
        }
        self.participants.insert(
            id,
            Participant {
                username: username.into(),
                negotiation: Negotiation::default(),
                published: Vec::new(),
                subscriptions: BTreeSet::new(),
                uplink_active: true,
            },
        );
        let effects = self.retarget();
        Ok(Joined { present, effects })
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

    /// Removes a participant, its links and everything it published.
    pub fn leave(&mut self, id: &SessionId) -> Result<Departure, DomainError> {
        if self.participants.remove(id).is_none() {
            return Err(DomainError::UnknownParticipant(id.clone()));
        }
        let had_direct_link = self.links.remove_participant(id);
        let mut effects = self.retarget();
        let closes =
            had_direct_link.into_iter().map(|peer| LinkClose { to: peer, peer: id.clone() });
        effects.link_closes.splice(0..0, closes);
        let remaining = self.participants.keys().cloned().collect();
        Ok(Departure { remaining, effects })
    }

    /// Where a track comes from, if it is still published.
    #[must_use]
    pub fn source_of(&self, track_id: &TrackId) -> Option<TrackSource> {
        self.participants
            .values()
            .flat_map(|participant| participant.published.iter())
            .find(|track| &track.id == track_id)
            .map(|track| track.source)
    }

    /// Adds a published track. Returns the participants that now have to receive it.
    ///
    /// A client does not say what a track shows, so the room works it out: a participant's
    /// first video is its camera, and a video it publishes next to that one is its screen.
    /// A third video is refused, so one participant shows at most one screen at a time.
    pub fn publish(
        &mut self,
        publisher: &SessionId,
        mut track: Track,
    ) -> Result<Vec<SessionId>, DomainError> {
        if self.participants.values().any(|p| p.published.iter().any(|t| t.id == track.id)) {
            return Err(DomainError::DuplicateTrack(track.id));
        }
        let participant = self.participant_mut(publisher)?;
        if track.kind == MediaKind::Video {
            let videos = participant.published.iter().filter(|t| t.kind == MediaKind::Video);
            track.source = match videos.count() {
                0 => TrackSource::Camera,
                1 => TrackSource::Screen,
                _ => return Err(DomainError::TooManyVideoTracks(publisher.clone())),
            };
        }
        participant.published.push(track.clone());

        let subscribers: Vec<SessionId> = self
            .participants
            .keys()
            .filter(|id| self.receives_through_sfu(publisher, id))
            .cloned()
            .collect();
        for id in &subscribers {
            self.participant_mut(id)?.subscriptions.insert(track.id.clone());
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
            .filter(|(other, _)| *other != id && self.link_is_not_direct(other, id))
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

    pub(super) fn participant_mut(
        &mut self,
        id: &SessionId,
    ) -> Result<&mut Participant, DomainError> {
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
        assert!(room.join(alice.clone(), "alice").unwrap().present.is_empty());

        let present = room.join(SessionId::generate(), "bob").unwrap().present;
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
                remaining: vec![bob.clone()],
                effects: Effects {
                    unsubscribe: vec![(bob.clone(), vec![track.id.clone()])],
                    ..Effects::default()
                },
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

    fn audio() -> Track {
        Track::new(TrackId::generate(), MediaKind::Audio)
    }

    #[test]
    fn the_first_video_is_the_camera_and_the_next_one_is_the_screen() {
        let mut room = room();
        let (alice, _) = join_ready(&mut room);
        let (camera, screen) = (video(), video());

        room.publish(&alice, camera.clone()).unwrap();
        room.publish(&alice, screen.clone()).unwrap();

        assert_eq!(room.source_of(&camera.id), Some(TrackSource::Camera));
        assert_eq!(room.source_of(&screen.id), Some(TrackSource::Screen));
    }

    #[test]
    fn audio_belongs_to_the_camera_whatever_is_published_around_it() {
        let mut room = room();
        let (alice, _) = join_ready(&mut room);
        let (voice, camera, screen) = (audio(), video(), video());

        room.publish(&alice, voice.clone()).unwrap();
        room.publish(&alice, camera).unwrap();
        room.publish(&alice, screen).unwrap();

        assert_eq!(room.source_of(&voice.id), Some(TrackSource::Camera));
    }

    #[test]
    fn a_participant_shows_one_screen_at_a_time() {
        let mut room = room();
        let (alice, _) = join_ready(&mut room);
        room.publish(&alice, video()).unwrap();
        room.publish(&alice, video()).unwrap();

        assert_eq!(
            room.publish(&alice, video()),
            Err(DomainError::TooManyVideoTracks(alice.clone()))
        );
    }

    #[test]
    fn a_screen_can_be_shared_again_after_it_stopped() {
        let mut room = room();
        let (alice, _) = join_ready(&mut room);
        room.publish(&alice, video()).unwrap();
        let first = video();
        room.publish(&alice, first.clone()).unwrap();
        room.unpublish(&alice, &first.id);

        let second = video();
        room.publish(&alice, second.clone()).unwrap();

        assert_eq!(room.source_of(&second.id), Some(TrackSource::Screen));
        assert_eq!(room.source_of(&first.id), None);
    }

    #[test]
    fn everyone_ready_receives_a_screen_and_a_late_joiner_gets_it_with_its_source() {
        let mut room = room();
        let (alice, _) = join_ready(&mut room);
        let (bob, _) = join_ready(&mut room);
        room.publish(&alice, video()).unwrap();
        let screen = video();

        assert_eq!(room.publish(&alice, screen.clone()).unwrap(), vec![bob]);

        let (_carol, existing) = join_ready(&mut room);
        let sources: Vec<TrackSource> = existing.iter().map(|t| t.source).collect();
        assert_eq!(sources, [TrackSource::Camera, TrackSource::Screen]);
    }
}
