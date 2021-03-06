// socket abstraction.

use std::fmt::Display;
use std::io::{BufReader, BufWriter, Write};
use std::net::{TcpStream, ToSocketAddrs};

use crossbeam_channel::{Receiver, Sender, TryRecvError};
use serde::{de::DeserializeOwned, Serialize};

struct Finished<T: Display + Send>(Option<T>);
struct Abort;

struct ControllableThread<T: Display + Send> {
    finished: Receiver<Finished<T>>,
    abort: Sender<Abort>,
}

impl<T: Display + Send + 'static> ControllableThread<T> {
    fn new<F>(name: &'static str, f: F) -> Self
    where
        F: FnOnce(Receiver<Abort>) -> Result<(), T> + Send + 'static,
        T: std::fmt::Display,
    {
        let (tx_finished, rx_finished) = crossbeam_channel::unbounded();
        let (tx_abort, rx_abort) = crossbeam_channel::unbounded();

        let _ = std::thread::Builder::new().name(name.into()).spawn(move || {
            let status = f(rx_abort);
            match &status {
                Err(error) => eprintln!("thread '{}' returned with error: {}", name, error),
                _ => (),
            }
            let _ = tx_finished.send(Finished(status.err()));
        });

        ControllableThread {
            finished: rx_finished,
            abort: tx_abort,
        }
    }

    fn tick(&self) -> Result<bool, T> {
        match self.finished.try_recv() {
            Ok(Finished(result)) => match result {
                Some(error) => Err(error),
                None => Ok(true),
            },
            Err(TryRecvError::Empty) => Ok(false),
            Err(TryRecvError::Disconnected) => Ok(true),
        }
    }

    fn abort(&self) {
        let _ = self.abort.send(Abort);
    }
}

// P is the packet type
pub struct Remote<P: Serialize + DeserializeOwned + Send + 'static> {
    rx: Receiver<P>,
    tx: Sender<P>,
    send: ControllableThread<anyhow::Error>,
    recv: ControllableThread<anyhow::Error>,
}

impl<P: Serialize + DeserializeOwned + Send + core::fmt::Debug + 'static> Remote<P> {
    pub fn new(addr: impl ToSocketAddrs) -> anyhow::Result<Self> {
        let stream = TcpStream::connect(addr)?;
        stream.set_nodelay(true)?;

        let (to_thread, from_main) = crossbeam_channel::unbounded();
        let (to_main, from_thread) = crossbeam_channel::unbounded();

        const MEGABYTE: usize = 1024 * 1024;

        let mut writer = BufWriter::with_capacity(2 * MEGABYTE, stream.try_clone()?);
        let send = ControllableThread::new("network send thread", move |abort| -> anyhow::Result<()> {
            loop {
                if let Ok(_) | Err(TryRecvError::Disconnected) = abort.try_recv() {
                    break
                }
                while let Ok(packet) = from_main.recv() {
                    bincode::serialize_into(&mut writer, &packet)?;
                    writer.flush()?;
                }
            }
            Ok(())
        });

        let mut reader = BufReader::with_capacity(2 * MEGABYTE, stream.try_clone()?);
        let recv = ControllableThread::new("network recv thread", move |abort| -> anyhow::Result<()> {
            loop {
                if let Ok(_) | Err(TryRecvError::Disconnected) = abort.try_recv() {
                    break
                }
                let packet = bincode::deserialize_from(&mut reader)?;
                if to_main.send(packet).is_err() {
                    anyhow::bail!("Couldn't send packet over to the main thread")
                }
            }
            Ok(())
        });

        Ok(Self {
            rx: from_thread,
            tx: to_thread,
            send,
            recv,
        })
    }

    pub fn send(&self, packet: P) -> anyhow::Result<()> {
        if self.tx.send(packet).is_err() {
            anyhow::bail!("Couldn't send packet over to the network thread")
        } else {
            Ok(())
        }
    }

    pub fn try_recv(&self) -> Option<P> {
        self.rx.try_recv().ok()
    }

    pub fn tick(&self) -> anyhow::Result<bool> {
        self.send.tick().and(self.recv.tick())
    }
}

impl<P: Serialize + DeserializeOwned + Send> Drop for Remote<P> {
    fn drop(&mut self) {
        // intentionally ignore the result:
        // if the thread has already finished, this will fail with an error, because the receiving end has
        // already disconnected.
        let _ = self.send.abort();
        let _ = self.recv.abort();
    }
}
