namespace Domain.Services;

public interface ISessionService
{
    Task<List<Session>> GetAllAsync(CancellationToken cancellationToken);
    Task<Session?> Get(Guid id, CancellationToken cancellationToken);
    Task<Session> CreateAsync(string name, Guid channelId, CancellationToken cancellationToken);
    Task<bool> UpdateAsync(Guid id, bool isMuted, bool isDeafened, bool isVideoEnabled, CancellationToken cancellationToken);
    Task<bool> DeleteAsync(Guid id, CancellationToken cancellationToken);
    Task<bool> DisconnectAsync(Guid id, CancellationToken cancellationToken);
}