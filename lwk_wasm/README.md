


```
wasm-pack build
npm init wasm-app www
cd www
```

add this lines to `www/package.json`

```
{
  // ...
  "dependencies": {                     // Add this three lines block!
    "wasm-game-of-life": "file:../pkg"
  },
  "devDependencies": {
    //...
  }
}
```

change this lines to `www/index.js`

```
import * as wasm from "wasm-game-of-life";

wasm.greet();
```

run:

```
npm install
npm run start
```

open `http://localhost:8080`