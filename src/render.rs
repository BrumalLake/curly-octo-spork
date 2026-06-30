use ash::{Entry, prelude::VkResult, vk::{self, ApplicationInfo, InstanceCreateInfo}};

pub struct TriangleApplication {
	
}

impl TriangleApplication {
	pub fn render() -> VkResult<()> {
		Self::init_vulkan()?;
		Self::main_loop()?;
		Self::cleanup()?;
		Ok(())
	}

	fn init_vulkan() -> VkResult<()> {
		let entry = Entry::linked();
		let application_info = ApplicationInfo::default();
		let instance_create_info = InstanceCreateInfo::default().application_info(&application_info);
		let instance = unsafe { entry.create_instance(&instance_create_info, None).unwrap() };
		todo!()
	}

	fn main_loop() -> VkResult<()> {
		todo!()
	}

	fn cleanup() -> VkResult<()> {
		todo!()
	}
}