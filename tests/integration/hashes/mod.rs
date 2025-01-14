use fred::client::RedisClient;
use fred::error::RedisError;
use fred::types::{RedisConfig, RedisMap, RedisValue};

use std::collections::{HashMap, HashSet};

fn assert_contains<'a, T: Eq + PartialEq>(values: Vec<T>, item: &'a T) {
  for value in values.iter() {
    if value == item {
      return;
    }
  }

  panic!("Failed to find item in set.");
}

fn assert_diff_len(values: Vec<&'static str>, value: RedisValue, len: usize) {
  if let RedisValue::Array(items) = value {
    let mut expected = HashSet::with_capacity(values.len());
    for value in values.into_iter() {
      expected.insert(value.to_owned());
    }
    let mut actual = HashSet::with_capacity(items.len());
    for item in items.into_iter() {
      let s = &*item.as_str().unwrap();
      actual.insert(s.to_owned());
    }

    let diff = expected.difference(&actual).fold(0, |m, _| m + 1);
    assert_eq!(diff, len);
  } else {
    panic!("Expected value array");
  }
}

pub async fn should_hset_and_hget(client: RedisClient, _: RedisConfig) -> Result<(), RedisError> {
  check_null!(client, "foo");

  let result: i64 = client.hset("foo", ("a", 1.into())).await?;
  assert_eq!(result, 1);
  let result: i64 = client.hset("foo", vec![("b", 2.into()), ("c", 3.into())]).await?;
  assert_eq!(result, 2);

  let a: i64 = client.hget("foo", "a").await?;
  assert_eq!(a, 1);
  let b: i64 = client.hget("foo", "b").await?;
  assert_eq!(b, 2);
  let c: i64 = client.hget("foo", "c").await?;
  assert_eq!(c, 3);

  Ok(())
}

pub async fn should_hset_and_hdel(client: RedisClient, _: RedisConfig) -> Result<(), RedisError> {
  check_null!(client, "foo");

  let result: i64 = client
    .hset("foo", vec![("a", 1.into()), ("b", 2.into()), ("c", 3.into())])
    .await?;
  assert_eq!(result, 3);
  let result: i64 = client.hdel("foo", vec!["a", "b"]).await?;
  assert_eq!(result, 2);
  let result: i64 = client.hdel("foo", "c").await?;
  assert_eq!(result, 1);
  let result: Option<i64> = client.hget("foo", "a").await?;
  assert!(result.is_none());

  Ok(())
}

pub async fn should_hexists(client: RedisClient, _: RedisConfig) -> Result<(), RedisError> {
  check_null!(client, "foo");

  let _: () = client.hset("foo", ("a", 1.into())).await?;
  let a: bool = client.hexists("foo", "a").await?;
  assert!(a);
  let b: bool = client.hexists("foo", "b").await?;
  assert!(!b);

  Ok(())
}

pub async fn should_hgetall(client: RedisClient, _: RedisConfig) -> Result<(), RedisError> {
  check_null!(client, "foo");

  let _: () = client
    .hset("foo", vec![("a", 1.into()), ("b", 2.into()), ("c", 3.into())])
    .await?;
  let values: HashMap<String, i64> = client.hgetall("foo").await?;

  assert_eq!(values.len(), 3);
  let mut expected = HashMap::new();
  expected.insert("a".into(), 1);
  expected.insert("b".into(), 2);
  expected.insert("c".into(), 3);
  assert_eq!(values, expected);

  Ok(())
}

pub async fn should_hincryby(client: RedisClient, _: RedisConfig) -> Result<(), RedisError> {
  check_null!(client, "foo");

  let result: i64 = client.hincrby("foo", "a", 1).await?;
  assert_eq!(result, 1);
  let result: i64 = client.hincrby("foo", "a", 2).await?;
  assert_eq!(result, 3);

  Ok(())
}

pub async fn should_hincryby_float(client: RedisClient, _: RedisConfig) -> Result<(), RedisError> {
  check_null!(client, "foo");

  let result: f64 = client.hincrbyfloat("foo", "a", 0.5).await?;
  assert_eq!(result, 0.5);
  let result: f64 = client.hincrbyfloat("foo", "a", 3.7).await?;
  assert_eq!(result, 4.2);

  Ok(())
}

pub async fn should_get_keys(client: RedisClient, _: RedisConfig) -> Result<(), RedisError> {
  check_null!(client, "foo");

  let _: () = client
    .hset("foo", vec![("a", 1.into()), ("b", 2.into()), ("c", 3.into())])
    .await?;

  let keys = client.hkeys("foo").await?;
  assert_diff_len(vec!["a", "b", "c"], keys, 0);

  Ok(())
}

pub async fn should_hmset(client: RedisClient, _: RedisConfig) -> Result<(), RedisError> {
  check_null!(client, "foo");

  let _: () = client
    .hmset("foo", vec![("a", 1.into()), ("b", 2.into()), ("c", 3.into())])
    .await?;

  let a: i64 = client.hget("foo", "a").await?;
  assert_eq!(a, 1);
  let b: i64 = client.hget("foo", "b").await?;
  assert_eq!(b, 2);
  let c: i64 = client.hget("foo", "c").await?;
  assert_eq!(c, 3);

  Ok(())
}

pub async fn should_hmget(client: RedisClient, _: RedisConfig) -> Result<(), RedisError> {
  check_null!(client, "foo");

  let _: () = client
    .hmset("foo", vec![("a", 1.into()), ("b", 2.into()), ("c", 3.into())])
    .await?;

  let result: Vec<i64> = client.hmget("foo", vec!["a", "b"]).await?;
  assert_eq!(result, vec![1, 2]);

  Ok(())
}

pub async fn should_hsetnx(client: RedisClient, _: RedisConfig) -> Result<(), RedisError> {
  check_null!(client, "foo");

  let _: () = client.hset("foo", ("a", 1.into())).await?;
  let result: bool = client.hsetnx("foo", "a", 2).await?;
  assert_eq!(result, false);
  let result: i64 = client.hget("foo", "a").await?;
  assert_eq!(result, 1);
  let result: bool = client.hsetnx("foo", "b", 2).await?;
  assert!(result);
  let result: i64 = client.hget("foo", "b").await?;
  assert_eq!(result, 2);

  Ok(())
}

pub async fn should_get_random_field(client: RedisClient, _: RedisConfig) -> Result<(), RedisError> {
  check_null!(client, "foo");

  let _: () = client
    .hmset("foo", vec![("a", 1.into()), ("b", 2.into()), ("c", 3.into())])
    .await?;

  let field: String = client.hrandfield("foo", None).await?;
  assert_contains(vec!["a", "b", "c"], &field.as_str());

  let fields = client.hrandfield("foo", Some((2, false))).await?;
  assert_diff_len(vec!["a", "b", "c"], fields, 1);

  let actual: HashMap<String, i64> = client.hrandfield("foo", Some((2, true))).await?;
  assert_eq!(actual.len(), 2);

  let mut expected = RedisMap::new();
  expected.insert("a".into(), "1".into());
  expected.insert("b".into(), "2".into());
  expected.insert("c".into(), "3".into());

  for (key, value) in actual.into_iter() {
    let expected_val: i64 = expected.get(&key).unwrap().clone().convert()?;
    assert_eq!(value, expected_val);
  }

  Ok(())
}

pub async fn should_get_strlen(client: RedisClient, _: RedisConfig) -> Result<(), RedisError> {
  check_null!(client, "foo");

  let expected = "abcdefhijklmnopqrstuvwxyz";
  let _: () = client.hset("foo", ("a", expected.clone().into())).await?;

  let len: usize = client.hstrlen("foo", "a").await?;
  assert_eq!(len, expected.len());

  Ok(())
}

pub async fn should_get_values(client: RedisClient, _: RedisConfig) -> Result<(), RedisError> {
  check_null!(client, "foo");

  let _: () = client.hmset("foo", vec![("a", "1".into()), ("b", "2".into())]).await?;

  let values: RedisValue = client.hvals("foo").await?;
  assert_diff_len(vec!["1", "2"], values, 0);

  Ok(())
}
