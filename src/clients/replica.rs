use crate::{
  clients::{Pipeline, RedisClient},
  error::RedisError,
  interfaces::{
    self,
    AuthInterface,
    ClientLike,
    FunctionInterface,
    GeoInterface,
    HashesInterface,
    HyperloglogInterface,
    KeysInterface,
    ListInterface,
    LuaInterface,
    MemoryInterface,
    MetricsInterface,
    ServerInterface,
    SetsInterface,
    SlowlogInterface,
    SortedSetsInterface,
    StreamsInterface,
  },
  modules::inner::RedisClientInner,
  protocol::command::{RedisCommand, RouterCommand},
  types::Server,
};
use std::{collections::HashMap, fmt, fmt::Formatter, sync::Arc};
use tokio::sync::oneshot::channel as oneshot_channel;

/// A struct for interacting with replica nodes.
///
/// All commands sent via this interface will use a replica node, if possible. The underlying connections are shared
/// with the main client in order to maintain an up-to-date view of the system in the event that replicas change or
/// are promoted. The cached replica routing table will be updated on the client when following cluster redirections
/// or when any connection closes.
///
/// [Redis replication is asynchronous](https://redis.io/docs/management/replication/).
// ### Cluster Replication
//
// In a clustered deployment replicas may redirect callers back to primary nodes, even with read-only commands,
// depending on the server configuration. The client will automatically follow these redirections, but callers should
// be aware of this behavior for monitoring or tracing purposes.
//
// #### Example
//
// ```bash
// // connect to a primary node, print cluster and replica info, and `GET bar`
// foo@d85c70fd4fc0:/project$ redis-cli -h 172.21.0.5 -p 30001
// 172.21.0.5:30001> cluster nodes
// 60ca8d301ef624956e847e6e6ecc865a36513bbe 172.21.0.3:30001@40001 slave f837e4056f564ab7fd69c24264279a1bd81d6420 0 1674165394000 3 connected
// ddc30573f0c7ee1f79d7f263e2f83d7b83ad0ba0 172.21.0.8:30001@40001 slave 101b2a992c6c909d807d4c5fbd149bcc28e63ef8 0 1674165396000 2 connected
// 101b2a992c6c909d807d4c5fbd149bcc28e63ef8 172.21.0.2:30001@40001 master - 0 1674165395807 2 connected 5461-10922
// 38a7f9d3e440a37adf42f2ceddd9ad52bfb4186e 172.21.0.7:30001@40001 slave bd48cbd28cd927a284bab4424bd41b077a25acb6 0 1674165396810 1 connected
// f837e4056f564ab7fd69c24264279a1bd81d6420 172.21.0.4:30001@40001 master - 0 1674165395000 3 connected 10923-16383
// bd48cbd28cd927a284bab4424bd41b077a25acb6 172.21.0.5:30001@40001 myself,master - 0 1674165393000 1 connected 0-5460
// 172.21.0.5:30001> info replication
// # Replication
// role:master
// connected_slaves:1
// slave0:ip=172.21.0.7,port=30001,state=online,offset=183696,lag=0
// [truncated]
// 172.21.0.5:30001> get bar
// "2"
//
// // connect to the associated replica and `GET bar`
// foo@d85c70fd4fc0:/project$ redis-cli -h 172.21.0.7 -p 30001
// 172.21.0.7:30001> role
// 1) "slave"
// 2) "172.21.0.5"
// 3) (integer) 30001
// 4) "connected"
// 5) (integer) 185390
// 172.21.0.7:30001> get bar
// (error) MOVED 5061 172.21.0.5:30001
// ```
//
// **This can result in unexpected latency or errors depending on the client configuration.**
#[derive(Clone)]
#[cfg_attr(docsrs, doc(cfg(feature = "replicas")))]
pub struct Replicas {
  inner: Arc<RedisClientInner>,
}

impl fmt::Debug for Replicas {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    f.debug_struct("Replicas").field("id", &self.inner.id).finish()
  }
}

#[doc(hidden)]
impl From<&Arc<RedisClientInner>> for Replicas {
  fn from(inner: &Arc<RedisClientInner>) -> Self {
    Replicas { inner: inner.clone() }
  }
}

impl ClientLike for Replicas {
  #[doc(hidden)]
  fn inner(&self) -> &Arc<RedisClientInner> {
    &self.inner
  }

  #[doc(hidden)]
  fn change_command(&self, command: &mut RedisCommand) {
    command.use_replica = true;
  }
}

impl GeoInterface for Replicas {}
impl HashesInterface for Replicas {}
impl HyperloglogInterface for Replicas {}
impl MetricsInterface for Replicas {}
impl KeysInterface for Replicas {}
impl LuaInterface for Replicas {}
impl FunctionInterface for Replicas {}
impl ListInterface for Replicas {}
impl MemoryInterface for Replicas {}
impl AuthInterface for Replicas {}
impl ServerInterface for Replicas {}
impl SlowlogInterface for Replicas {}
impl SetsInterface for Replicas {}
impl SortedSetsInterface for Replicas {}
impl StreamsInterface for Replicas {}

impl Replicas {
  /// Read a mapping of replica server IDs to primary server IDs.
  pub fn nodes(&self) -> HashMap<Server, Server> {
    self.inner.server_state.read().replicas.clone()
  }

  /// Send a series of commands in a [pipeline](https://redis.io/docs/manual/pipelining/).
  pub fn pipeline(&self) -> Pipeline<Replicas> {
    Pipeline::from(self.clone())
  }

  /// Read the underlying [RedisClient](crate::clients::RedisClient) that interacts with primary nodes.
  pub fn client(&self) -> RedisClient {
    RedisClient::from(&self.inner)
  }

  /// Sync the cached replica routing table with the server(s).
  ///
  /// This will also disconnect and reset any replica connections.
  pub async fn sync(&self) -> Result<(), RedisError> {
    let (tx, rx) = oneshot_channel();
    let cmd = RouterCommand::SyncReplicas { tx };
    let _ = interfaces::send_to_router(&self.inner, cmd)?;
    rx.await?
  }
}
