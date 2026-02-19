using System.Text.Json.Serialization;
using Shared.Redis;

namespace Database.Redis.Messages;

public class SdpAnswer : IRedisMessage
{
    [JsonPropertyName("channel_id")]
    public Guid ChannelId { get; set; }

    [JsonPropertyName("session_id")]
    public Guid SessionId { get; set; }

    [JsonPropertyName("sdp_type")]
    public string SdpType { get; set; }

    [JsonPropertyName("response")]
    public string Response { get; set; }
}