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
It is important to enable WebGPU on the Browser (https://web.dev/gpu-compute/): 

```
WebGPU is available for now in Chrome Canary on desktop behind an experimental flag. 
You can enable it at chrome://flags/#enable-unsafe-webgpu. The API is constantly 
changing and currently unsafe. As GPU sandboxing isn't implemented yet for the WebGPU 
API, it is possible to read GPU data for other processes! Don't browse the web with it enabled.
```
