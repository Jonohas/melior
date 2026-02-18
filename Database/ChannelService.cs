using Database.Redis.Messages;
using Domain;
using Domain.Repositories;
using Domain.Services;
using Shared.Redis;

namespace Database;

public class ChannelService : IChannelService
{
    private readonly IChannelRepository _repo;
    private readonly ISessionRepository _sessionRepository;
    private readonly IPublishingService _publishingService;

    public ChannelService(IChannelRepository repo, ISessionRepository sessionRepository, IPublishingService publishingService)
    {
        _repo = repo;
        _sessionRepository = sessionRepository;
        _publishingService = publishingService;
    }

    public Task<List<Channel>> GetAllAsync(CancellationToken cancellationToken = default)
    {
        return _repo.GetAllAsync(cancellationToken);
    }

    public Task<Channel?> Get(Guid id, CancellationToken cancellationToken = default)
    {
        return _repo.Get(id, cancellationToken);
    }

    public async Task<Channel> CreateAsync(string name, CancellationToken cancellationToken = default)
    {
        var channel = Channel.Create(name);
        await _repo.SaveAsync(channel, cancellationToken);
        return channel;
    }

    public async Task<bool> ConnectAsync(Guid id, string displayName, CancellationToken cancellationToken)
    {
        var channel = await _repo.Get(id, cancellationToken);
        if (channel == null) return false;
        var session = Session.Create(displayName, id);
        await _sessionRepository.SaveAsync(session, cancellationToken);

        await _publishingService.PublishAsync(new ChannelMessage
        {
            ChannelId = id,
            SessionId = session.Id,
        });
        return true;
    }

    public async Task<bool> DisconnectAsync(Guid id, Guid sessionId, CancellationToken cancellationToken)
    {
        var session = await _sessionRepository.GetWithIncludesAsync(sessionId, cancellationToken);
        if (session == null) return false;
        if (session.Channel.Id != id) return false;

        return true;
    }

    public async Task<bool> UpdateAsync(Guid id, string? name, CancellationToken cancellationToken = default)
    {
        var channel = await _repo.Get(id, cancellationToken);
        if (channel == null) return false;
        var updated = channel.Update(name);
        if (updated)
        {
            await _repo.SaveAsync(channel, cancellationToken);
        }

        return updated;
    }

    public async Task<bool> DeleteAsync(Guid id, CancellationToken cancellationToken = default)
    {
        var Channel = await _repo.Get(id, cancellationToken);
        if (Channel == null) return false;
        _repo.Delete(Channel);
        return true;
    }
}
