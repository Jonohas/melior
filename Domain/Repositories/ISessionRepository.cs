using Shared;

namespace Domain.Repositories;

public interface ISessionRepository : IRepository<Session, Guid>
{
    Task<List<Session>> GetAllAsync(CancellationToken cancellationToken);
    Task<Session?> GetWithIncludesAsync(Guid id, CancellationToken cancellationToken);
}