using Domain.Services;
using Microsoft.AspNetCore.Mvc;

namespace MeliorApi.Controllers;

[ApiController]
[Route("/channels")]
public class ChannelController(IChannelService service, ISessionService sessionService) : ControllerBase
{
    [HttpGet]
    public async Task<IActionResult> GetAll(CancellationToken cancellationToken)
    {
        var items = await service.GetAllAsync(cancellationToken);
        return Ok(items);
    }

    [HttpGet("{id:guid}", Name = nameof(GetChannelsById))]
    public async Task<IActionResult> GetChannelsById([FromRoute] Guid id, CancellationToken cancellationToken)
    {
        var item = await service.Get(id, cancellationToken);
        if (item == null) return NotFound();
        return Ok(item);
    }

    public record ChannelCreateRequest(string Name);

    public record ChannelUpdateRequest(string? Name);

    [HttpPost]
    public async Task<IActionResult> Create([FromBody] ChannelCreateRequest request, CancellationToken cancellationToken)
    {
        var created = await service.CreateAsync(request.Name, cancellationToken);
        return CreatedAtRoute(nameof(GetChannelsById), new { id = created.Id }, created);
    }

    public record ConnectSessionRequest(string DisplayName, string SdpType, string Sdp);

    public record DisconnectSessionRequest(Guid SessionId);
    
    [HttpPost("{id:guid}/connect")]
    public async Task<IActionResult> Connect([FromRoute] Guid id, [FromBody] ConnectSessionRequest request, CancellationToken cancellationToken)
    {
        var success = await service.ConnectAsync(id, request.DisplayName, request.SdpType, request.Sdp, cancellationToken);
        if (!success) return NotFound();
        return NoContent();
    }
    
    [HttpPost("{id:guid}/disconnect")]
    public async Task<IActionResult> Disconnect([FromRoute] Guid id, [FromBody] DisconnectSessionRequest request, CancellationToken cancellationToken)
    {
        var success = await service.DisconnectAsync(id, request.SessionId, cancellationToken);
        if (!success) return NotFound();
        return NoContent();
    }
    
    [HttpPut("{id:guid}")]
    public async Task<IActionResult> Update([FromRoute] Guid id, [FromBody] ChannelUpdateRequest request, CancellationToken cancellationToken)
    {
        var success = await service.UpdateAsync(id, request.Name, cancellationToken);
        if (!success) return NotFound();
        return NoContent();
    }

    [HttpDelete("{id:guid}")]
    public async Task<IActionResult> Delete([FromRoute] Guid id, CancellationToken cancellationToken)
    {
        var success = await service.DeleteAsync(id, cancellationToken);
        if (!success) return NotFound();
        return NoContent();
    }
}