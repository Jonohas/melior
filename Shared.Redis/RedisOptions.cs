namespace Shared.Redis;

public class RedisOptions
{
    public const string SectionName = "Redis";
    public const string ChannelConnectionChannel = "ChannelConnectionChannel";
    public string ConnectionString { get; set; } = string.Empty;
    public string InstanceName { get; set; } = string.Empty;
    public int DefaultDatabase { get; set; } = 0;
}