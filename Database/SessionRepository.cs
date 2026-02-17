using Domain;
using Domain.Repositories;
using Microsoft.EntityFrameworkCore;
using Shared;

namespace Database;

public class SessionRepository(MeliorDbContext dbContextFactory) : EFRepository<Session, Guid, MeliorDbContext>(dbContextFactory), ISessionRepository
{
    public Task<List<Session>> GetAllAsync(CancellationToken cancellationToken)
    {
        return Set.ToListAsync(cancellationToken);
    }

    public Task<Session?> GetWithIncludesAsync(Guid id, CancellationToken cancellationToken)
    {
        return Set.Include(s => s.Channel).FirstOrDefaultAsync(s => s.Id == id, cancellationToken);
    }
}