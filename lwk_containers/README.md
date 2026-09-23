# LWK Containers
Docker containers for tests environment.

This is a test crate used mainly internally, we reserve to do breaking changes.

This crate uses [testcontainers](https://github.com/testcontainers/testcontainers-rs).

## Accessing docker logs

Accessing the pin server logs can be done by keeping the docker around after testing with

```
export TESTCONTAINERS=keep
```

Then identifying the id of the docker with:

```
docker ps
```

Then:

```
docker logs $ID
```
