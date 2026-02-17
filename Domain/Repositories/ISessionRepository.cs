using Shared;

namespace Domain.Repositories;

public interface ISessionRepository : IRepository<Session, Guid>
{
    Task<List<Session>> GetAllAsync(CancellationToken cancellationToken);
}