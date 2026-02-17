using Domain.Repositories;

namespace Domain.Services;

public class ChannelService : IChannelService
{
    private readonly IChannelRepository _repo;

    public ChannelService(IChannelRepository repo)
    {
        _repo = repo;
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
