using System.Text.Json.Serialization;
using Shared.Redis;

namespace Database.Redis.Messages;

[RedisChannel("ChannelConnectionChannel")]
public class ChannelMessage : IRedisMessage
{
    [JsonPropertyName("channel_id")]
    public Guid ChannelId { get; set; }

    [JsonPropertyName("session_id")]
    public Guid SessionId { get; set; }
}