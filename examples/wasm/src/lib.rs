use std::iter::repeat_with;
use wasm_bindgen::prelude::*;

//use wasm_rs_async_executor::single_threaded::block_on;

use futuresdr::blocks::{WgpuBuilderWasm};
use futuresdr::blocks::VectorSink;
use futuresdr::blocks::VectorSinkBuilder;
use futuresdr::blocks::VectorSourceBuilder;
use log::info;
use futuresdr::runtime::Flowgraph;
use futuresdr::runtime::Runtime;
use futuresdr::runtime::buffer::wgpu;
use futuresdr::runtime::buffer::wgpu::WgpuBroker;

#[wasm_bindgen]
pub async fn run_fg() {
    run().await;


}

async fn run(){

    //std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    //futuresdr::runtime::init();

    //futuresdr::log::info!("hallo test");
    //log::info!("hallo test start");
    let mut fg = Flowgraph::new();

    let n_items = 50_000;
    let orig: Vec<f32> = repeat_with(rand::random::<f32>).take(n_items).collect();

    log::info!("*** start building wgpu Broker ***");



    let wgpu_broker = WgpuBroker::new().await;

    log::info!("*** start creating blocks  - SRC ***");
    let src = VectorSourceBuilder::<f32>::new(orig.clone()).build();
    log::info!("*** start creating blocks - WGPU  ***");

    let wgpu = WgpuBuilderWasm::new(wgpu_broker).build();
    log::info!("*** start creating blocks - SNK  ***");
    let snk = VectorSinkBuilder::<f32>::new().build();
    info!("*** start adding blocks  ***");
    let src = fg.add_block(src);
    let wgpu = fg.add_block(wgpu);
    let snk = fg.add_block(snk);

    log::info!("*** connect streams ***");
    fg.connect_stream_with_type(src, "out", wgpu, "in", wgpu::H2D::new()).unwrap();
    fg.connect_stream_with_type(wgpu, "out", snk, "in", wgpu::D2H::new()).unwrap();
    log::info!("*** start runtime  ***");
    fg = Runtime::new().run(fg).unwrap();
    log::info!("*** start sink  ***");
    let snk = fg.block_async::<VectorSink<f32>>(snk).unwrap();
    let v = snk.items();

    assert_eq!(v.len(), n_items);
    for i in 0..v.len() {
        assert!((orig[i] * 12.0 - v[i]).abs() <  f32::EPSILON);
    }

}