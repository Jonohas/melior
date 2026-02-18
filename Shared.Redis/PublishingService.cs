using System.Reflection;
using System.Text.Json;
using StackExchange.Redis;

namespace Shared.Redis;

public class PublishingService(ISubscriber subscriber) : IPublishingService
{
    public async Task PublishAsync<T>(T message) where T : class
    {
        // 1. Resolve channel name from Attribute
        var attribute = typeof(T).GetCustomAttribute<RedisChannelAttribute>();

        if (attribute == null)
        {
            throw new InvalidOperationException(
                $"Type {typeof(T).Name} is missing RedisChannelAttribute.");
        }

        // 2. Serialize
        string payload = JsonSerializer.Serialize(message);

        // 3. Publish
        await subscriber.PublishAsync(
            RedisChannel.Literal(attribute.ChannelName),
            payload
        );
    }
}