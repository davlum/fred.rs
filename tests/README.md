# Testing

Tests are organized by category, similar to the [commands](../src/commands) folder.

By default, most tests run 4 times based on the following configuration parameters: clustered vs centralized servers and pipelined vs non-pipelined clients. Helper macros exist to make this easy so each test only has to be written once.

**The tests require Redis version >=6.2**

## Installation

The [environ](environ) file contains any environment variables that might be needed. **This should be loaded before installing or running any tests, unless otherwise set manually.**

In order to run the installation scripts the following must be installed:

* Bash (all the scripts assume `bash`)
* `build-essential` (or equivalent)
* OpenSSL (`libssl-dev`) for testing TLS features without vendored OpenSSL dependencies
* `docker`
* `docker-compose` (this may come with `docker` depending on the version you use)
* `expect`

### Fresh Installation

A script is included to start a fresh install, but **it will shut down any local redis processes first, including those from docker.**

```
source tests/environ
./tests/scripts/full_install.sh
```

### Custom Installation

Redis installation scripts exist to install Redis at any version. Use the env variable `REDIS_VERSION` to configure this, either setting manually **after** sourcing the [environ](environ), or by changing this file. These scripts will only modify the contents of the [tests/tmp](../tests/tmp) folder. 

* [Install Centralized](scripts/install_redis_centralized.sh) will download, install, and start a centralized server on port 6379.
* [Install Clustered](scripts/install_redis_clustered.sh) will download, install, and start a clustered deployment on ports 30001-30006.
* [Install Sentinel](scripts/docker-install-redis-sentinel.sh) will download, install, and start a sentinel deployment with docker-compose.

The tests assume that redis servers are running on the above ports. The installation scripts will modify ACL rules, so installing redis via other means may not work with the tests as they're currently written unless users manually set up the test users as well.

## Running Tests

Once the required local Redis servers are installed users can run tests with the [runner](runners) scripts.

* [all-features](runners/all-features.sh) will run tests with all features (except chaos monkey and sentinel tests).
* [default-features](runners/default-features.sh) will run tests with default features (except sentinel tests).
* [no-features](runners/no-features.sh) will run the tests without any of the feature flags.
* [sentinel-features](runners/sentinel-features.sh) will run the centralized tests against a sentinel deployment. This is the only test runner that requires the sentinel deployment via docker-compose.
* [everything](runners/everything.sh) will run all of the above scripts. 

These scripts will pass through any extra argv so callers can filter tests as needed.

See the [CI configuration](../.circleci/config.yml) for more information.

Note: the [stop redis script](scripts/stop_all_redis.sh) can stop all local Redis servers, including those started via docker.

## Adding Tests

Adding tests is straightforward with the help of some macros and utility functions.

Note: When writing tests that operate on multiple keys be sure to use a [hash_tag](https://redis.io/topics/cluster-spec#keys-hash-tags) so that all keys used by a command exist on the same node in a cluster. 

1. If necessary create a new file in the appropriate folder.
2. Create a new async function in the appropriate file. This function should take a `RedisClient` and `RedisConfig` as arguments and should return a `Result<(), RedisError>`. The client will already be connected when this function runs.
3. This new function should **not** be marked as a `#[test]` or `#[tokio::test]`
4. Call the test from the appropriate [integration/cluster.rs](integration/cluster.rs) or [integration/centralized.rs](integration/centralized.rs) files, or both. Create a wrapping `mod` block with the same name as the test's folder if necessary.
5. Use `centralized_test!` or `cluster_test!` to generate tests in the appropriate module. Centralized tests will be automatically converted to sentinel tests if using the sentinel testing features.

Tests that use this pattern will run 4 times to check the functionality against a clustered and centralized redis servers using both pipelined and non-pipelined clients.

## Chaos Monkey

This module ships with a testing module that will randomly stop, start, restart, and rebalance the redis servers while tests are running. If this is enabled the tests should take longer but should not produce any errors.

To run the tests with the chaos monkey process use [run_with_chaos_monkey.sh](./run_with_chaos_monkey.sh) from the application root. 

## Notes

* Since we're mutating shared state in external redis servers with these tests it's necessary to run the tests with `--test-threads=1`. The test runner scripts will do this automatically.
* **The tests will periodically call `flushall` before each test iteration.**

If you're not running any other docker containers locally, and you feel like your docker containers aren't updating, try a fresh install...

```
# THIS WILL DESTROY ANY LOCAL DOCKER CONTAINER STATE
docker container kill $(docker ps -q)
docker container rm $(docker ps -aq)
```

## Contributing

The following modules still need better test coverage:

* ACL commands
* Cluster commands. This one is more complicated though since many of these modify the cluster.
* Client commands