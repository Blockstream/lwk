# Jade emulator for LWK


```shell
git clone https://github.com/Blockstream/Jade
cd Jade
git checkout $VERSION
git submodule update --init --recursive
docker build -t xenoky/local-jade-emulator:$VERSION -f Dockerfile.qemu .
docker push xenoky/local-jade-emulator:$VERSION # needs auth
```
