# FutureSDR

Fork of https://github.com/FutureSDR/FutureSDR

## To compile WebGPU natively:

```console
run --package futuresdr --example wgpu
```

## To compile WebGPU to Wasm:

```console
cd examples/wasm
make
cd dist
../server.py
```
