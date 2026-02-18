using System.Text.Json.Serialization;
using Shared.Redis;

namespace Database.Redis.Messages;

[RedisChannel("sfu_offers")]
public class SdpOffer : IRedisMessage
{
    [JsonPropertyName("channel_id")]
    public Guid ChannelId { get; set; }

    [JsonPropertyName("session_id")]
    public Guid SessionId { get; set; }

    [JsonPropertyName("sdp_type")]
    public string SdpType { get; set; }

    [JsonPropertyName("sdp")]
    public string Sdp { get; set; }
}