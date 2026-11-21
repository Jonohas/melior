//! Changes to categories and channels: everything here needs `manage_channels`.

use super::{
    Category, CategoryId, Channel, ChannelKind, CommunityError, CommunityState, Name, Override,
    Permissions, Rejected, Standing, Topic,
};
use crate::conference::{ChannelId, ChannelMedia, MediaMode, MeshPolicy};

/// A new channel as a client asks for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelDraft {
    pub name: Name,
    pub kind: ChannelKind,
    pub topic: Topic,
    pub category_id: Option<CategoryId>,
    pub media: ChannelMedia,
}

/// Changes to a channel; `None` leaves a field alone. `category_id: Some(None)`
/// takes the channel out of its category, `user_limit: Some(None)` removes the limit.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChannelChanges {
    pub name: Option<Name>,
    pub topic: Option<Topic>,
    pub category_id: Option<Option<CategoryId>>,
    pub position: Option<u32>,
    pub media_mode: Option<MediaMode>,
    pub user_limit: Option<Option<u8>>,
    pub overrides: Option<Vec<Override>>,
    pub synced: Option<bool>,
}

/// Changes to a category; `None` leaves a field alone.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CategoryChanges {
    pub name: Option<Name>,
    pub position: Option<u32>,
    pub overrides: Option<Vec<Override>>,
}

impl CommunityState {
    /// A new category after the existing ones.
    pub fn new_category(&self, actor: &Standing, name: Name) -> Result<Category, Rejected> {
        actor.require(Permissions::MANAGE_CHANNELS)?;
        Ok(Category {
            id: CategoryId::generate(),
            name,
            position: self.next_category_position(),
            overrides: Vec::new(),
        })
    }

    pub fn edit_category(
        &self,
        actor: &Standing,
        id: &CategoryId,
        changes: CategoryChanges,
    ) -> Result<Category, Rejected> {
        let category = self.known_category(id)?;
        if let Some(overrides) = &changes.overrides {
            self.check_overrides(overrides)?;
        }
        actor.require(Permissions::MANAGE_CHANNELS)?;
        Ok(Category {
            id: category.id.clone(),
            name: changes.name.unwrap_or_else(|| category.name.clone()),
            position: changes.position.unwrap_or(category.position),
            overrides: changes.overrides.unwrap_or_else(|| category.overrides.clone()),
        })
    }

    /// Deleting a category; its channels stay, without a category.
    pub fn delete_category(&self, actor: &Standing, id: &CategoryId) -> Result<(), Rejected> {
        self.known_category(id)?;
        actor.require(Permissions::MANAGE_CHANNELS)?;
        Ok(())
    }

    /// A new channel after the existing ones. In a category it starts synced with it.
    pub fn new_channel(
        &self,
        actor: &Standing,
        draft: ChannelDraft,
        policy: MeshPolicy,
    ) -> Result<Channel, Rejected> {
        if let Some(category) = &draft.category_id {
            self.known_category(category)?;
        }
        Channel::check_media(draft.kind, draft.media, policy)?;
        actor.require(Permissions::MANAGE_CHANNELS)?;
        Ok(Channel {
            id: ChannelId::generate(),
            name: draft.name,
            topic: draft.topic,
            kind: draft.kind,
            synced: draft.category_id.is_some(),
            category_id: draft.category_id,
            position: self.next_channel_position(),
            overrides: Vec::new(),
            media: draft.media,
        })
    }

    /// The channel with `changes` applied. New overrides unsync it from its
    /// category; `synced` in the same change has the last word.
    pub fn edit_channel(
        &self,
        actor: &Standing,
        id: &ChannelId,
        changes: ChannelChanges,
        policy: MeshPolicy,
    ) -> Result<Channel, Rejected> {
        let mut channel = self.known_channel(id)?.clone();
        if let Some(Some(category)) = &changes.category_id {
            self.known_category(category)?;
        }
        if let Some(overrides) = &changes.overrides {
            self.check_overrides(overrides)?;
        }
        let media = ChannelMedia {
            mode: changes.media_mode.unwrap_or(channel.media.mode),
            user_limit: changes.user_limit.unwrap_or(channel.media.user_limit),
        };
        Channel::check_media(channel.kind, media, policy)?;
        actor.require(Permissions::MANAGE_CHANNELS)?;

        channel.media = media;
        apply_channel_changes(&mut channel, changes);
        Ok(channel)
    }

    pub fn delete_channel(&self, actor: &Standing, id: &ChannelId) -> Result<(), Rejected> {
        self.known_channel(id)?;
        actor.require(Permissions::MANAGE_CHANNELS)?;
        Ok(())
    }

    fn known_category(&self, id: &CategoryId) -> Result<&Category, CommunityError> {
        self.category(id).ok_or_else(|| CommunityError::UnknownCategory(id.to_string()))
    }

    fn known_channel(&self, id: &ChannelId) -> Result<&Channel, CommunityError> {
        self.channel(id).ok_or(CommunityError::UnknownChannel)
    }
}

fn apply_channel_changes(channel: &mut Channel, changes: ChannelChanges) {
    if let Some(name) = changes.name {
        channel.name = name;
    }
    if let Some(topic) = changes.topic {
        channel.topic = topic;
    }
    if let Some(category_id) = changes.category_id {
        channel.category_id = category_id;
    }
    if let Some(position) = changes.position {
        channel.position = position;
    }
    if let Some(overrides) = changes.overrides {
        channel.overrides = overrides;
        channel.synced = false;
    }
    if let Some(synced) = changes.synced {
        channel.synced = synced;
    }
}
