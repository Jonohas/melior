using Domain;
using Domain.Repositories;
using Microsoft.EntityFrameworkCore;
using Shared;

namespace Database;

public class ChannelRepository(MeliorDbContext dbContextFactory)
    : EFRepository<Channel, Guid, MeliorDbContext>(dbContextFactory), IChannelRepository
{
    public Task<List<Channel>> GetAllAsync(CancellationToken cancellationToken)
    {
        return Set.ToListAsync(cancellationToken);
    }
}