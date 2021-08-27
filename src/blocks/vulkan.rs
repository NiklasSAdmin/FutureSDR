use anyhow::{Context, Result};
use std::sync::Arc;
use vulkano::buffer::BufferUsage;
use vulkano::buffer::CpuAccessibleBuffer;
use vulkano::command_buffer::AutoCommandBufferBuilder;
use vulkano::command_buffer::CommandBufferUsage;
use vulkano::descriptor_set::layout::DescriptorSetLayout;
use vulkano::descriptor_set::PersistentDescriptorSet;
use vulkano::pipeline::ComputePipeline;
use vulkano::pipeline::ComputePipelineAbstract;
use vulkano::sync::{self, GpuFuture};

use crate::runtime::buffer::vulkan::Broker;
use crate::runtime::buffer::vulkan::BufferEmpty;
use crate::runtime::buffer::vulkan::ReaderH2D;
use crate::runtime::buffer::vulkan::WriterD2H;
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

mod cs {
    vulkano_shaders::shader! {
        ty: "compute",
        src: "
#version 450

layout(local_size_x = 64, local_size_y = 1, local_size_z = 1) in;

layout(set = 0, binding = 0) buffer Data {
    float data[];
} buf;

void main() {
    uint idx = gl_GlobalInvocationID.x;
    buf.data[idx] *= 12.0;
}"
    }
}

pub struct Vulkan {
    broker: Arc<Broker>,
    capacity: u64,
    pipeline: Option<Arc<ComputePipeline>>,
    layout: Option<Arc<DescriptorSetLayout>>,
}

impl Vulkan {
    pub fn new(broker: Arc<Broker>, capacity: u64) -> Block {
        Block::new_async(
            BlockMetaBuilder::new("Vulkan").build(),
            StreamIoBuilder::new()
                .add_stream_input("in", 4)
                .add_stream_output("out", 4)
                .build(),
            MessageIoBuilder::<Vulkan>::new().build(),
            Vulkan {
                broker,
                pipeline: None,
                layout: None,
                capacity,
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
impl AsyncKernel for Vulkan {
    async fn init(
        &mut self,
        sio: &mut StreamIo,
        _m: &mut MessageIo<Self>,
        _b: &mut BlockMeta,
    ) -> Result<()> {
        let input = i(sio, 0);

        for _ in 0..4u32 {
            let buffer;
            unsafe {
                buffer = CpuAccessibleBuffer::uninitialized_array(
                    self.broker.device().clone(),
                    self.capacity,
                    BufferUsage {
                        storage_buffer: true,
                        ..BufferUsage::none()
                    },
                    false,
                )?;
            }
            input.submit(BufferEmpty { buffer });
        }

        let shader = cs::Shader::load(self.broker.device())?;
        let pipeline = Arc::new(ComputePipeline::new(
            self.broker.device(),
            &shader.main_entry_point(),
            &(),
            None,
        )?);
        self.pipeline = Some(pipeline);
        self.layout = Some(
            self.pipeline
                .as_ref()
                .context("no pipeline")?
                .layout()
                .descriptor_set_layouts()
                .get(0)
                .context("no desc layout")?
                .clone(),
        );

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
            debug!("vulkan: forwarding buff from output to input");
            i(sio, 0).submit(m);
        }

        for m in i(sio, 0).buffers().drain(..) {
            debug!("vulkan block: launching full buffer");

            let set = Arc::new(
                PersistentDescriptorSet::start(self.layout.as_ref().context("no layout")?.clone())
                    .add_buffer(m.buffer.clone())?
                    .build()?,
            );

            let mut dispatch = m.used_bytes as u32 / 4 / 64; // 4: item size, 64: work group size
            if m.used_bytes as u32 / 4 % 64 > 0 {
                dispatch += 1;
            }

            let mut builder = AutoCommandBufferBuilder::primary(
                self.broker.device().clone(),
                self.broker.queue().family(),
                CommandBufferUsage::OneTimeSubmit,
            )?;

            builder.dispatch(
                [dispatch, 1, 1],
                self.pipeline.as_ref().context("no pipeline")?.clone(),
                set.clone(),
                (),
            )?;
            let command_buffer = builder.build()?;

            let future = sync::now(self.broker.device().clone())
                .then_execute(self.broker.queue().clone(), command_buffer)
                .unwrap()
                .then_signal_fence_and_flush()?;

            future.wait(None)?;

            debug!("vulkan block: forwarding processed buffer");
            o(sio, 0).submit(m);
        }

        if i(sio, 0).finished() {
            io.finished = true;
        }

        Ok(())
    }
}

pub struct VulkanBuilder {
    broker: Arc<Broker>,
    capacity: u64,
}

impl VulkanBuilder {
    pub fn new(broker: Arc<Broker>) -> VulkanBuilder {
        VulkanBuilder {
            broker,
            capacity: 8192,
        }
    }

    pub fn capacity(mut self, c: u64) -> VulkanBuilder {
        self.capacity = c;
        self
    }

    pub fn build(self) -> Block {
        Vulkan::new(self.broker, self.capacity)
    }
}