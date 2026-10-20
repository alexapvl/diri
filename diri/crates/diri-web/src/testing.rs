//! A fake Engine on a Unix socket: answers Hello itself, every other request
//! with `answer`, and pushes whatever events the test sends.

use std::sync::Arc;
use std::time::Duration;

use diri_client::DaemonClient;
use diri_proto::{ControlMessage, Method};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::sync::mpsc;

pub struct FakeEngine {
    pub client: Arc<DaemonClient>,
    /// Every request but Hello, in arrival order.
    pub calls: mpsc::Receiver<(String, Option<Value>)>,
    /// `(name, params)` pushed to the client as events.
    pub events: mpsc::Sender<(String, Value)>,
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
    answer: impl Fn(&str, Option<&Value>) -> Value + Send + 'static,
) -> FakeEngine {
    let temp = tempfile::tempdir_in("/tmp").unwrap();
    let socket = temp.path().join("engine.sock");
    let listener = diri_platform::ipc::asynchronous::UnixListener::bind(&socket).unwrap();
    let (calls_tx, calls) = mpsc::channel(64);
    let (events, mut events_rx) = mpsc::channel::<(String, Value)>(64);
    let task = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let (reader, mut writer) = stream.into_split();
        let mut lines = BufReader::new(reader).lines();
        let mut seq = 0;
        loop {
            let message = tokio::select! {
                line = lines.next_line() => {
                    let Ok(Some(line)) = line else { return };
                    let ControlMessage::Request { id, method, params } =
                        serde_json::from_str(&line).unwrap()
                    else {
                        continue;
                    };
                    let result = if method == Method::HELLO {
                        json!({"proto": diri_proto::WIRE_VERSION, "build":"test", "pid":1, "engineKind":diri_proto::RUST_ENGINE_KIND})
                    } else {
                        let result = answer(&method, params.as_ref());
                        let _ = calls_tx.send((method, params)).await;
                        result
                    };
                    ControlMessage::Response { id, result: Ok(result) }
                }
                Some((name, params)) = events_rx.recv() => {
                    seq += 1;
                    ControlMessage::Event { name, seq, params }
                }
            };
            let mut bytes = serde_json::to_vec(&message).unwrap();
            bytes.push(b'\n');
            if writer.write_all(&bytes).await.is_err() {
                return;
            }
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
        task,
        _temp: temp,
    }
}
