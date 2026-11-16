/** `GET /api/rooms/{id}` answered 404: nobody has joined or created the room yet. */
export class RoomNotFoundError extends Error {
    constructor() {
        super('Room not found.');
        this.name = 'RoomNotFoundError';
    }
}

/** The server rejected the request as invalid (400), for example a bad username. */
export class RoomRejectedError extends Error {
    constructor(message = 'The room settings were rejected.') {
        super(message);
        this.name = 'RoomRejectedError';
    }
}
