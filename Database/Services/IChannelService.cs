using Database.Redis.Messages;
using Domain;
using JV.ResultUtilities;

namespace Domain.Services;

public interface IChannelService
{
    Task<List<Channel>> GetAllAsync(CancellationToken cancellationToken);
    Task<Channel?> Get(Guid id, CancellationToken cancellationToken);
    Task<Channel> CreateAsync(string name, CancellationToken cancellationToken);
    Task<Result<SdpAnswer>> ConnectAsync(Guid id, string displayName, string sdpType, string sdp, CancellationToken cancellationToken);
    Task<bool> DisconnectAsync(Guid id, Guid sessionId, CancellationToken cancellationToken);
    Task<bool> UpdateAsync(Guid id, string? name, CancellationToken cancellationToken);
    Task<bool> DeleteAsync(Guid id, CancellationToken cancellationToken);
}
