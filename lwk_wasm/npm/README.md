# LWK npm workspace

This directory is the private npm workspace used to publish:

- `lwk_node`
- `lwk_web`

Workspace development:

```sh
npm ci
npm run build
npm run test
```

These commands validate both published workspace packages.

Package tarball checks:

```sh
npm run pack:check
```
