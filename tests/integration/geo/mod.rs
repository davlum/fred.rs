use fred::prelude::*;
use std::convert::TryInto;

fn loose_eq(lhs: f64, rhs: f64, precision: u32) -> bool {
  let pow = 10_u32.pow(precision) as f64;
  (lhs * pow).round() / pow == (rhs * pow).round() / pow
}

fn loose_eq_pos(lhs: &GeoPosition, rhs: &GeoPosition) -> bool {
  // the test values are accurate to ~5 decimal places but the same values from redis go to ~10 decimal places
  loose_eq(lhs.longitude, rhs.longitude, 5) && loose_eq(lhs.latitude, rhs.latitude, 5)
}

async fn create_fake_data(client: &RedisClient, key: &str) -> Result<Vec<GeoPosition>, RedisError> {
  // GEOADD key 13.361389 38.115556 "Palermo" 15.087269 37.502669 "Catania"

  let values = vec![
    (13.361389, 38.115556, "Palermo").try_into()?,
    (15.087269, 37.502669, "Catania").try_into()?,
  ];

  let _: () = client.geoadd(key, None, false, values.clone()).await?;
  Ok(values.into_iter().map(|p| p.coordinates).collect())
}

pub async fn should_geoadd_values(client: RedisClient, _: RedisConfig) -> Result<(), RedisError> {
  let values: Vec<GeoValue> = vec![
    (13.361389, 38.115556, "Palermo").try_into()?,
    (15.087269, 37.502669, "Catania").try_into()?,
  ];

  for value in values.into_iter() {
    let result: i64 = client.geoadd("foo", None, false, value).await?;
    assert_eq!(result, 1);
  }
  let result: usize = client.zcard("foo").await?;
  assert_eq!(result, 2);

  Ok(())
}

pub async fn should_geohash_values(client: RedisClient, _: RedisConfig) -> Result<(), RedisError> {
  let _ = create_fake_data(&client, "foo").await?;

  let result: String = client.geohash("foo", "Palermo").await?;
  assert_eq!(result, "sqc8b49rny0");
  let result: String = client.geohash("foo", "Catania").await?;
  assert_eq!(result, "sqdtr74hyu0");

  let result: Vec<String> = client.geohash("foo", vec!["Palermo", "Catania"]).await?;
  assert_eq!(result, vec!["sqc8b49rny0", "sqdtr74hyu0"]);

  Ok(())
}

pub async fn should_geopos_values(client: RedisClient, _: RedisConfig) -> Result<(), RedisError> {
  let expected = create_fake_data(&client, "foo").await?;

  let result = client.geopos("foo", vec!["Palermo", "Catania"]).await?;
  let result: Vec<GeoPosition> = result
    .into_array()
    .into_iter()
    .map(|p| p.as_geo_position().unwrap().unwrap())
    .collect();
  for (idx, value) in result.into_iter().enumerate() {
    if !loose_eq_pos(&value, &expected[idx]) {
      panic!("{:?} not equal to {:?}", value, &expected[idx]);
    }
  }

  let result = client.geopos("foo", "Palermo").await?;
  let result = result.as_geo_position().unwrap().unwrap();
  assert!(loose_eq_pos(&result, &expected[0]));

  let result = client.geopos("foo", "Catania").await?;
  let result = result.as_geo_position().unwrap().unwrap();
  assert!(loose_eq_pos(&result, &expected[1]));

  Ok(())
}

pub async fn should_geodist_values(client: RedisClient, _: RedisConfig) -> Result<(), RedisError> {
  let _ = create_fake_data(&client, "foo").await?;

  let result: f64 = client.geodist("foo", "Palermo", "Catania", None).await?;
  assert!(loose_eq(result, 166274.1516, 4));
  let result: f64 = client
    .geodist("foo", "Palermo", "Catania", Some(GeoUnit::Kilometers))
    .await?;
  assert!(loose_eq(result, 166.2742, 4));
  let result: f64 = client
    .geodist("foo", "Palermo", "Catania", Some(GeoUnit::Miles))
    .await?;
  assert!(loose_eq(result, 103.3182, 4));

  Ok(())
}

pub async fn should_georadius_values(client: RedisClient, _: RedisConfig) -> Result<(), RedisError> {
  let _ = create_fake_data(&client, "foo").await?;

  let result = client
    .georadius(
      "foo",
      (15.0, 37.0),
      200.0,
      GeoUnit::Kilometers,
      false,
      true,
      false,
      None,
      None,
      None,
      None,
    )
    .await?;
  let expected: Vec<GeoRadiusInfo> = vec![
    GeoRadiusInfo {
      member: "Palermo".into(),
      distance: Some(190.4424),
      position: None,
      hash: None,
    },
    GeoRadiusInfo {
      member: "Catania".into(),
      distance: Some(56.4413),
      position: None,
      hash: None,
    },
  ];
  assert_eq!(result, expected);

  let result = client
    .georadius(
      "foo",
      (15.0, 37.0),
      200.0,
      GeoUnit::Kilometers,
      true,
      false,
      false,
      None,
      None,
      None,
      None,
    )
    .await?;
  let expected: Vec<GeoRadiusInfo> = vec![
    GeoRadiusInfo {
      member: "Palermo".into(),
      distance: None,
      position: Some((13.36138933897018433, 38.11555639549629859).into()),
      hash: None,
    },
    GeoRadiusInfo {
      member: "Catania".into(),
      distance: None,
      position: Some((15.08726745843887329, 37.50266842333162032).into()),
      hash: None,
    },
  ];
  assert_eq!(result, expected);

  let result = client
    .georadius(
      "foo",
      (15.0, 37.0),
      200.0,
      GeoUnit::Kilometers,
      true,
      true,
      false,
      None,
      None,
      None,
      None,
    )
    .await?;
  let expected: Vec<GeoRadiusInfo> = vec![
    GeoRadiusInfo {
      member: "Palermo".into(),
      distance: Some(190.4424),
      position: Some((13.36138933897018433, 38.11555639549629859).into()),
      hash: None,
    },
    GeoRadiusInfo {
      member: "Catania".into(),
      distance: Some(56.4413),
      position: Some((15.08726745843887329, 37.50266842333162032).into()),
      hash: None,
    },
  ];
  assert_eq!(result, expected);

  Ok(())
}

pub async fn should_georadiusbymember_values(client: RedisClient, _: RedisConfig) -> Result<(), RedisError> {
  let _ = create_fake_data(&client, "foo").await?;
  let agrigento: GeoValue = (13.583333, 37.316667, "Agrigento").try_into()?;
  let _ = client.geoadd("foo", None, false, agrigento).await?;

  let result = client
    .georadiusbymember(
      "foo",
      "Agrigento",
      100.0,
      GeoUnit::Kilometers,
      false,
      false,
      false,
      None,
      None,
      None,
      None,
    )
    .await?;
  let expected = vec![
    GeoRadiusInfo {
      member: "Agrigento".into(),
      distance: None,
      position: None,
      hash: None,
    },
    GeoRadiusInfo {
      member: "Palermo".into(),
      distance: None,
      position: None,
      hash: None,
    },
  ];
  assert_eq!(result, expected);

  Ok(())
}

pub async fn should_geosearch_values(client: RedisClient, _: RedisConfig) -> Result<(), RedisError> {
  let _ = create_fake_data(&client, "foo").await?;
  let values = vec![
    (12.758489, 38.788135, "edge1").try_into()?,
    (17.241510, 38.788135, "edge2").try_into()?,
  ];
  let _ = client.geoadd("foo", None, false, values).await?;

  let lonlat: GeoPosition = (15.0, 37.0).into();
  let result = client
    .geosearch(
      "foo",
      None,
      Some(lonlat.clone()),
      Some((200.0, GeoUnit::Kilometers)),
      None,
      Some(SortOrder::Asc),
      None,
      false,
      false,
      false,
    )
    .await?;
  let expected = vec![
    GeoRadiusInfo {
      member: "Catania".into(),
      distance: None,
      position: None,
      hash: None,
    },
    GeoRadiusInfo {
      member: "Palermo".into(),
      distance: None,
      position: None,
      hash: None,
    },
  ];
  assert_eq!(result, expected);

  let result = client
    .geosearch(
      "foo",
      None,
      Some(lonlat),
      None,
      Some((400.0, 400.0, GeoUnit::Kilometers)),
      Some(SortOrder::Asc),
      None,
      true,
      true,
      false,
    )
    .await?;
  let expected = vec![
    GeoRadiusInfo {
      member: "Catania".into(),
      distance: Some(56.4413),
      position: Some((15.08726745843887329, 37.50266842333162032).into()),
      hash: None,
    },
    GeoRadiusInfo {
      member: "Palermo".into(),
      distance: Some(190.4424),
      position: Some((13.36138933897018433, 38.11555639549629859).into()),
      hash: None,
    },
    GeoRadiusInfo {
      member: "edge2".into(),
      distance: Some(279.7403),
      position: Some((17.24151045083999634, 38.78813451624225195).into()),
      hash: None,
    },
    GeoRadiusInfo {
      member: "edge1".into(),
      distance: Some(279.7405),
      position: Some((12.7584877610206604, 38.78813451624225195).into()),
      hash: None,
    },
  ];
  assert_eq!(result, expected);

  Ok(())
}
