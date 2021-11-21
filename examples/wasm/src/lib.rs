use std::iter::repeat_with;
use wasm_bindgen::prelude::*;

use futuresdr::blocks::WgpuBuilderWasm;
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
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init().expect("could not initialize logger");

    log::info!("starting");
    let mut fg = Flowgraph::new();

    //let n_items = 20_000;
    let n_items = 2048 * 2048;
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
    //fg.connect_stream_with_type(src, "out", wgpu, "in", wgpu::H2D::new());
    fg.connect_stream(src, "out", wgpu, "in").unwrap();
    fg.connect_stream_with_type(wgpu, "out", snk, "in", wgpu::D2H::new());
    log::info!("*** start runtime  ***");
    fg = Runtime::new().run(fg).await.unwrap();

    log::info!("*** flowgraph finished ***");
    let snk = fg.block_async::<VectorSink<f32>>(snk).unwrap();
    let v = snk.items();

    assert_eq!(v.len(), n_items);
    for i in 0..v.len() {
        if (orig[i] * 12.0 - v[i]).abs() > f32::EPSILON {
            log::info!("***********+");
            log::info!("output wrong: i {}  orig {}   res {}", i, orig[i] * 12.0, v[i]);
            log::info!("output wrong: i {}  orig {}   res {}", i+1, orig[i+1] * 12.0, v[i+1]);
            panic!("wrong data");
        } else {
            if i==0 {
                log::info!("output right: i {}  orig {}   res {}", i, orig[i] * 12.0, v[i]);
                log::info!("output right: i {}  orig {}   res {}", i+1, orig[i+1] * 12.0, v[i+1]);
            }

        }
    }

    log::info!("OUTPUT MATCHES YAY!");
}
