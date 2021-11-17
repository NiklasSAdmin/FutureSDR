use anyhow::{Result};

use std::{borrow::Cow};
use wgpu::{ComputePipeline};

use crate::runtime::buffer::wgpu::{WgpuBroker};
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
use crate::runtime::buffer::wgpu::{ReaderH2D, WriterD2H, BufferEmpty};


pub struct WgpuWasm {
    broker: WgpuBroker,
    capacity: u64,
    pipeline: Option<ComputePipeline>,
}

impl WgpuWasm {
    pub fn new(broker: WgpuBroker, capacity: u64) -> Block {
        Block::new_async(
            BlockMetaBuilder::new("Wgpu").build(),
            StreamIoBuilder::new()
                .add_input("in", 4)
                .add_output("out", 4)
                .build(),
            MessageIoBuilder::<WgpuWasm>::new().build(),
            WgpuWasm {
                broker,
                capacity,
                pipeline: None,
            },
        )
    }
}

#[inline]
fn o(sio: &mut StreamIo, id: usize) -> &mut WriterD2H {
    sio.output(id).try_as::<WriterD2H>().unwrap()
}

#[inline]
fn i(sio: &mut StreamIo, id: usize) -> &mut ReaderH2D {
    sio.input(id).try_as::<ReaderH2D>().unwrap()
}

#[async_trait]
impl AsyncKernel for WgpuWasm {
    async fn init(
        &mut self,
        sio: &mut StreamIo,
        _m: &mut MessageIo<Self>,
        _b: &mut BlockMeta,
    ) -> Result<()> {

        let input = i(sio, 0);

                    let staging_buffer;

                        staging_buffer = self.broker.device.create_buffer(&wgpu::BufferDescriptor {
                            label: None,
                            size: self.capacity,
                            usage:  wgpu::BufferUsages::COPY_DST
                                | wgpu::BufferUsages::COPY_SRC
                                | wgpu::BufferUsages::MAP_READ
                                | wgpu::BufferUsages::STORAGE,
                            mapped_at_creation: true,
                        });

                    input.submit( BufferEmpty { buffer:  staging_buffer} );


        let cs_module = self.broker.device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: None,
            //source: wgpu::ShaderSource::SpirV(Cow::Borrowed(include_bytes!("comp.spv"))),
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
            debug!("webgpu: forwarding buff from output to input");
            i(sio, 0).submit(m);
        }

        for m in i(sio, 0).buffers().drain(..) {


            let tmp_buffer = self.broker.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: 8192,
                usage:  wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });


            let slice = tmp_buffer.slice(..);
            let future = slice.map_async(wgpu::MapMode::Read);
            log::info!("*** tmp buffer test ***");
            self.broker.device.poll(wgpu::Maintain::Poll);
            if let Ok(()) = future.await {
                log::info!("***SUCCESS: Buffer inputs WORK: ***");
                info!(" {:?}", tmp_buffer.slice(..));

            } else {
                panic!("failed to run compute on gpu!")
            }




            // Instantiates the bind group, once again specifying the binding of buffers.
           let bind_group_layout = self.pipeline.as_ref().unwrap().get_bind_group_layout(0);
            let bind_group = self.broker.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: m.buffer.as_entire_binding(),
                }],
            });
            log::info!("*** bind group created ***");

            let mut dispatch = m.used_bytes as u32 / 4 / 64; // 4: item size, 64: work group size
            if m.used_bytes as u32 / 4 % 64 > 0 {
                dispatch += 1;
            }

            {
                let mut encoder =
                    self.broker.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
                {
                    let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });
                    cpass.set_pipeline(&self.pipeline.as_ref().unwrap());
                    cpass.set_bind_group(0, &bind_group, &[]);
                    cpass.insert_debug_marker("compute collatz iterations");
                    cpass.dispatch(dispatch, 1, 1); // Number of cells to run, the (x,y,z) size of item being processed
                }


            m.buffer.unmap();

            // Submits command encoder for processing
            log::info!("*** queue submit ***");
            self.broker.queue.submit(Some(encoder.finish()));
            }
            let buffer_slice = m.buffer.slice(..);
           let buffer_future = buffer_slice.map_async(wgpu::MapMode::Read);
            log::info!("*** after map async ***");




            // Poll the device in a blocking manner so that our future resolves.
            // In an actual application, `device.poll(...)` should
            // be called in an event loop or on another thread.

           // self.broker.device.poll(wgpu::Maintain::Wait);
            //log::info!("*** after poll  ***");
            if let Ok(()) = buffer_future.await {



                log::info!("*** output submit ***");
                o(sio, 0).submit(m);

           } else {
                panic!("failed to run compute on gpu!")
            }  }

            // Returns data from buffer
            if i(sio, 0).finished() {
                io.finished = true;
            }


            Ok(())

    }
}

pub struct WgpuBuilderWasm {
    wgpu_broker: WgpuBroker,
    capacity: u64,
}

impl WgpuBuilderWasm {
    pub fn new(broker: WgpuBroker) -> WgpuBuilderWasm {
        WgpuBuilderWasm {
            wgpu_broker: broker,
            capacity: 8192,
        }
    }

    pub fn capacity(mut self, c: u64) -> WgpuBuilderWasm {
        self.capacity = c;
        self
    }

    pub fn build(self) -> Block {
        WgpuWasm::new(self.wgpu_broker, self.capacity)
    }
}
