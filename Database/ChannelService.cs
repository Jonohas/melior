using System.Text.Json;
using Database.Redis.Messages;
using Domain;
using Domain.Repositories;
using Domain.Services;
using JV.ResultUtilities;
using JV.ResultUtilities.ValidationMessage;
using Shared.Redis;
using StackExchange.Redis;

namespace Database;

public class ChannelService : IChannelService
{
    private readonly IChannelRepository _repo;
    private readonly ISessionRepository _sessionRepository;
    private readonly IPublishingService _publishingService;
    private readonly ISubscriber _subscriber;

    public ChannelService(IChannelRepository repo, ISessionRepository sessionRepository, IPublishingService publishingService, ISubscriber subscriber)
    {
        _repo = repo;
        _sessionRepository = sessionRepository;
        _publishingService = publishingService;
        _subscriber = subscriber;
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

    public async Task<Result<SdpAnswer>> ConnectAsync(Guid id, string displayName, string sdpType, string sdp, CancellationToken cancellationToken)
    {
        var channel = await _repo.Get(id, cancellationToken);
        if (channel == null) return Result.Error(ValidationKeyDefinition.Create("channel-not-found", "Channel was not found"));
        var session = Session.Create(displayName, id);
        await _sessionRepository.SaveAsync(session, cancellationToken);

        var tcs = new TaskCompletionSource<string?>(TaskCreationOptions.RunContinuationsAsynchronously);

        // 2. Subscribe to the unique response channel first
        var responseChannel = $"sfu_offers:{session.Id}";

        await _subscriber.SubscribeAsync(responseChannel, (_, message) =>
        {
            // 3. When message arrives, complete the task
            tcs.TrySetResult(message.ToString());
        });

        // here we want to wait for the response. The message will be published on the channel "sfp_offers:session_id"
        try
        {
            // 4. Publish the offer to the Rust SFU
            await _publishingService.PublishAsync(new SdpOffer
            {
                ChannelId = id,
                SessionId = session.Id,
                Sdp = sdp,
                SdpType = sdpType,
            });

            // 5. Wait for the Rust server to send the Answer (with a timeout)
            var timeoutTask = Task.Delay(TimeSpan.FromSeconds(10), cancellationToken);
            var completedTask = await Task.WhenAny(tcs.Task, timeoutTask);

            if (completedTask == timeoutTask)
            {
                throw new TimeoutException("Rust SFU did not respond within 10 seconds.");
            }

            var responseString = await tcs.Task;

            if (responseString == null)
            {
                return Result.Error(ValidationKeyDefinition.Create("invalid-sdp-offer", "The sdp answer was not in the correct format."));
            }

            var response = JsonSerializer.Deserialize<SdpAnswer>(responseString);

            return response == null ? Result.Error(ValidationKeyDefinition.Create("invalid-sdp-offer", "The sdp answer was not in the correct format.")) : Result.Ok(response); // This is the Answer SDP from Rust
        }
        catch (JsonException e)
        {
            return Result.Error(ValidationKeyDefinition.Create("invalid-sdp-offer", "The sdp answer was not in the correct format."));
        }
        catch (TimeoutException e)
        {
            return Result.Error(ValidationKeyDefinition.Create("sdp-answer-timed-out", "The sdp answer timed out."));
        }
        finally
        {
            // Always unsubscribe to clean up the Redis connection
            await _subscriber.UnsubscribeAsync(responseChannel);
        }
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
