use crate::error::RedisError;
use crate::modules::inner::RedisClientInner;
use crate::protocol::utils as protocol_utils;
use bytes::BytesMut;
use parking_lot::RwLock;
use rand::Rng;
use redis_protocol::resp2::decode::decode as resp2_decode;
use redis_protocol::resp2::encode::encode_bytes as resp2_encode;
use redis_protocol::resp2::types::Frame as Resp2Frame;
use redis_protocol::resp3::decode::streaming::decode as resp3_decode;
use redis_protocol::resp3::encode::complete::encode_bytes as resp3_encode;
use redis_protocol::resp3::types::{Frame as Resp3Frame, RespVersion, StreamedFrame};
use std::sync::Arc;
use tokio_util::codec::{Decoder, Encoder};

#[cfg(feature = "blocking-encoding")]
use crate::globals::globals;
#[cfg(feature = "metrics")]
use crate::modules::metrics::MovingStats;
#[cfg(feature = "network-logs")]
use std::str;

pub enum RespFrame {
  Resp2(Resp2Frame),
  Resp3(Resp3Frame),
}

impl RespFrame {
  pub fn to_resp3_frame(self) -> Resp3Frame {
    match self {
      RespFrame::Resp2(frame) => redis_protocol::resp2_frame_to_resp3(frame),
      RespFrame::Resp3(frame) => frame,
    }
  }
}

impl From<Resp2Frame> for RespFrame {
  fn from(frame: Resp2Frame) -> Self {
    RespFrame::Resp2(frame)
  }
}

impl From<Resp3Frame> for RespFrame {
  fn from(frame: Resp3Frame) -> Self {
    RespFrame::Resp3(frame)
  }
}

#[cfg(feature = "network-logs")]
#[derive(Debug)]
enum DebugFrame {
  String(String),
  Bytes(Vec<u8>),
  Integer(i64),
  Array(Vec<DebugFrame>),
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
impl<'a> From<&'a Resp3Frame> for DebugFrame {
  fn from(f: &'a Resp3Frame) -> Self {
    unimplemented!()
  }
}

#[cfg(feature = "network-logs")]
fn log_resp2_frame(name: &str, frame: &Resp2Frame, encode: bool) {
  let prefix = if encode { "Encoded" } else { "Decoded" };
  trace!("{}: {} {:?}", name, prefix, DebugFrame::from(frame))
}

#[cfg(not(feature = "network-logs"))]
fn log_resp2_frame(_: &str, _: &Resp2Frame, _: bool) {}

#[cfg(feature = "network-logs")]
fn log_resp3_frame(name: &str, frame: &Resp3Frame, encode: bool) {
  let prefix = if encode { "Encoded" } else { "Decoded" };
  trace!("{}: {} {:?}", name, prefix, DebugFrame::from(frame))
}

#[cfg(not(feature = "network-logs"))]
fn log_resp3_frame(_: &str, _: &Resp3Frame, _: bool) {}

#[cfg(feature = "metrics")]
fn sample_stats(codec: &RedisCodec, decode: bool, value: i64) {
  if decode {
    codec.res_size_stats.write().sample(value);
  } else {
    codec.req_size_stats.write().sample(value);
  }
}

#[cfg(not(feature = "metrics"))]
fn sample_stats(_: &RedisCodec, _: bool, _: i64) {}

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
  sample_stats(&codec, false, len as i64);

  Ok(())
}

fn resp2_decode_frame(codec: &RedisCodec, src: &mut BytesMut) -> Result<Option<Resp2Frame>, RedisError> {
  trace!("{}: Recv {} bytes from {}.", codec.name, src.len(), codec.server);
  if src.is_empty() {
    return Ok(None);
  }

  if let Some((frame, amt)) = resp2_decode(src)? {
    trace!("{}: Parsed {} bytes from {}", codec.name, amt, codec.server);
    log_resp2_frame(&codec.name, &frame, false);
    sample_stats(&codec, true, amt as i64);

    let _ = src.split_to(amt);
    Ok(Some(protocol_utils::check_auth_error(frame)))
  } else {
    Ok(None)
  }
}

fn resp3_encode_frame(codec: &RedisCodec, item: Resp3Frame, dst: &mut BytesMut) -> Result<(), RedisError> {
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
  log_resp3_frame(&codec.name, &item, true);
  sample_stats(&codec, false, len as i64);

  Ok(())
}

fn resp3_decode_frame(codec: &mut RedisCodec, src: &mut BytesMut) -> Result<Option<Resp3Frame>, RedisError> {
  trace!("{}: Recv {} bytes from {}.", codec.name, src.len(), codec.server);
  if src.is_empty() {
    return Ok(None);
  }

  if let Some((frame, amt)) = resp2_decode(src)? {
    trace!("{}: Parsed {} bytes from {}", codec.name, amt, codec.server);
    log_resp2_frame(&codec.name, &frame, false);
    sample_stats(&codec, true, amt as i64);

    let _ = src.split_to(amt);
    Ok(Some(protocol_utils::check_auth_error(frame)))
  } else {
    Ok(None)
  }
}

fn is_resp2(codec: &RedisCodec) -> bool {
  *codec.resp_version.read() == RespVersion::RESP2
}

pub struct RedisCodec {
  pub name: Arc<String>,
  pub server: String,
  pub decoder_stream: Option<StreamedFrame>,
  pub resp_version: Arc<RwLock<RespVersion>>,
  #[cfg(feature = "metrics")]
  pub req_size_stats: Arc<RwLock<MovingStats>>,
  #[cfg(feature = "metrics")]
  pub res_size_stats: Arc<RwLock<MovingStats>>,
}

impl RedisCodec {
  pub fn new(inner: &Arc<RedisClientInner>, server: String, resp_version: &Arc<RwLock<RespVersion>>) -> Self {
    RedisCodec {
      server,
      name: inner.id.clone(),
      decoder_stream: None,
      resp_version: resp_version.clone(),
      #[cfg(feature = "metrics")]
      req_size_stats: inner.req_size_stats.clone(),
      #[cfg(feature = "metrics")]
      res_size_stats: inner.res_size_stats.clone(),
    }
  }
}

impl Encoder<RespFrame> for RedisCodec {
  type Error = RedisError;

  #[cfg(not(feature = "blocking-encoding"))]
  fn encode(&mut self, item: Resp2Frame, dst: &mut BytesMut) -> Result<(), Self::Error> {
    match item {
      RespFrame::Resp2(frame) => resp2_encode_frame(&self, frame, dst),
      RespFrame::Resp3(frame) => resp3_encode_frame(&self, frame, dst),
    }
  }

  #[cfg(feature = "blocking-encoding")]
  fn encode(&mut self, item: RespFrame, dst: &mut BytesMut) -> Result<(), Self::Error> {
    let frame_size = protocol_utils::frame_size(&item);

    if frame_size >= globals().blocking_encode_threshold() {
      trace!("{}: Encoding in blocking task with size {}", self.name, frame_size);
      tokio::task::block_in_place(|| match item {
        RespFrame::Resp2(frame) => resp2_encode_frame(&self, frame, dst),
        RespFrame::Resp3(frame) => resp3_encode_frame(&self, frame, dst),
      })
    } else {
      match item {
        RespFrame::Resp2(frame) => resp2_encode_frame(&self, frame, dst),
        RespFrame::Resp3(frame) => resp3_encode_frame(&self, frame, dst),
      }
    }
  }
}

impl Decoder for RedisCodec {
  type Item = RespFrame;
  type Error = RedisError;

  #[cfg(not(feature = "blocking-encoding"))]
  fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
    if is_resp2(&self) {
      resp2_decode_frame(&self, src).map(|f| f.map(|f| f.into()))
    } else {
      resp3_decode_frame(&mut self, src).map(|f| f.map(|f| f.into()))
    }
  }

  #[cfg(feature = "blocking-encoding")]
  fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
    if src.len() >= globals().blocking_encode_threshold() {
      trace!("{}: Decoding in blocking task with size {}", self.name, src.len());
      tokio::task::block_in_place(|| {
        if is_resp2(&self) {
          resp2_decode_frame(&self, src).map(|f| f.map(|f| f.into()))
        } else {
          resp3_decode_frame(&mut self, src).map(|f| f.map(|f| f.into()))
        }
      })
    } else {
      if is_resp2(&self) {
        resp2_decode_frame(&self, src).map(|f| f.map(|f| f.into()))
      } else {
        resp3_decode_frame(&mut self, src).map(|f| f.map(|f| f.into()))
      }
    }
  }
}
