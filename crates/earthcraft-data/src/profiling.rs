//! Opt-in bounded timing capture. Callers never write profile files.
use std::{
    io::{BufWriter, Write},
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
const PROFILE_QUEUE_EVENTS: usize = 8192;
const PROFILE_FLUSH_INTERVAL: Duration = Duration::from_millis(250);
const PROFILE_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
static LOGGER: OnceLock<Option<Logger>> = OnceLock::new();
static DROPPED: AtomicU64 = AtomicU64::new(0);
struct Logger {
    sender: mpsc::SyncSender<Message>,
}
enum Message {
    Event(Event),
    Flush(mpsc::SyncSender<()>),
}
struct Event {
    operation: &'static str,
    unix_us: u128,
    elapsed_us: u128,
    values: Values,
}
#[derive(Clone, Copy, Default)]
pub struct Values {
    pub width: usize,
    pub height: usize,
    pub inflight: usize,
    pub resident_bytes: usize,
    pub count: usize,
}
pub fn init() {
    LOGGER.get_or_init(|| {
        let path = std::env::var_os("EARTHCRAFT_PROFILE")?;
        let (sender, events) = mpsc::sync_channel(PROFILE_QUEUE_EVENTS);
        let result = std::thread::Builder::new()
            .name("earthcraft-profile".into())
            .spawn(move || {
                let result = (|| -> std::io::Result<()> {
                    // Never truncate an existing report accidentally.
                    let file = std::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(path)?;
                    write_events(BufWriter::new(file), events)
                })();
                if let Err(error) = result {
                    eprintln!("Profile capture failed: {error}");
                }
            });
        match result {
            Ok(_) => Some(Logger { sender }),
            Err(error) => {
                eprintln!("Profile worker failed: {error}");
                None
            }
        }
    });
}
fn write_events(mut writer: impl Write, events: mpsc::Receiver<Message>) -> std::io::Result<()> {
    writeln!(
        writer,
        "unix_us,operation,elapsed_us,width,height,inflight,resident_bytes,count,dropped_events"
    )?;
    loop {
        match events.recv_timeout(PROFILE_FLUSH_INTERVAL) {
            Ok(Message::Event(event)) => {
                let v = event.values;
                writeln!(
                    writer,
                    "{},{},{},{},{},{},{},{},{}",
                    event.unix_us,
                    event.operation,
                    event.elapsed_us,
                    v.width,
                    v.height,
                    v.inflight,
                    v.resident_bytes,
                    v.count,
                    DROPPED.load(Ordering::Relaxed)
                )?;
            }
            Ok(Message::Flush(done)) => {
                writer.flush()?;
                let _ = done.send(());
            }
            Err(mpsc::RecvTimeoutError::Timeout) => writer.flush()?,
            Err(mpsc::RecvTimeoutError::Disconnected) => return writer.flush(),
        }
    }
}
pub fn flush() {
    let Some(Some(logger)) = LOGGER.get() else {
        return;
    };
    let (done, result) = mpsc::sync_channel(1);
    if logger.sender.try_send(Message::Flush(done)).is_ok() {
        let _ = result.recv_timeout(PROFILE_SHUTDOWN_TIMEOUT);
    }
}
pub struct Span {
    operation: &'static str,
    start: Instant,
    unix_us: u128,
    pub values: Values,
}
impl Span {
    pub fn new(operation: &'static str) -> Self {
        Self {
            operation,
            start: Instant::now(),
            unix_us: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_micros(),
            values: Values::default(),
        }
    }
    pub fn elapsed(operation: &'static str, start: Instant, values: Values) {
        let span = Self {
            operation,
            start,
            values,
            unix_us: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_micros(),
        };
        drop(span);
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        let Some(Some(logger)) = LOGGER.get() else {
            return;
        };
        let event = Event {
            operation: self.operation,
            unix_us: self.unix_us,
            elapsed_us: self.start.elapsed().as_micros(),
            values: self.values,
        };
        if logger.sender.try_send(Message::Event(event)).is_err() {
            DROPPED.fetch_add(1, Ordering::Relaxed);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn report_records_dimensions_duration_and_flushes() {
        let (tx, rx) = mpsc::sync_channel(4);
        tx.send(Message::Event(Event {
            operation: "ui.update",
            unix_us: 10,
            elapsed_us: 20,
            values: Values {
                width: 30,
                height: 40,
                ..Values::default()
            },
        }))
        .unwrap();
        drop(tx);
        let mut report = Vec::new();
        write_events(&mut report, rx).unwrap();
        let text = String::from_utf8(report).unwrap();
        assert!(text.contains("10,ui.update,20,30,40,0,0,0,"));
    }
}
