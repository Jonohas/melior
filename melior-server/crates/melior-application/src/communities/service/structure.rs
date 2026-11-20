//! Categories and channels.

use super::{ChannelView, CommunitiesError, CommunityService};
use crate::communities::ports::CommunityEvent;
use melior_domain::{
    Category, CategoryChanges, CategoryId, ChannelChanges, ChannelDraft, ChannelId, CommunityId,
    Name, User,
};
use tracing::info;

impl CommunityService {
    pub async fn create_category(
        &self,
        id: &CommunityId,
        user: &User,
        name: &str,
    ) -> Result<Category, CommunitiesError> {
        let name = Name::parse(name)?;
        let seat = self.seat(id, user).await?;
        let category = seat.state.new_category(&seat.standing, name)?;
        self.store.save_category(id, &category).await?;
        self.publish(id, CommunityEvent::StructureChanged).await;
        Ok(category)
    }

    pub async fn edit_category(
        &self,
        id: &CommunityId,
        user: &User,
        category_id: &CategoryId,
        changes: CategoryChanges,
    ) -> Result<Category, CommunitiesError> {
        let seat = self.seat(id, user).await?;
        seat.state.category(category_id).ok_or(CommunitiesError::NotFound)?;
        let category = seat.state.edit_category(&seat.standing, category_id, changes)?;
        self.store.save_category(id, &category).await?;
        self.publish(id, CommunityEvent::StructureChanged).await;
        Ok(category)
    }

    /// Deletes a category; its channels stay, without one.
    pub async fn delete_category(
        &self,
        id: &CommunityId,
        user: &User,
        category_id: &CategoryId,
    ) -> Result<(), CommunitiesError> {
        let seat = self.seat(id, user).await?;
        seat.state.category(category_id).ok_or(CommunitiesError::NotFound)?;
        seat.state.delete_category(&seat.standing, category_id)?;
        self.store.delete_category(id, category_id).await?;
        self.publish(id, CommunityEvent::StructureChanged).await;
        Ok(())
    }

    pub async fn create_channel(
        &self,
        id: &CommunityId,
        user: &User,
        draft: ChannelDraft,
    ) -> Result<ChannelView, CommunitiesError> {
        let seat = self.seat(id, user).await?;
        let channel = seat.state.new_channel(&seat.standing, draft, self.mesh)?;
        self.store.save_channel(id, &channel).await?;
        info!(community = %id, channel = %channel.id, kind = channel.kind.as_str(), "channel created");
        self.publish(id, CommunityEvent::StructureChanged).await;
        let permissions = seat.state.channel_permissions(&seat.standing, &channel);
        Ok(self.channel_view(id, &channel, permissions))
    }

    /// One channel, for a member who may view it.
    pub async fn channel(
        &self,
        channel_id: &ChannelId,
        user: &User,
    ) -> Result<ChannelView, CommunitiesError> {
        let seat = self.seat_by_channel(channel_id, user).await?;
        let channel = seat.state.channel(channel_id).ok_or(CommunitiesError::NotFound)?;
        let permissions = seat.state.channel_permissions(&seat.standing, channel);
        if permissions.is_empty() {
            return Err(CommunitiesError::Forbidden);
        }
        Ok(self.channel_view(seat.state.id(), channel, permissions))
    }

    pub async fn edit_channel(
        &self,
        channel_id: &ChannelId,
        user: &User,
        changes: ChannelChanges,
    ) -> Result<ChannelView, CommunitiesError> {
        let mut seat = self.seat_by_channel(channel_id, user).await?;
        let channel = seat.state.edit_channel(&seat.standing, channel_id, changes, self.mesh)?;
        let id = seat.state.id().clone();
        self.store.save_channel(&id, &channel).await?;
        self.publish(&id, CommunityEvent::StructureChanged).await;
        seat.state.channels.retain(|c| c.id != channel.id);
        seat.state.channels.push(channel.clone());
        let permissions = seat.state.channel_permissions(&seat.standing, &channel);
        Ok(self.channel_view(&id, &channel, permissions))
    }

    /// Deletes a channel and its history. Its participants are removed at their
    /// next re-check.
    pub async fn delete_channel(
        &self,
        channel_id: &ChannelId,
        user: &User,
    ) -> Result<(), CommunitiesError> {
        let seat = self.seat_by_channel(channel_id, user).await?;
        seat.state.delete_channel(&seat.standing, channel_id)?;
        self.store.delete_channel(channel_id).await?;
        info!(community = %seat.state.id(), channel = %channel_id, "channel deleted");
        self.publish(seat.state.id(), CommunityEvent::StructureChanged).await;
        Ok(())
    }
}
