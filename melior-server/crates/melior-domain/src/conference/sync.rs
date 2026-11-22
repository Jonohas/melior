//! Lip sync on the forwarded path: which media sections a receiver plays in step.
//!
//! A browser plays the audio and video of one stream (the `a=msid` stream id of
//! their media sections) in step with each other, and holds the earlier one back
//! to match the later. That is wanted for one participant's microphone and camera,
//! and for its screen's picture and sound. It is not wanted across participants:
//! one shared stream would hold everyone's video back to match whichever audio
//! plays latest.
//!
//! A media section is reused for other tracks and its stream id cannot change, so
//! the stream id belongs to the section. A track therefore goes to a section whose
//! stream id is already its group's, or to one whose stream id no other group uses.

use super::{SessionId, TrackSource};
use std::fmt;
use uuid::Uuid;

/// The tracks a receiver plays in step: one participant's microphone and camera,
/// or its screen's picture and sound. Each holds at most one audio track, which
/// is what a browser can line the video up against.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SyncGroup {
    publisher: SessionId,
    source: TrackSource,
}

impl SyncGroup {
    #[must_use]
    pub fn new(publisher: SessionId, source: TrackSource) -> Self {
        Self { publisher, source }
    }

    #[must_use]
    pub fn publisher(&self) -> &SessionId {
        &self.publisher
    }

    #[must_use]
    pub fn source(&self) -> TrackSource {
        self.source
    }
}

/// The stream id of a media section, as a receiver sees it. Says nothing about
/// whose track the section carries.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StreamLabel(String);

impl StreamLabel {
    #[must_use]
    pub fn generate() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StreamLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// One media section of a subscriber's connection, as [`choose_section`] sees it.
#[derive(Debug, Clone, Copy)]
pub struct SectionView<'a> {
    pub label: &'a StreamLabel,
    /// The group of the track it carries; `None` while it is idle.
    pub carrying: Option<&'a SyncGroup>,
    /// Whether it can carry the new track (same kind and codec).
    pub fits: bool,
}

/// Where a new track goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionChoice {
    /// The idle section at this index.
    Reuse(usize),
    /// A new section with this stream id.
    Open(StreamLabel),
}

/// Picks the media section for a track of `group`, so a receiver plays it in step
/// with the rest of its group and with nobody else's.
///
/// - If the group already has a section, the track goes to an idle fitting section
///   with the same stream id, or to a new section with that id.
/// - Otherwise it goes to an idle fitting section whose stream id no section in use
///   carries, or to a new section with a new id.
///
/// The number of sections stays bounded: a stream id serves one group at a time
/// and gets at most one section per kind of track.
#[must_use]
pub fn choose_section(group: &SyncGroup, sections: &[SectionView<'_>]) -> SectionChoice {
    let label_in_use = |label: &StreamLabel| {
        sections.iter().any(|section| section.carrying.is_some() && section.label == label)
    };
    let own = sections
        .iter()
        .find(|section| section.carrying == Some(group))
        .map(|section| section.label);
    let idle = |index: &usize| sections[*index].fits && sections[*index].carrying.is_none();
    let reusable = match own {
        Some(label) => (0..sections.len()).filter(idle).find(|&i| sections[i].label == label),
        None => (0..sections.len()).filter(idle).find(|&i| !label_in_use(sections[i].label)),
    };
    match (reusable, own) {
        (Some(index), _) => SectionChoice::Reuse(index),
        (None, Some(label)) => SectionChoice::Open(label.clone()),
        (None, None) => SectionChoice::Open(StreamLabel::generate()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group(publisher: &SessionId, source: TrackSource) -> SyncGroup {
        SyncGroup::new(publisher.clone(), source)
    }

    fn view<'a>(
        label: &'a StreamLabel,
        carrying: Option<&'a SyncGroup>,
        fits: bool,
    ) -> SectionView<'a> {
        SectionView { label, carrying, fits }
    }

    #[test]
    fn a_first_track_opens_a_section_with_a_new_label() {
        let alice = group(&SessionId::generate(), TrackSource::Camera);
        assert!(matches!(choose_section(&alice, &[]), SectionChoice::Open(_)));
    }

    #[test]
    fn the_second_track_of_a_group_joins_its_label() {
        let alice = group(&SessionId::generate(), TrackSource::Camera);
        let label = StreamLabel::generate();
        let sections = [view(&label, Some(&alice), false)];
        assert_eq!(choose_section(&alice, &sections), SectionChoice::Open(label.clone()));
    }

    #[test]
    fn an_idle_section_of_the_groups_label_is_reused() {
        let alice = group(&SessionId::generate(), TrackSource::Camera);
        let own = StreamLabel::generate();
        let other = StreamLabel::generate();
        let sections =
            [view(&other, None, true), view(&own, Some(&alice), false), view(&own, None, true)];
        assert_eq!(choose_section(&alice, &sections), SectionChoice::Reuse(2));
    }

    #[test]
    fn a_group_never_takes_a_section_of_another_label_when_it_has_one() {
        let alice = group(&SessionId::generate(), TrackSource::Camera);
        let own = StreamLabel::generate();
        let other = StreamLabel::generate();
        let sections = [view(&own, Some(&alice), false), view(&other, None, true)];
        assert_eq!(choose_section(&alice, &sections), SectionChoice::Open(own.clone()));
    }

    #[test]
    fn a_new_group_never_shares_a_label_in_use() {
        let bob = SessionId::generate();
        let alice = group(&SessionId::generate(), TrackSource::Camera);
        let bobs_camera = group(&bob, TrackSource::Camera);
        let used = StreamLabel::generate();
        let free = StreamLabel::generate();
        // Bob's microphone is on `used`; his camera section there is idle.
        let sections = [
            view(&used, Some(&bobs_camera), false),
            view(&used, None, true),
            view(&free, None, true),
        ];
        assert_eq!(choose_section(&alice, &sections), SectionChoice::Reuse(2));

        let only_used = &sections[..2];
        let SectionChoice::Open(label) = choose_section(&alice, only_used) else {
            panic!("a label in use must not be shared");
        };
        assert_ne!(label, used);
    }

    #[test]
    fn a_screen_is_not_in_step_with_the_camera() {
        let alice = SessionId::generate();
        let camera = group(&alice, TrackSource::Camera);
        let screen = group(&alice, TrackSource::Screen);
        let label = StreamLabel::generate();
        let sections = [view(&label, Some(&camera), false), view(&label, None, true)];
        let SectionChoice::Open(new) = choose_section(&screen, &sections) else {
            panic!("the screen must not join the camera's label");
        };
        assert_ne!(new, label);
    }

    #[test]
    fn a_section_that_does_not_fit_is_never_reused() {
        let alice = group(&SessionId::generate(), TrackSource::Camera);
        let label = StreamLabel::generate();
        let sections = [view(&label, None, false)];
        assert!(matches!(choose_section(&alice, &sections), SectionChoice::Open(l) if l != label));
    }

    #[test]
    fn guests_coming_and_going_reuse_the_same_sections() {
        // A guest publishes audio and video, leaves, and the next one takes both sections.
        let label = StreamLabel::generate();
        let next = group(&SessionId::generate(), TrackSource::Camera);
        let idle = [view(&label, None, true), view(&label, None, true)];
        assert_eq!(choose_section(&next, &idle), SectionChoice::Reuse(0));
        let after_audio = [view(&label, Some(&next), false), view(&label, None, true)];
        assert_eq!(choose_section(&next, &after_audio), SectionChoice::Reuse(1));
    }
}
