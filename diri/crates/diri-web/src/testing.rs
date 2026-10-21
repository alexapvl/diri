//! A fake Engine on a Unix socket: answers Hello itself, every other request
//! with `answer`, and pushes whatever events the test sends. A connection
//! that opens with a preview request is handed to the test as a raw stream,
//! acknowledged, so the test writes the binary frames itself.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use diri_client::DaemonClient;
use diri_platform::ipc::asynchronous::UnixStream;
use diri_proto::{ControlMessage, Method};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::sync::mpsc;

type Answer = Arc<dyn Fn(&str, Option<&Value>) -> Value + Send + Sync>;
/// The event feed, taken by whichever connection subscribes first.
type EventFeed = Arc<Mutex<Option<mpsc::Receiver<(String, Value)>>>>;

pub struct FakeEngine {
    pub client: Arc<DaemonClient>,
    /// Every request but Hello, in arrival order.
    pub calls: mpsc::Receiver<(String, Option<Value>)>,
    /// `(name, params)` pushed to the client as events.
    pub events: mpsc::Sender<(String, Value)>,
    /// `(session id, stream)` for every accepted preview, already answered
    /// with `PreviewReady`.
    pub previews: mpsc::Receiver<(String, UnixStream)>,
    task: tokio::task::JoinHandle<()>,
    _temp: tempfile::TempDir,
}

impl Drop for FakeEngine {
    fn drop(&mut self) {
        self.client.begin_shutdown();
        self.task.abort();
    }
}

pub async fn fake_engine(
    answer: impl Fn(&str, Option<&Value>) -> Value + Send + Sync + 'static,
) -> FakeEngine {
    let answer: Answer = Arc::new(answer);
    let temp = tempfile::tempdir_in("/tmp").unwrap();
    let socket = temp.path().join("engine.sock");
    let listener = diri_platform::ipc::asynchronous::UnixListener::bind(&socket).unwrap();
    let (calls_tx, calls) = mpsc::channel(64);
    let (events, events_rx) = mpsc::channel::<(String, Value)>(64);
    let events_rx = Arc::new(Mutex::new(Some(events_rx)));
    let (previews_tx, previews) = mpsc::channel(16);
    let task = tokio::spawn(async move {
        let mut connections = tokio::task::JoinSet::new();
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            connections.spawn(serve(
                stream,
                Arc::clone(&answer),
                calls_tx.clone(),
                Arc::clone(&events_rx),
                previews_tx.clone(),
            ));
        }
    });
    let client = Arc::new(DaemonClient::with_socket_path(socket));
    client.connect();
    client
        .wait_until_connected(Duration::from_secs(2))
        .await
        .unwrap();
    FakeEngine {
        client,
        calls,
        events,
        previews,
        task,
        _temp: temp,
    }
}

async fn serve(
    stream: UnixStream,
    answer: Answer,
    calls_tx: mpsc::Sender<(String, Option<Value>)>,
    events_rx: EventFeed,
    previews: mpsc::Sender<(String, UnixStream)>,
) {
    let mut reader = BufReader::new(stream);
    let mut first = String::new();
    if reader.read_line(&mut first).await.unwrap_or(0) == 0 {
        return;
    }
    let value: Value = serde_json::from_str(&first).unwrap_or(Value::Null);
    if let Some(id) = value.get("preview").and_then(Value::as_str) {
        let mut stream = reader.into_inner();
        let mut ready = serde_json::to_vec(&json!({"preview": id, "version": 1})).unwrap();
        ready.push(b'\n');
        stream.write_all(&ready).await.unwrap();
        let _ = previews.send((id.to_owned(), stream)).await;
        return;
    }
    let mut events_rx = events_rx.lock().unwrap().take();
    let (reader, mut writer) = reader.into_inner().into_split();
    let mut lines = BufReader::new(reader).lines();
    let mut pending = Some(first);
    let mut seq = 0;
    loop {
        let message = if let Some(line) = pending.take() {
            answer_line(&line, &answer, &calls_tx).await
        } else {
            tokio::select! {
                line = lines.next_line() => {
                    let Ok(Some(line)) = line else { return };
                    answer_line(&line, &answer, &calls_tx).await
                }
                Some((name, params)) = async {
                    match events_rx.as_mut() {
                        Some(events) => events.recv().await,
                        None => std::future::pending().await,
                    }
                } => {
                    seq += 1;
                    Some(ControlMessage::Event { name, seq, params })
                }
            }
        };
        let Some(message) = message else { continue };
        let mut bytes = serde_json::to_vec(&message).unwrap();
        bytes.push(b'\n');
        if writer.write_all(&bytes).await.is_err() {
            return;
        }
    }
}

async fn answer_line(
    line: &str,
    answer: &Answer,
    calls_tx: &mpsc::Sender<(String, Option<Value>)>,
) -> Option<ControlMessage> {
    let ControlMessage::Request { id, method, params } = serde_json::from_str(line.trim()).ok()?
    else {
        return None;
    };
    let result = if method == Method::HELLO {
        json!({"proto": diri_proto::WIRE_VERSION, "build":"test", "pid":1, "engineKind":diri_proto::RUST_ENGINE_KIND})
    } else {
        let result = answer(&method, params.as_ref());
        let _ = calls_tx.send((method, params)).await;
        result
    };
    Some(ControlMessage::Response {
        id,
        result: Ok(result),
    })
}
