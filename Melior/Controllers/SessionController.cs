using Domain.Services;
using Microsoft.AspNetCore.Mvc;

namespace MeliorApi.Controllers;

[ApiController]
[Route("/sessions")]
public class SessionController(ISessionService service) : ControllerBase
{
    [HttpGet]
    public async Task<IActionResult> GetAll(CancellationToken cancellationToken)
    {
        var items = await service.GetAllAsync(cancellationToken);
        return Ok(items);
    }
    
    [HttpGet("{id:guid}", Name = nameof(GetSessionsById))]
    public async Task<IActionResult> GetSessionsById([FromRoute] Guid id, CancellationToken cancellationToken)
    {
        var item = await service.Get(id, cancellationToken);
        if (item == null) return NotFound();
        return Ok(item);
    }

    public record ConnectSessionRequest(string DisplayName, Guid ChannelId);
    public record SessionUpdateRequest(bool IsMuted, bool IsDeafened, bool IsVideoEnabled);
    
    [HttpPost("connect")]
    public async Task<IActionResult> Connect([FromBody] ConnectSessionRequest request, CancellationToken cancellationToken)
    {
        var created = await service.CreateAsync(request.DisplayName, request.ChannelId, cancellationToken);
        return CreatedAtRoute(nameof(GetSessionsById), new { id = created.Id }, created);
    }

    [HttpPut("{id:guid}")]
    public async Task<IActionResult> Update([FromRoute] Guid id, [FromBody] SessionUpdateRequest request, CancellationToken cancellationToken)
    {
        var success = await service.UpdateAsync(id, request.IsMuted, request.IsDeafened, request.IsVideoEnabled, cancellationToken);
        if (!success) return NotFound();
        return NoContent();
    }

    [HttpPut("{id:guid}/disconnect")]
    public async Task<IActionResult> Delete([FromRoute] Guid id, CancellationToken cancellationToken)
    {
        var success = await service.DisconnectAsync(id, cancellationToken);
        if (!success) return NotFound();
        return NoContent();
    }
}