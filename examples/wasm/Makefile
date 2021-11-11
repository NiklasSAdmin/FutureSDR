.PHONY: build

build:
	#wasm-pack build --target web --out-name wasm --out-dir ./dist --dev
	RUSTFLAGS=--cfg=web_sys_unstable_apis cargo build --no-default-features --target wasm32-unknown-unknown

# Generate bindings in a `target/generated` directory for the hello-triangle example
	wasm-bindgen --out-dir ./dist --web ../../target/wasm32-unknown-unknown/debug/wasm.wasm

