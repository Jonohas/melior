namespace Domain;

public class Channel : Entity<Guid>
{
    public string Name { get; set; }
    
    public IEnumerable<Session> Sessions { get; set; }
    
    private Channel(string name)
    {
        Id = Guid.NewGuid();
        Name = name;
        CreatedAt = DateTimeOffset.UtcNow;
    }

    public static Channel Create(string name)
    {
        return new Channel(name);
    }

    public bool Update(string? name)
    {
        if (name != null)
        {
            Name = name;
        }

        return true;
    }
}