namespace Domain;

public class Channel : Entity<Guid>
{
    public string Name { get; set; }
    
    private Channel(string name, bool done)
    {
        Id = Guid.NewGuid();
        Name = name;
        CreatedAt = DateTimeOffset.UtcNow;
    }

    public static Channel Create(string name, bool done)
    {
        return new Channel(name, done);
    }

    public bool Update(string? name, bool? done)
    {
        if (name != null)
        {
            Name = name;
        }

        return true;
    }
}