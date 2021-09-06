use crate::error::*;
use crate::inner::*;
use crate::types::*;
use futures::Stream;
use std::fmt;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock as AsyncRwLock;
use tokio_stream::wrappers::UnboundedReceiverStream;

#[derive(Debug)]
/// A command streamed via the `MONITOR` command.
pub struct MonitorFrame {
  pub key: Option<String>,
  pub args: Vec<RedisValue>,
  pub received: Instant,
  pub server: Arc<String>,
}

/// A client used for reading the output of the `MONITOR` command.
#[derive(Clone)]
pub struct Monitor {
  inner: Arc<RedisClientInner>,
  running: Arc<AsyncRwLock<bool>>,
}

impl fmt::Debug for Monitor {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "[Monitor Client]")
  }
}

impl Monitor {
  /// Create a new client to run the `MONITOR` command.
  pub fn new(config: RedisConfig) -> Self {
    Monitor {
      inner: RedisClientInner::new(config),
      running: Arc::new(AsyncRwLock::new(false)),
    }
  }

  /// Run the `MONITOR` command against the associated server(s).
  ///
  /// When run against a clustered deployment this will run `MONITOR` against every node in the cluster. To monitor
  /// individual nodes in a cluster the caller should provide a centralized `RedisConfig`.
  pub async fn monitor(&self) -> impl Stream<Item = Result<MonitorFrame, RedisError>> {
    // need to make sure the running flag is set correctly once the receiver is dropped
    unimplemented!()
  }

  /// Stop the monitor stream and close the connection(s) to the server.
  pub async fn stop(&self) {
    //
    unimplemented!()
  }
}

/*
add resp version as arc/rwlock/version on inner
need to store it htat way to the codec can change it

change connection logic to use this when creating codecs
change protocol utils to work ith resp3 frames


*/
