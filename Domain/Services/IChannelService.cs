using Domain;

namespace Domain.Services;

public interface IChannelService
{
    Task<List<Channel>> GetAllAsync(CancellationToken cancellationToken);
    Task<Channel?> Get(Guid id, CancellationToken cancellationToken);
    Task<Channel> CreateAsync(string name, CancellationToken cancellationToken);
    Task<bool> UpdateAsync(Guid id, string? name, CancellationToken cancellationToken);
    Task<bool> DeleteAsync(Guid id, CancellationToken cancellationToken);
}
