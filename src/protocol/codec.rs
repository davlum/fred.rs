use crate::error::RedisError;
use crate::inner::RedisClientInner;
use crate::metrics::SizeStats;
use crate::protocol::utils as protocol_utils;
use bytes::BytesMut;
use parking_lot::RwLock;
use redis_protocol::resp2::decode::decode as resp2_decode;
use redis_protocol::resp2::encode::encode_bytes as resp2_encode;
use redis_protocol::resp2::types::Frame as Resp2Frame;
use redis_protocol::resp3::decode::streaming::decode as resp3_decode;
use redis_protocol::resp3::encode::complete::encode_bytes as resp3_encode;
use redis_protocol::resp3::types::{Frame as Resp3Frame, FrameKind as Resp3FrameKind, RespVersion, StreamedFrame};
use std::sync::Arc;
use tokio_util::codec::{Decoder, Encoder};

#[cfg(feature = "network-logs")]
use std::str;

#[cfg(feature = "blocking-encoding")]
use crate::globals::globals;
use redis_protocol::resp2_frame_to_resp3;

#[cfg(not(feature = "network-logs"))]
fn log_resp2_frame(_: &str, _: &Resp2Frame, _: bool) {}

#[cfg(feature = "network-logs")]
#[derive(Debug)]
enum DebugFrame {
  String(String),
  Bytes(Vec<u8>),
  Integer(i64),
  Array(Vec<DebugFrame>),
  None,
}

#[cfg(feature = "network-logs")]
impl From<Option<String>> for DebugFrame {
  fn from(d: Option<String>) -> DebugFrame {
    d.map(|d| DebugFrame::String(d)).unwrap_or(DebugFrame::None)
  }
}

#[cfg(feature = "network-logs")]
impl From<Option<i64>> for DebugFrame {
  fn from(d: Option<i64>) -> DebugFrame {
    d.map(|d| DebugFrame::Integer(d)).unwrap_or(DebugFrame::None)
  }
}

#[cfg(feature = "network-logs")]
impl<'a> From<&'a Resp2Frame> for DebugFrame {
  fn from(f: &'a Resp2Frame) -> Self {
    match f {
      Resp2Frame::Error(s) | Resp2Frame::SimpleString(s) => DebugFrame::String(s.to_owned()),
      Resp2Frame::Integer(i) => DebugFrame::Integer(*i),
      Resp2Frame::BulkString(b) => match str::from_utf8(b) {
        Ok(s) => DebugFrame::String(s.to_owned()),
        Err(_) => DebugFrame::Bytes(b.to_vec()),
      },
      Resp2Frame::Null => DebugFrame::String("nil".into()),
      Resp2Frame::Array(frames) => DebugFrame::Array(frames.iter().map(|f| f.into()).collect()),
    }
  }
}

#[cfg(feature = "network-logs")]
fn log_resp2_frame(name: &str, frame: &Resp2Frame, encode: bool) {
  let prefix = if encode { "Encoded" } else { "Decoded" };
  trace!("{}: {} {:?}", name, prefix, DebugFrame::from(frame))
}

pub enum RespFrame {
  Resp2(Resp2Frame),
  Resp3(Resp3Frame),
}

impl RespFrame {
  pub fn into_resp3_frame(self) -> Resp3Frame {
    match self {
      RespFrame::Resp2(frame) => resp2_frame_to_resp3(frame),
      RespFrame::Resp3(frame) => frame,
    }
  }
}

impl From<Resp3Frame> for RespFrame {
  fn from(f: Resp3Frame) -> Self {
    RespFrame::Resp3(f)
  }
}

impl From<Resp2Frame> for RespFrame {
  fn from(f: Resp2Frame) -> Self {
    RespFrame::Resp2(f)
  }
}

fn resp2_encode_frame(codec: &RedisCodec, item: Resp2Frame, dst: &mut BytesMut) -> Result<(), RedisError> {
  let offset = dst.len();

  let res = resp2_encode(dst, &item)?;
  let len = res.saturating_sub(offset);

  trace!(
    "{}: Encoded {} bytes to {}. Buffer len: {}",
    codec.name,
    len,
    codec.server,
    res
  );
  log_resp2_frame(&codec.name, &item, true);
  codec.req_size_stats.write().sample(len as u64);

  Ok(())
}

fn resp2_decode_frame(codec: &RedisCodec, src: &mut BytesMut) -> Result<Option<RespFrame>, RedisError> {
  trace!("{}: Recv {} bytes from {}.", codec.name, src.len(), codec.server);
  if src.is_empty() {
    return Ok(None);
  }

  if let Some((frame, amt)) = resp2_decode(src)? {
    trace!("{}: Parsed {} bytes from {}", codec.name, amt, codec.server);
    log_resp2_frame(&codec.name, &frame, false);
    codec.res_size_stats.write().sample(amt as u64);

    let _ = src.split_to(amt);
    Ok(Some(protocol_utils::check_auth_error(frame).into()))
  } else {
    Ok(None)
  }
}

fn resp3_encode_frame(codec: &RedisCodec, item: Resp3Frame, dest: &mut BytesMut) -> Result<(), RedisError> {
  let offset = dst.len();

  let res = resp3_encode(dst, &item)?;
  let len = res.saturating_sub(offset);

  trace!(
    "{}: Encoded {} bytes to {}. Buffer len: {}",
    codec.name,
    len,
    codec.server,
    res
  );
  // TODO log debug frame
  codec.req_size_stats.write().sample(len as u64);

  Ok(())
}

fn resp3_decode_frame(codec: &mut Resp3Codec, src: &mut BytesMut) -> Result<Option<RespFrame>, RedisError> {
  unimplemented!()
}

pub struct RedisCodec {
  pub name: Arc<String>,
  pub server: String,
  pub req_size_stats: Arc<RwLock<SizeStats>>,
  pub res_size_stats: Arc<RwLock<SizeStats>>,
  pub version: Arc<RwLock<RespVersion>>,
}

impl RedisCodec {
  pub fn new(inner: &Arc<RedisClientInner>, server: String, version: &Arc<RwLock<RespVersion>>) -> Self {
    RedisCodec {
      server,
      version: version.clone(),
      name: inner.id.clone(),
      req_size_stats: inner.req_size_stats.clone(),
      res_size_stats: inner.res_size_stats.clone(),
    }
  }

  pub fn is_resp3(&self) -> bool {
    *self.version.read() == RespVersion::RESP3
  }
}

impl Encoder<RespFrame> for RedisCodec {
  type Error = RedisError;

  #[cfg(not(feature = "blocking-encoding"))]
  fn encode(&mut self, item: RespFrame, dst: &mut BytesMut) -> Result<(), Self::Error> {
    match item {
      RespFrame::Resp2(item) => resp2_encode_frame(&self, item, dst),
      RespFrame::Resp3(item) => resp3_encode_frame(&self, item, dst),
    }
  }

  #[cfg(feature = "blocking-encoding")]
  fn encode(&mut self, item: RespFrame, dst: &mut BytesMut) -> Result<(), Self::Error> {
    match item {
      RespFrame::Resp2(item) => {
        let frame_size = protocol_utils::frame_size(&item);

        if frame_size >= globals().blocking_encode_threshold() {
          trace!("{}: Encoding in blocking task with size {}", self.name, frame_size);
          tokio::task::block_in_place(|| resp2_encode_frame(&self, item, dst))
        } else {
          resp2_encode_frame(&self, item, dst)
        }
      }
      RespFrame::Resp3(item) => unimplemented!(),
    }
  }
}

impl Decoder for RedisCodec {
  type Item = RespFrame;
  type Error = RedisError;

  #[cfg(not(feature = "blocking-encoding"))]
  fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
    if self.is_resp3() {
      unimplemented!()
    } else {
      Ok(resp2_decode_frame(&self, src)?.map(|f| RespFrame::Resp2(f)))
    }
  }

  #[cfg(feature = "blocking-encoding")]
  fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
    if src.len() >= globals().blocking_encode_threshold() {
      trace!("{}: Decoding in blocking task with size {}", self.name, src.len());
      tokio::task::block_in_place(|| {
        if self.is_resp3() {
          unimplemented!()
        } else {
          resp2_decode_frame(&self, src)
        }
      })
    } else {
      resp2_decode_frame(&self, src)
    }
  }
}
