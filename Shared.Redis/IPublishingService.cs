namespace Shared.Redis;

public interface IPublishingService
{
    Task PublishAsync<T>(T message) where T : class;
}