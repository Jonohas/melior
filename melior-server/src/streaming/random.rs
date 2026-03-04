struct Channel; // has peer

struct Peer; // Has its own incoming and outgoing tracks

struct Track; // Has the actual track data, etc

enum TrackType {
    Camera,
    Audio,
    ScreenShare,
}
