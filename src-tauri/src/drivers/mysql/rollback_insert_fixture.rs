//! Inert loopback protocol fixture, not a MySQL server or a live database test.
//! It accepts only the fixed queries used by the INSERT regression tests.
use sqlx::{
    mysql::{MySqlConnectOptions, MySqlSslMode},
    Connection,
};
use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

pub(super) struct State {
    pub queries: Vec<String>,
    pub rows: BTreeSet<u64>,
    pub schema: String,
    pub engine: String,
    pub triggers: bool,
    pub auto_increment: bool,
    pub next_id: u64,
    pub fail_refresh: bool,
}

pub(super) struct Fixture {
    pub conn: sqlx::MySqlConnection,
    pub state: Arc<Mutex<State>>,
    worker: tokio::task::JoinHandle<()>,
}

impl Fixture {
    pub async fn new() -> Self {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let state = Arc::new(Mutex::new(State {
            queries: Vec::new(),
            rows: BTreeSet::new(),
            schema: "fixture".into(),
            engine: "InnoDB".into(),
            triggers: false,
            auto_increment: false,
            next_id: 1,
            fail_refresh: false,
        }));
        let server_state = state.clone();
        let worker = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            stream.set_nodelay(true).unwrap();
            let capabilities = 0x0008_a201_u32;
            let mut hello = vec![10];
            hello.extend_from_slice(b"8.0.0-fixture\0");
            hello.extend_from_slice(&1_u32.to_le_bytes());
            hello.extend_from_slice(b"12345678\0");
            hello.extend_from_slice(&(capabilities as u16).to_le_bytes());
            hello.push(45);
            hello.extend_from_slice(&2_u16.to_le_bytes());
            hello.extend_from_slice(&((capabilities >> 16) as u16).to_le_bytes());
            hello.push(21);
            hello.extend_from_slice(&[0; 10]);
            hello.extend_from_slice(b"abcdefghijkl\0mysql_native_password\0");
            stream.write_all(&packet(0, &hello)).await.unwrap();
            if read_packet(&mut stream).await.is_none() {
                return;
            }
            stream.write_all(&packet(2, &ok(0))).await.unwrap();
            while let Some(command) = read_packet(&mut stream).await {
                let reply = match command.first() {
                    Some(1) | None => break,
                    Some(14) => packet(1, &ok(0)),
                    Some(3) => {
                        let query = std::str::from_utf8(&command[1..]).unwrap();
                        respond(&mut server_state.lock().unwrap(), query)
                    }
                    _ => panic!("Unexpected prepared/binary command: {:?}", command.first()),
                };
                if stream.write_all(&reply).await.is_err() {
                    break;
                }
            }
        });
        let options = MySqlConnectOptions::new()
            .host(&address.ip().to_string())
            .port(address.port())
            .username("protocol_fixture")
            .ssl_mode(MySqlSslMode::Disabled);
        let conn = sqlx::MySqlConnection::connect_with(&options).await.unwrap();
        state.lock().unwrap().queries.clear();
        Self {
            conn,
            state,
            worker,
        }
    }

    pub fn count(&self) -> usize {
        self.state.lock().unwrap().queries.len()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.worker.abort();
    }
}

async fn read_packet(stream: &mut TcpStream) -> Option<Vec<u8>> {
    let mut header = [0; 4];
    stream.read_exact(&mut header).await.ok()?;
    let len =
        usize::from(header[0]) | (usize::from(header[1]) << 8) | (usize::from(header[2]) << 16);
    let mut body = vec![0; len];
    stream.read_exact(&mut body).await.ok()?;
    Some(body)
}

fn packet(sequence: u8, body: &[u8]) -> Vec<u8> {
    let mut out = vec![
        body.len() as u8,
        (body.len() >> 8) as u8,
        (body.len() >> 16) as u8,
        sequence,
    ];
    out.extend_from_slice(body);
    out
}

fn ok(affected: u8) -> Vec<u8> {
    vec![0, affected, 0, 2, 0, 0, 0]
}

fn text(value: &str) -> Option<String> {
    Some(value.into())
}

fn lenenc(out: &mut Vec<u8>, value: &[u8]) {
    assert!(value.len() < 251, "fixture only supports short text fields");
    out.push(value.len() as u8);
    out.extend_from_slice(value);
}

// Column types: VAR_STRING, LONGLONG (signed counts, unsigned counters).
fn rows(types: &[(u8, u16)], values: Vec<Vec<Option<String>>>) -> Vec<u8> {
    let mut out = packet(1, &[types.len() as u8]);
    let mut seq = 2;
    for (index, (kind, flags)) in types.iter().enumerate() {
        let mut col = Vec::new();
        for value in ["def", "fixture", "items", "items", &format!("c{index}"), ""] {
            lenenc(&mut col, value.as_bytes());
        }
        col.extend_from_slice(&[12, 45, 0]);
        col.extend_from_slice(&1024_u32.to_le_bytes());
        col.push(*kind);
        col.extend_from_slice(&flags.to_le_bytes());
        col.extend_from_slice(&[0, 0, 0]);
        out.extend(packet(seq, &col));
        seq += 1;
    }
    let eof = [0xfe, 0, 0, 2, 0];
    out.extend(packet(seq, &eof));
    seq += 1;
    for value in values {
        let mut row = Vec::new();
        for field in value {
            match field {
                Some(value) => lenenc(&mut row, value.as_bytes()),
                None => row.push(0xfb),
            }
        }
        out.extend(packet(seq, &row));
        seq += 1;
    }
    out.extend(packet(seq, &eof));
    out
}

fn respond(state: &mut State, query: &str) -> Vec<u8> {
    state.queries.push(query.to_string());
    let string = (0xfd, 0);
    let unsigned = (8, 32);
    if query.starts_with("SET ") {
        return packet(1, &ok(0));
    }
    if query == "SELECT DATABASE()" {
        return rows(&[string], vec![vec![text(&state.schema)]]);
    }
    if query.starts_with("SELECT ENGINE, TABLE_TYPE, AUTO_INCREMENT") {
        return rows(
            &[string, string, unsigned],
            vec![vec![
                text(&state.engine),
                text("BASE TABLE"),
                state.auto_increment.then(|| state.next_id.to_string()),
            ]],
        );
    }
    if query.starts_with("SELECT AUTO_INCREMENT FROM") {
        if state.fail_refresh {
            let mut error = vec![0xff, 0x51, 0x04];
            error.extend_from_slice(b"#HY000fixture counter read failed");
            return packet(1, &error);
        }
        return rows(&[unsigned], vec![vec![Some(state.next_id.to_string())]]);
    }
    if query.starts_with("SELECT COLUMN_NAME, DATA_TYPE, EXTRA, GENERATION_EXPRESSION") {
        return rows(
            &[string; 4],
            vec![vec![
                text("id"),
                text("bigint"),
                text(if state.auto_increment {
                    "auto_increment"
                } else {
                    ""
                }),
                text(""),
            ]],
        );
    }
    if query.starts_with("SELECT COLUMN_NAME FROM information_schema.STATISTICS") {
        return rows(&[string], vec![vec![text("id")]]);
    }
    if query.contains("information_schema.TRIGGERS") {
        return rows(
            &[(8, 0)],
            vec![vec![Some(u8::from(state.triggers).to_string())]],
        );
    }
    if query.starts_with("SELECT 1 FROM") && query.contains("WHERE FALSE FOR UPDATE") {
        return rows(&[(8, 0)], vec![]);
    }
    if query.starts_with("SELECT CASE WHEN") {
        let id: u64 = query
            .split("`id` <=> ")
            .nth(1)
            .unwrap()
            .chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .parse()
            .unwrap();
        let captured = if state.rows.contains(&id) {
            let hex = id
                .to_string()
                .bytes()
                .map(|b| format!("{b:02X}"))
                .collect::<String>();
            vec![vec![text(&format!("X'{hex}'"))]]
        } else {
            vec![]
        };
        return rows(&[string], captured);
    }
    if query.starts_with("INSERT INTO items (id) VALUES (") {
        let id = query
            .trim_start_matches("INSERT INTO items (id) VALUES (")
            .trim_end_matches(')')
            .parse::<u64>()
            .unwrap();
        assert!(
            state.rows.insert(id),
            "duplicate insert escaped the row-image guard"
        );
        if state.auto_increment {
            state.next_id = state.next_id.max(id + 1);
        }
        return packet(1, &ok(1));
    }
    panic!("Unexpected fixture query: {query}");
}
