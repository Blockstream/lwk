# Jade emulator for LWK

it's still a manual process

```shell
$ export LWK=<point to lwk project dir>
$ export JADE_EMULATOR_DIR=${LWK}/context/jade-emulator/
$ export VERSION=1.0.27 # update with recent tag
$ cd /tmp
$ git clone https://github.com/Blockstream/Jade
$ cd Jade
$ git checkout $VERSION
$ git submodule update --init --recursive
$ export BUILDER=$(cat .gitlab-ci.yml | grep sha256 | cut -d' ' -f2) # blockstream/verde@sha256:b95127cfd8c3df6031b6dcb8cdef163abd7da005d514f41d8ecefcfa21cc61d2
$ docker run -v ${PWD}:/jade -p 30121:30121 -it $BUILDER bash
# . $HOME/esp/esp-idf/export.sh 
# cd /jade
# cp configs/sdkconfig_qemu_psram.defaults ./sdkconfig.defaults
# idf.py all
# virtualenv -p python3 ./venv3
# source ./venv3/bin/activate
# pip install -r requirements.txt
# pip install click
# ./tools/fwprep.py build/jade.bin build
# ./main/qemu/make-flash-img.sh
# mkdir firmware
# cp /flash_image.bin firmware/
# cp /qemu_efuse.bin firmware/
# exit
$ sudo chown -R $USER:$USER firmware/
$ mv firmware/* $JADE_EMULATOR_DIR
$ cd $JADE_EMULATOR_DIR
$ echo "FROM $BUILDER" > Dockerfile
$ cat Dockerfile.suffix >> Dockerfile
$ docker build . -t xenoky/local-jade-emulator:$VERSION
$ docker push xenoky/local-jade-emulator:$VERSION # needs auth
```
