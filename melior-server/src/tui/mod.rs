use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
    Frame, Terminal,
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::sync::broadcast;
use tokio::sync::Mutex;

pub const SOCKET_PATH: &str = "/tmp/melior.sock";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TuiEvent {
    WsConnect {
        session_id: String,
        channel_id: String,
    },
    WsDisconnect {
        session_id: String,
    },
    IceStateChange {
        session_id: String,
        state: String,
    },
    TrackAdded {
        session_id: String,
        codec: String,
        track_id: String,
    },
    TrackRemoved {
        session_id: String,
        track_id: String,
    },
    Log {
        level: String,
        message: String,
        timestamp: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackInfo {
    pub session_id: String,
    pub codec: String,
    pub track_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub tracks: Vec<TrackInfo>,
    pub events: Vec<TuiEvent>,
    pub logs: Vec<TuiEvent>,
}

pub struct EventEmitter {
    tx: broadcast::Sender<TuiEvent>,
    logs_tx: broadcast::Sender<TuiEvent>,
}

impl EventEmitter {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1000);
        let (logs_tx, _) = broadcast::channel(1000);
        Self { tx, logs_tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<TuiEvent> {
        self.tx.subscribe()
    }

    pub fn subscribe_logs(&self) -> broadcast::Receiver<TuiEvent> {
        self.logs_tx.subscribe()
    }

    pub fn emit(&self, event: TuiEvent) {
        let _ = self.tx.send(event.clone());
        if matches!(event, TuiEvent::Log { .. }) {
            let _ = self.logs_tx.send(event);
        }
    }

    pub fn emit_log(&self, level: &str, message: &str) {
        self.emit(TuiEvent::Log {
            level: level.to_string(),
            message: message.to_string(),
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
        });
    }
}

impl Default for EventEmitter {
    fn default() -> Self {
        Self::new()
    }
}

pub struct TuiState {
    pub events: VecDeque<TuiEvent>,
    pub logs: VecDeque<TuiEvent>,
    pub tracks: Vec<TrackInfo>,
    pub selected_track_index: usize,
    pub connection_count: usize,
}

impl Default for TuiState {
    fn default() -> Self {
        Self {
            events: VecDeque::new(),
            logs: VecDeque::new(),
            tracks: Vec::new(),
            selected_track_index: 0,
            connection_count: 0,
        }
    }
}

pub fn run_tui(
    mut event_rx: broadcast::Receiver<TuiEvent>,
    mut logs_rx: broadcast::Receiver<TuiEvent>,
) {
    let stdout = std::io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut state = TuiState::default();

    terminal.clear().unwrap();

    loop {
        while let Ok(event) = event_rx.try_recv() {
            match &event {
                TuiEvent::WsConnect { .. } => {
                    state.connection_count += 1;
                }
                TuiEvent::WsDisconnect { .. } => {
                    state.connection_count = state.connection_count.saturating_sub(1);
                }
                TuiEvent::TrackAdded {
                    session_id,
                    codec,
                    track_id,
                } => {
                    state.tracks.push(TrackInfo {
                        session_id: session_id.clone(),
                        codec: codec.clone(),
                        track_id: track_id.clone(),
                    });
                }
                TuiEvent::TrackRemoved { track_id, .. } => {
                    state.tracks.retain(|t| &t.track_id != track_id);
                }
                _ => {}
            }

            if state.events.len() > 100 {
                state.events.pop_front();
            }
            state.events.push_back(event);
        }

        while let Ok(log) = logs_rx.try_recv() {
            if state.logs.len() > 100 {
                state.logs.pop_front();
            }
            state.logs.push_back(log);
        }

        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
                .split(f.size());

            let right_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(chunks[1]);

            render_tracks_panel(f, chunks[0], &mut state);
            render_events_panel(f, right_chunks[0], &state);
            render_logs_panel(f, right_chunks[1], &state);
        });

        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

fn render_tracks_panel(f: &mut Frame, area: Rect, state: &mut TuiState) {
    let title = format!("Tracks ({})", state.connection_count);

    let tracks: Vec<ListItem> = state
        .tracks
        .iter()
        .enumerate()
        .map(|(i, track)| {
            let style = if i == state.selected_track_index {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(vec![
                Span::raw(format!("{}: ", track.session_id)),
                Span::raw(format!("{} ", track.codec)),
            ]))
            .style(style)
        })
        .collect();

    let list = List::new(tracks)
        .block(Block::default().title(title).borders(Borders::ALL))
        .style(Style::default().fg(Color::White));

    f.render_widget(list, area);
}

fn render_events_panel(f: &mut Frame, area: Rect, state: &TuiState) {
    let events: Vec<ListItem> = state
        .events
        .iter()
        .rev()
        .filter(|e| !matches!(e, TuiEvent::Log { .. }))
        .take(20)
        .map(|event| {
            let (color, text) = match event {
                TuiEvent::WsConnect {
                    session_id,
                    channel_id,
                } => (
                    Color::Green,
                    format!("[WS] Connected: {} ({})", session_id, channel_id),
                ),
                TuiEvent::WsDisconnect { session_id } => {
                    (Color::Red, format!("[WS] Disconnected: {}", session_id))
                }
                TuiEvent::IceStateChange {
                    session_id,
                    state: ice_state,
                } => (Color::Blue, format!("[ICE] {}: {}", session_id, ice_state)),
                TuiEvent::TrackAdded {
                    session_id,
                    codec,
                    track_id,
                } => (
                    Color::Cyan,
                    format!("[Track] {} added {} ({})", session_id, codec, track_id),
                ),
                TuiEvent::TrackRemoved {
                    session_id,
                    track_id,
                } => (
                    Color::LightRed,
                    format!("[Track] {} removed {}", session_id, track_id),
                ),
                TuiEvent::Log { .. } => return None,
            };
            Some(ListItem::new(
                Line::from(Span::raw(text)).style(Style::default().fg(color)),
            ))
        })
        .filter_map(|e| e)
        .collect();

    let list = List::new(events)
        .block(Block::default().title("Events").borders(Borders::ALL))
        .style(Style::default().fg(Color::White));

    f.render_widget(list, area);
}

fn render_logs_panel(f: &mut Frame, area: Rect, state: &TuiState) {
    let logs: Vec<ListItem> = state
        .logs
        .iter()
        .rev()
        .take(20)
        .filter_map(|event| {
            if let TuiEvent::Log {
                level,
                message,
                timestamp,
            } = event
            {
                let color = match level.as_str() {
                    "ERROR" => Color::Red,
                    "WARN" => Color::Yellow,
                    "INFO" => Color::Blue,
                    _ => Color::White,
                };
                Some(ListItem::new(
                    Line::from(Span::raw(format!("[{}] {}: {}", timestamp, level, message)))
                        .style(Style::default().fg(color)),
                ))
            } else {
                None
            }
        })
        .collect();

    let list = List::new(logs)
        .block(Block::default().title("Logs").borders(Borders::ALL))
        .style(Style::default().fg(Color::White));

    f.render_widget(list, area);
}

pub fn start_socket_server(emitter: Arc<Mutex<EventEmitter>>) {
    tokio::spawn(async move {
        let emitter_clone = emitter.clone();
        let subscriber = {
            let emitter_guard = emitter.lock().await;
            emitter_guard.subscribe()
        };

        if std::path::Path::new(SOCKET_PATH).exists() {
            std::fs::remove_file(SOCKET_PATH).ok();
        }

        let listener = tokio::net::UnixListener::bind(SOCKET_PATH).unwrap();

        {
            let em = emitter_clone.lock().await;
            em.emit_log("INFO", &format!("TUI socket listening on {}", SOCKET_PATH));
        }

        loop {
            if let Ok((stream, _)) = listener.accept().await {
                let subscriber = subscriber.resubscribe();
                tokio::spawn(handle_tui_connection(stream, subscriber));
            }
        }
    });
}

pub async fn run_embedded_tui(emitter: Arc<Mutex<EventEmitter>>) {
    let (event_tx, event_rx) = broadcast::channel::<TuiEvent>(1000);
    let (logs_tx, logs_rx) = broadcast::channel::<TuiEvent>(1000);

    let emitter_clone = emitter.clone();
    tokio::spawn(async move {
        let mut rx = emitter_clone.lock().await.subscribe();
        let mut logs_rx = emitter_clone.lock().await.subscribe_logs();
        loop {
            tokio::select! {
                event = rx.recv() => {
                    if let Ok(e) = event {
                        let _ = event_tx.send(e);
                    }
                }
                log = logs_rx.recv() => {
                    if let Ok(l) = log {
                        let _ = logs_tx.send(l);
                    }
                }
            }
        }
    });

    run_tui(event_rx, logs_rx);
}

async fn handle_tui_connection(
    mut stream: UnixStream,
    mut event_rx: broadcast::Receiver<TuiEvent>,
) {
    let mut buffer = [0u8; 4096];

    loop {
        tokio::select! {
            result = stream.read(&mut buffer) => {
                if let Ok(0) = result {
                    break;
                }
            }
            event = event_rx.recv() => {
                if let Ok(event) = event {
                    let json = serde_json::to_string(&event).unwrap();
                    let response = format!("{}\n", json);
                    let _ = stream.write_all(response.as_bytes()).await;
                }
            }
        }
    }
}

pub fn update_state_from_snapshot(state: &mut TuiState, snapshot: StateSnapshot) {
    state.tracks = snapshot.tracks;
    for event in snapshot.events {
        if state.events.len() > 100 {
            state.events.pop_front();
        }
        state.events.push_back(event);
    }
    for log in snapshot.logs {
        if state.logs.len() > 100 {
            state.logs.pop_front();
        }
        state.logs.push_back(log);
    }
}

#[cfg(test)]
mod tests {
    use crate::tui::{EventEmitter, TuiEvent, SOCKET_PATH};
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixStream;
    use tokio::sync::Mutex;

    fn create_emitter() -> Arc<Mutex<EventEmitter>> {
        Arc::new(Mutex::new(EventEmitter::new()))
    }

    #[tokio::test]
    async fn test_event_emitter_emit_and_subscribe() {
        let emitter = create_emitter();
        let mut rx = emitter.lock().await.subscribe();

        emitter.lock().await.emit(TuiEvent::WsConnect {
            session_id: "test-session".to_string(),
            channel_id: "test-channel".to_string(),
        });

        let event = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();

        match event {
            TuiEvent::WsConnect {
                session_id,
                channel_id,
            } => {
                assert_eq!(session_id, "test-session");
                assert_eq!(channel_id, "test-channel");
            }
            _ => panic!("Expected WsConnect event"),
        }
    }

    #[tokio::test]
    async fn test_event_emitter_log_separate_channel() {
        let emitter = create_emitter();
        let mut logs_rx = emitter.lock().await.subscribe_logs();

        emitter.lock().await.emit(TuiEvent::Log {
            level: "INFO".to_string(),
            message: "Test log message".to_string(),
            timestamp: "12:00:00".to_string(),
        });

        let event = tokio::time::timeout(std::time::Duration::from_secs(1), logs_rx.recv())
            .await
            .unwrap()
            .unwrap();

        match event {
            TuiEvent::Log {
                level,
                message,
                timestamp,
            } => {
                assert_eq!(level, "INFO");
                assert_eq!(message, "Test log message");
                assert_eq!(timestamp, "12:00:00");
            }
            _ => panic!("Expected Log event"),
        }
    }

    #[tokio::test]
    async fn test_event_types_serialization() {
        let event = TuiEvent::WsConnect {
            session_id: "session1".to_string(),
            channel_id: "channel1".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("session1"));
        assert!(json.contains("channel1"));

        let event = TuiEvent::WsDisconnect {
            session_id: "session1".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("session1"));

        let event = TuiEvent::IceStateChange {
            session_id: "session1".to_string(),
            state: "connected".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("session1"));
        assert!(json.contains("connected"));

        let event = TuiEvent::TrackAdded {
            session_id: "session1".to_string(),
            codec: "VP8".to_string(),
            track_id: "track1".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("session1"));
        assert!(json.contains("VP8"));
        assert!(json.contains("track1"));

        let event = TuiEvent::Log {
            level: "ERROR".to_string(),
            message: "Error occurred".to_string(),
            timestamp: "12:00:00".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("ERROR"));
        assert!(json.contains("Error occurred"));
    }

    #[tokio::test]
    async fn test_unix_socket_connection() {
        use tokio::net::UnixListener;

        let socket_path = std::path::Path::new(SOCKET_PATH);
        if socket_path.exists() {
            std::fs::remove_file(socket_path).ok();
        }

        let listener = UnixListener::bind(SOCKET_PATH).unwrap();

        let handle = tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0u8; 1024];
                let n = stream.read(&mut buf).await.unwrap();
                let msg = String::from_utf8_lossy(&buf[..n]);
                assert!(msg.contains("WsConnect"));
                assert!(msg.contains("test-session"));

                stream.write_all(b"ok\n").await.unwrap();
            }
        });

        let mut client = UnixStream::connect(SOCKET_PATH).await.unwrap();
        let event = TuiEvent::WsConnect {
            session_id: "test-session".to_string(),
            channel_id: "test-channel".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        client
            .write_all(format!("{}\n", json).as_bytes())
            .await
            .unwrap();

        let mut buf = [0u8; 1024];
        let n = client.read(&mut buf).await.unwrap();
        let response = String::from_utf8_lossy(&buf[..n]);
        assert_eq!(response.trim(), "ok");

        handle.await.unwrap();

        std::fs::remove_file(SOCKET_PATH).ok();
    }

    #[tokio::test]
    async fn test_multiple_events_sequence() {
        let emitter = create_emitter();
        let mut rx = emitter.lock().await.subscribe();

        emitter.lock().await.emit(TuiEvent::WsConnect {
            session_id: "session1".to_string(),
            channel_id: "channel1".to_string(),
        });

        emitter.lock().await.emit(TuiEvent::TrackAdded {
            session_id: "session1".to_string(),
            codec: "VP8".to_string(),
            track_id: "track1".to_string(),
        });

        emitter.lock().await.emit(TuiEvent::IceStateChange {
            session_id: "session1".to_string(),
            state: "checking".to_string(),
        });

        let event1 = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(event1, TuiEvent::WsConnect { .. }));

        let event2 = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(event2, TuiEvent::TrackAdded { .. }));

        let event3 = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(event3, TuiEvent::IceStateChange { .. }));
    }
}
