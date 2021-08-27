use std::sync::Arc;
use vulkano::buffer::CpuAccessibleBuffer;
use vulkano::device::physical::{PhysicalDevice, PhysicalDeviceType};
use vulkano::device::Device;
use vulkano::device::DeviceExtensions;
use vulkano::device::Features;
use vulkano::device::Queue;
use vulkano::instance::Instance;
use vulkano::instance::InstanceExtensions;
use vulkano::Version;

mod d2h;
pub use d2h::ReaderD2H;
pub use d2h::WriterD2H;
pub use d2h::D2H;
mod h2d;
pub use h2d::ReaderH2D;
pub use h2d::WriterH2D;
pub use h2d::H2D;

// ================== VULKAN MESSAGE ============================
#[derive(Debug)]
pub struct BufferFull {
    pub buffer: Arc<CpuAccessibleBuffer<[u8]>>,
    pub used_bytes: usize,
}

#[derive(Debug)]
pub struct BufferEmpty {
    pub buffer: Arc<CpuAccessibleBuffer<[u8]>>,
}

// ================== VULKAN BROKER ============================
#[derive(Debug)]
pub struct Broker {
    device: Arc<Device>,
    queue: Arc<Queue>,
}

impl Broker {
    pub fn new() -> Broker {
        let instance =
            Instance::new(None, Version::V1_1, &InstanceExtensions::none(), None).unwrap();

        let device_extensions = DeviceExtensions {
            khr_storage_buffer_storage_class: true,
            ..DeviceExtensions::none()
        };
        let (physical_device, queue_family) = PhysicalDevice::enumerate(&instance)
            .filter(|&p| p.supported_extensions().is_superset_of(&device_extensions))
            .filter_map(|p| {
                p.queue_families()
                    .find(|&q| q.supports_compute())
                    .map(|q| (p, q))
            })
            .min_by_key(|(p, _)| match p.properties().device_type {
                PhysicalDeviceType::DiscreteGpu => 0,
                PhysicalDeviceType::IntegratedGpu => 1,
                PhysicalDeviceType::VirtualGpu => 2,
                PhysicalDeviceType::Cpu => 3,
                PhysicalDeviceType::Other => 4,
            })
            .unwrap();

        debug!(
            "Using device: {} (type: {:?})",
            physical_device.properties().device_name,
            physical_device.properties().device_type
        );

        let (device, mut queues) = Device::new(
            physical_device,
            &Features::none(),
            &physical_device
                .required_extensions()
                .union(&device_extensions),
            [(queue_family, 0.5)].iter().cloned(),
        )
        .unwrap();

        let queue = queues.next().unwrap();

        Broker { device, queue }
    }

    pub fn device(&self) -> Arc<Device> {
        self.device.clone()
    }

    pub fn queue(&self) -> Arc<Queue> {
        self.queue.clone()
    }
}

impl Default for Broker {
    fn default() -> Self {
        Self::new()
    }
}