using Domain;
using Microsoft.EntityFrameworkCore;

namespace Database;

public class MeliorDbContext : DbContext
{
    public const string DefaultSchema = "melior";
    public const string MigrationsSchema = "migrations";
    public MeliorDbContext(DbContextOptions<MeliorDbContext> options)
        : base(options)
    {
    }
    
    public DbSet<Todo> TodoItems { get; set; }
    public DbSet<Channel> Channels { get; set; }
    public DbSet<Session> Sessions { get; set; }
    
    /// <summary>
    /// Configure the model that was discovered by convention from the entity types exposed in <see cref="DbSet{TEntity}"/> properties on this context.
    /// </summary>
    /// <param name="modelBuilder"></param>
    protected override void OnModelCreating(ModelBuilder modelBuilder)
    {
        modelBuilder.HasDefaultSchema(DefaultSchema);
        modelBuilder.ApplyConfigurationsFromAssembly(typeof(MeliorDbContext).Assembly);
        
        modelBuilder.Entity<Channel>(channelBuilder =>
        {
            channelBuilder
                .HasMany(c => c.Sessions)
                .WithOne(s => s.Channel)
                .HasForeignKey(s => s.ChannelId)
                .OnDelete(DeleteBehavior.Cascade);
        });
    }

}