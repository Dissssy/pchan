#!/bin/bash

cd /git/pchan/frontend
trunk build --release
for i in $(ls ./dist/*.wasm); do mv $i ./tmp.wasm; /git/binaryen/bin/wasm-opt -Oz -o $i ./tmp.wasm; rm ./tmp.wasm; done
