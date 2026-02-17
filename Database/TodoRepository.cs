using Domain;
using Domain.Repositories;
using Microsoft.EntityFrameworkCore;
using Shared;

namespace Database;

public class TodoRepository(MeliorDbContext dbContextFactory)
    : EFRepository<Todo, Guid, MeliorDbContext>(dbContextFactory), ITodoRepository
{
    public Task<List<Todo>> GetAllAsync(CancellationToken cancellationToken)
    {
        return Set.ToListAsync(cancellationToken);
    }
}