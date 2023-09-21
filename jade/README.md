# jade

rust reimplementation of jade cbor messaging

jade docs: https://github.com/Blockstream/Jade/blob/master/docs/index.rst

test uses [testcontainers](https://github.com/testcontainers/testcontainers-rs) and [insta](https://github.com/mitsuhiko/insta)

if you want to update the snapshots you'll need the cargo insta tool

todo:

emulator (docker image / tcp)

- [x] add_entropy
- [x] set_epoch
- [x] ping
- [x] version_info
- [ ] ...

hardware (serial / actual device)

- [ ] auth_user
- [ ] handshake_init
- [ ] handshake_complete
- [ ] logout
- [ ] ...
