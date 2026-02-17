using Domain.Repositories;

namespace Domain.Services;

public class SessionService : ISessionService
{
    private readonly ISessionRepository _repo;

    public SessionService(ISessionRepository repo)
    {
        _repo = repo;
    }

    public Task<List<Session>> GetAllAsync(CancellationToken cancellationToken = default)
    {
        return _repo.GetAllAsync(cancellationToken);
    }

    public Task<Session?> Get(Guid id, CancellationToken cancellationToken = default)
    {
        return _repo.Get(id, cancellationToken);
    }

    public async Task<Session> CreateAsync(string name, Guid channelId, CancellationToken cancellationToken = default)
    {
        var session = Session.Create(name, channelId);
        await _repo.SaveAsync(session, cancellationToken);
        return session;
    }

    public async Task<bool> UpdateAsync(Guid id, bool isMuted, bool isDeafened, bool isVideoEnabled, CancellationToken cancellationToken)
    {
        var session = await _repo.Get(id, cancellationToken);
        if (session == null) return false;
        var updated = session.UpdateState(isMuted, isDeafened, isVideoEnabled);
        if (updated)
        {
            await _repo.SaveAsync(session, cancellationToken);
        }
        return updated;
    }

    public async Task<bool> DeleteAsync(Guid id, CancellationToken cancellationToken = default)
    {
        var session = await _repo.Get(id, cancellationToken);
        if (session == null) return false;
        _repo.Delete(session);
        return true;
    }

    public async Task<bool> DisconnectAsync(Guid id, CancellationToken cancellationToken)
    {
        var session = await _repo.Get(id, cancellationToken);
        if (session == null) return false;
        var disconnected = session.Disconnect();
        if (disconnected)
        {
            await _repo.SaveAsync(session, cancellationToken);
        }
        return true;
    }
}