using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Options;
using StackExchange.Redis;

namespace Shared.Redis;

public static class RedisDependencyInstaller
{
    public static IServiceCollection AddRedisInfrastructure(
        this IServiceCollection services,
        IConfiguration configuration)
    {
        // Define the section
        var redisSection = configuration.GetSection(RedisOptions.SectionName);

        // Correct binding: Pass the section directly to the Configure method
        // This requires the Microsoft.Extensions.Options.ConfigurationExtensions NuGet package
        services.Configure<RedisOptions>(redisSection);

        // 2. Register ConnectionMultiplexer as a Singleton
        services.AddSingleton<IConnectionMultiplexer>(sp =>
        {
            var options = sp.GetRequiredService<IOptions<RedisOptions>>().Value;
            var configurationOptions = ConfigurationOptions.Parse(options.ConnectionString);

            // Scientific default: Do not crash on startup if Redis is down
            configurationOptions.AbortOnConnectFail = false;

            return ConnectionMultiplexer.Connect(configurationOptions);
        });

        // 3. Register IDatabase as Transient (Cheap lookup from Multiplexer)
        services.AddTransient<IDatabase>(sp =>
        {
            var multiplexer = sp.GetRequiredService<IConnectionMultiplexer>();
            var options = sp.GetRequiredService<IOptions<RedisOptions>>().Value;
            return multiplexer.GetDatabase(options.DefaultDatabase);
        });

        services.AddTransient<ISubscriber>(sp =>
        {
            var multiplexer = sp.GetRequiredService<IConnectionMultiplexer>();
            return multiplexer.GetSubscriber();
        });

        services.AddScoped<IPublishingService, PublishingService>();

        return services;
    }
}