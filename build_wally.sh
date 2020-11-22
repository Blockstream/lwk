git clone https://github.com/ElementsProject/libwally-core
cd libwally-core
git checkout 88fc78ff72a4f3345fcb87d1c19dc5f6cc5b0e4c
./tools/autogen.sh
./configure --enable-debug --prefix=$PWD/build --enable-static --disable-shared --enable-elements --enable-ecmult-static-precomputation
make
make install
cd ..
