use anyhow::Result;
use std::borrow::Cow;
use wgpu::ComputePipeline;

use crate::runtime::buffer::wgpu::WgpuBroker;
use crate::runtime::buffer::BufferReaderCustom;
use crate::runtime::AsyncKernel;
use crate::runtime::Block;
use crate::runtime::BlockMeta;
use crate::runtime::BlockMetaBuilder;
use crate::runtime::MessageIo;
use crate::runtime::MessageIoBuilder;
use crate::runtime::StreamIo;
use crate::runtime::StreamIoBuilder;
use crate::runtime::WorkIo;
use crate::runtime::buffer::wgpu::{ReaderH2D, WriterD2H, BufferEmpty, BufferFull};

pub struct WgpuWasm {
    broker: WgpuBroker,
    buffer_items: u64,
    pipeline: Option<ComputePipeline>,
    storage_buffer: wgpu::Buffer,
    output_buffers: Vec<BufferEmpty>,
    input_buffers: Vec<BufferFull>,
}

impl WgpuWasm {
    pub fn new(broker: WgpuBroker, buffer_items: u64) -> Block {

        let storage_buffer = broker.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: buffer_items * 4,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Block::new_async(
            BlockMetaBuilder::new("Wgpu").build(),
            StreamIoBuilder::new()
                .add_input("in", 4)
                .add_output("out", 4)
                .build(),
            MessageIoBuilder::<WgpuWasm>::new().build(),
            WgpuWasm {
                broker,
                buffer_items,
                pipeline: None,
                storage_buffer,
                output_buffers: Vec::new(),
                input_buffers: Vec::new(),
            },
        )
    }
}

#[inline]
fn o(sio: &mut StreamIo, id: usize) -> &mut WriterD2H {
    sio.output(id).try_as::<WriterD2H>().unwrap()
}

// #[inline]
// fn i(sio: &mut StreamIo, id: usize) -> &mut ReaderH2D {
//     sio.input(id).try_as::<ReaderH2D>().unwrap()
// }

#[async_trait]
impl AsyncKernel for WgpuWasm {
    async fn init(
        &mut self,
        sio: &mut StreamIo,
        _m: &mut MessageIo<Self>,
        _b: &mut BlockMeta,
    ) -> Result<()> {

        let output_buffer = self.broker.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: self.buffer_items * 4,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: true,
        });

        self.output_buffers.push(BufferEmpty{buffer: output_buffer});

        // let input_buffer = self.broker.device.create_buffer(&wgpu::BufferDescriptor {
        //     label: None,
        //     size: self.buffer_items * 4,
        //     usage: wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC,
        //     mapped_at_creation: true,
        // });
        // let input = i(sio, 0);
        // input.submit( BufferEmpty { buffer:  input_buffer} );

        let cs_module = self.broker.device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
        });

        let compute_pipeline = self.broker.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: None,
            module: &cs_module,
            entry_point: "main",
        });

        self.pipeline = Some( compute_pipeline );

        Ok(())
    }

    async fn work(
        &mut self,
        io: &mut WorkIo,
        sio: &mut StreamIo,
        _mio: &mut MessageIo<Self>,
        _meta: &mut BlockMeta,
    ) -> Result<()> {
        for m in o(sio, 0).buffers().drain(..) {
            debug!("webgpu: received empty output buffer");
            self.output_buffers.push(m);
        }

        // for m in i(sio, 0).buffers().drain(..) {
        //     debug!("webgpu: received full input buffer");
        //     self.input_buffers.push(m);
        // }


        let input = sio.input(0).slice::<u8>();

        let n = std::cmp::min(input.len() / 8192, self.output_buffers.len());

        for i in 0..n {
            let output = self.output_buffers.pop().unwrap();

            // Instantiates the bind group, once again specifying the binding of buffers.
            let bind_group_layout = self.pipeline.as_ref().unwrap().get_bind_group_layout(0);
            let bind_group = self.broker.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.storage_buffer.as_entire_binding(),
                }],
            });
            log::info!("*** bind group created ***");

            let mut dispatch = 8192 as u32 / 4 / 64; // 4: item size, 64: work group size
            // if input.used_bytes as u32 / 4 % 64 > 0 {
            //     dispatch += 1;
            // }

            {
                self.broker.queue.write_buffer(&self.storage_buffer, 0, &input[(i*8192)..((i+1)*8192)]);
                let mut encoder =
                    self.broker.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

                {
                    let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });
                    cpass.set_pipeline(&self.pipeline.as_ref().unwrap());
                    cpass.set_bind_group(0, &bind_group, &[]);
                    cpass.insert_debug_marker("FutureSDR compute");
                    cpass.dispatch(dispatch, 1, 1);
                }

                encoder.copy_buffer_to_buffer(&self.storage_buffer, 0, &output.buffer, 0, 8192);

                log::info!("*** queue submit ***");
                output.buffer.unmap();
                self.broker.queue.submit(Some(encoder.finish()));
            }

            log::info!("*** remapping result buffer ***");
            let buffer_slice = output.buffer.slice(..);
            let buffer_future = buffer_slice.map_async(wgpu::MapMode::Read);

            self.broker.device.poll(wgpu::Maintain::Wait);

            if let Ok(()) = buffer_future.await {
                o(sio, 0).submit(BufferFull {buffer: output.buffer, used_bytes: 8192 });
            } else {
                panic!("failed to map result buffer")
            }
        }

        sio.input(0).consume(n * 2048);

        // Returns data from buffer
        if sio.input(0).finished() && n * 2048 == input.len() {
            io.finished = true;
        }

        Ok(())
    }
}

pub struct WgpuBuilderWasm {
    wgpu_broker: WgpuBroker,
    buffer_items: u64,
}

impl WgpuBuilderWasm {
    pub fn new(broker: WgpuBroker) -> WgpuBuilderWasm {
        WgpuBuilderWasm {
            wgpu_broker: broker,
            buffer_items: 2048,
        }
    }

    pub fn buffer_items(mut self, items: u64) -> WgpuBuilderWasm {
        self.buffer_items = items;
        self
    }

    pub fn build(self) -> Block {
        WgpuWasm::new(self.wgpu_broker, self.buffer_items)
    }
}
