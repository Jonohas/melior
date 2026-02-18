namespace Shared.Redis;

[AttributeUsage(AttributeTargets.Class | AttributeTargets.Struct)]
public class RedisChannelAttribute(string channelName) : Attribute
{
    public string ChannelName { get; } = channelName;
}