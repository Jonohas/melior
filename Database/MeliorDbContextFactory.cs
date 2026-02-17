using System;
using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Diagnostics;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.Logging;
using Shared;

namespace Database;

public class MeliorDbContextFactory : ICustomDbContextFactory<MeliorDbContext>
{
  private readonly string _connectionString;
  private readonly bool _enableSensitiveDataLogging;
  private MeliorDbContext? _dbContext;

  public MeliorDbContextFactory(string connectionString, IConfiguration configuration)
  {
    _connectionString = connectionString;
    var enableQueryLoggingString = configuration["Debugging:EnableQueryLogging"];
    if (!bool.TryParse(enableQueryLoggingString, out _enableSensitiveDataLogging))
    {
      throw new ArgumentException("EnableQueryLogging configuration is not valid");
    }

    _dbContext = CreateDbContext();
  }

  public MeliorDbContext CreateDbContext()
  {
    if (_dbContext is null)
      _dbContext = CreateNew();

    return _dbContext;
  }

  public MeliorDbContext CreateNew()
  {
    var dbOptions = new DbContextOptionsBuilder<MeliorDbContext>()
      .UseNpgsql(_connectionString,
        options =>
        {
          options.MigrationsAssembly(typeof(MeliorDbContextFactory).Assembly.FullName);
          options.MigrationsHistoryTable(MeliorDbContext.MigrationsSchema, MeliorDbContext.MigrationsSchema);
        })
      .UseSnakeCaseNamingConvention();

    if (_enableSensitiveDataLogging)
    {
      dbOptions
        .EnableSensitiveDataLogging()
        .LogTo(Console.WriteLine, (eventId, logLevel) => logLevel >= LogLevel.Information
                                                         || eventId == RelationalEventId.DataReaderDisposing);
    }

    _dbContext = new MeliorDbContext(dbOptions.Options);

    return _dbContext;
  }

  public void Dispose()
  {
    _dbContext?.Dispose();
  }

  public async ValueTask DisposeAsync()
  {
    if (_dbContext != null)
      await _dbContext.DisposeAsync();
  }
}