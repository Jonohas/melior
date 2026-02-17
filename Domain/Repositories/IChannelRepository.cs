using Shared;

namespace Domain.Repositories;

public interface IChannelRepository : IRepository<Channel, Guid>
{
    Task<List<Channel>> GetAllAsync(CancellationToken cancellationToken);
}