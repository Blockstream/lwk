
# Accessing docker logs

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
docker log $ID
```