.PHONY: build

build:
	RUSTFLAGS=--cfg=web_sys_unstable_apis cargo build --lib --no-default-features --target wasm32-unknown-unknown
	wasm-bindgen --out-dir ./dist --web /home/basti/.cargo/target/wasm32-unknown-unknown/debug/wasm.wasm

