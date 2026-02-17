namespace Domain;

public class Session : Entity<Guid>
{
    public string DisplayName { get; set; }
    public Guid ChannelId { get; set; }
    public bool IsMuted { get; set; }
    public bool IsDeafened { get; set; }
    public bool IsVideoEnabled { get; set; }
    public DateTimeOffset IsJoinedAt { get; set; }
    public DateTimeOffset LastActiveAt { get; set; }
    public bool IsActive { get; set; }
    
    public Channel Channel { get; set; }
    
    private Session(string displayName, Guid channelId)
    {
        Id = Guid.NewGuid();
        CreatedAt = DateTimeOffset.UtcNow;
        DisplayName = displayName;
        IsMuted = false;
        IsDeafened = false;
        IsVideoEnabled = false;
        IsJoinedAt = DateTimeOffset.UtcNow;
        LastActiveAt = DateTimeOffset.UtcNow;
        ChannelId = channelId;
        IsActive = true;
    }

    public static Session Create(string name, Guid channelId)
    {
        return new Session(name, channelId);
    }

    public bool Update(string? name)
    {
        if (name != null)
        {
            DisplayName = name;
        }

        return true;
    }
    
    public bool UpdateState(bool isMuted, bool isDeafened, bool isVideoEnabled)
    {
        IsMuted = isMuted;
        IsDeafened = isDeafened;
        IsVideoEnabled = isVideoEnabled;

        return true;
    }

    public bool Disconnect()
    {
        IsActive = false;

        return true;
    }
}