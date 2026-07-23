use std::{
	borrow::Cow,
	collections::HashSet,
	ffi::{CStr, c_char},
	fs::File,
	path::Path,
	ptr::null_mut,
};

use ash::{
	Device, Entry, Instance,
	ext::debug_utils,
	khr,
	vk::{self, Handle},
};
use glfw::{
	GLFW_CLIENT_API, GLFW_FALSE, GLFW_NO_API, GLFW_RESIZABLE, GLFWwindow, glfwCreateWindow,
	glfwCreateWindowSurface, glfwDestroyWindow, glfwGetFramebufferSize,
	glfwGetRequiredInstanceExtensions, glfwInit, glfwPollEvents, glfwTerminate, glfwWindowHint,
	glfwWindowShouldClose,
};

const ENABLE_VALIDATION_LAYERS: bool = cfg!(debug_assertions);
const VALIDATION_LAYERS: &[&CStr] = &[c"VK_LAYER_KHRONOS_validation"];

const WIDTH: i32 = 800;
const HEIGHT: i32 = 600;

#[derive(Default)]
pub struct TriangleApplication {
	window: *mut GLFWwindow,

	// default is linked
	entry: Entry,
	instance: Option<Instance>,
	debug_instance: Option<debug_utils::Instance>,
	surface_instance: Option<khr::surface::Instance>,
	debug_messenger: vk::DebugUtilsMessengerEXT,
	physical_device: vk::PhysicalDevice,
	device: Option<Device>,
	queue: Option<vk::Queue>,
	surface: vk::SurfaceKHR,
	device_swapchain_functions: Option<khr::swapchain::Device>,
	swapchain: vk::SwapchainKHR,
	graphics_queue_index: u32,
	surface_format: vk::SurfaceFormatKHR,
	extent: vk::Extent2D,
	swapchain_image: Vec<vk::Image>,
	image_views: Vec<vk::ImageView>,
	shader_module: vk::ShaderModule,
	pipeline_layout: vk::PipelineLayout,
	pipeline: vk::Pipeline,
	command_pool: vk::CommandPool,
	command_buffer: vk::CommandBuffer,
	present_complete_sem: vk::Semaphore,
	render_complete_sem: vk::Semaphore,
	draw_fence: vk::Fence,
}

impl TriangleApplication {
	pub fn render(mut self) {
		self.init_window();
		self.init_vulkan();
		self.main_loop();
	}

	fn init_window(&mut self) {
		unsafe {
			glfwInit();

			glfwWindowHint(GLFW_CLIENT_API, GLFW_NO_API);
			glfwWindowHint(GLFW_RESIZABLE, GLFW_FALSE);

			self.window =
				glfwCreateWindow(WIDTH, HEIGHT, c"Vulkan".as_ptr(), null_mut(), null_mut());
		}
	}

	fn init_vulkan(&mut self) {
		self.create_instance();
		self.setup_debug_messenger();
		self.create_surface();
		self.pick_physical_device();
		self.create_logical_device();
		self.create_swapchain();
		self.create_imageviews();
		self.create_graphics_pipeline();
		self.create_command_pool();
		self.create_command_buffer();
		self.create_sync_objects();
	}

	fn create_instance(&mut self) {
		let application_info = vk::ApplicationInfo::default()
			.application_name(c"Hello Triangle")
			.application_version(0)
			.engine_name(c"No Engine")
			.engine_version(1)
			.api_version(vk::API_VERSION_1_3);

		// verify required layers
		let mut required_layers: Vec<*const c_char> = vec![];
		if ENABLE_VALIDATION_LAYERS {
			required_layers.extend(VALIDATION_LAYERS.iter().map(|l| l.as_ptr()));
		}

		let layer_properties = unsafe { self.entry.enumerate_instance_layer_properties().unwrap() };

		let layer_names: HashSet<_> = layer_properties
			.iter()
			.map(|l| l.layer_name_as_c_str().unwrap())
			.collect();

		for &required_layer in required_layers.iter() {
			assert!(
				layer_names.contains(unsafe { CStr::from_ptr(required_layer) }),
				"{required_layer:?} not found in {layer_names:#?}"
			);
		}

		let required_instance_extensions = Self::required_instance_extensions();

		// verify required extensions are all available
		let extension_properties = unsafe {
			self.entry
				.enumerate_instance_extension_properties(None)
				.unwrap()
		};

		let extension_properties: HashSet<_> = extension_properties
			.iter()
			.map(|e| e.extension_name_as_c_str().unwrap())
			.collect();

		for &required_extension in required_instance_extensions.iter() {
			let required_extension = unsafe { CStr::from_ptr(required_extension) };
			assert!(
				extension_properties.contains(&required_extension),
				"{required_extension:?} not found in {extension_properties:#?}",
			);
		}

		let instance_create_info = vk::InstanceCreateInfo::default()
			.enabled_extension_names(&required_instance_extensions)
			.enabled_layer_names(&required_layers)
			.application_info(&application_info);

		self.instance = Some(unsafe {
			self.entry
				.create_instance(&instance_create_info, None)
				.unwrap()
		});
	}

	fn required_instance_extensions() -> Vec<*const c_char> {
		let mut res: Vec<*const c_char>;

		let mut glfw_extension_count = 0;
		let glfw_extensions =
			unsafe { glfwGetRequiredInstanceExtensions(&mut glfw_extension_count) };
		assert!(!glfw_extensions.is_null());

		res = unsafe {
			std::slice::from_raw_parts(glfw_extensions, glfw_extension_count.try_into().unwrap())
		}
		.iter()
		.map(|&e| e)
		.collect();

		if ENABLE_VALIDATION_LAYERS {
			res.push(vk::EXT_DEBUG_UTILS_NAME.as_ptr());
		}

		res
	}

	fn setup_debug_messenger(&mut self) {
		let debug_instance =
			debug_utils::Instance::new(&self.entry, self.instance.as_ref().unwrap());

		let debug_messenger_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
			.message_severity(
				vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
					| vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
			)
			.message_type(
				vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
					| vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
					| vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION,
			)
			.pfn_user_callback(Some(debug_callback));

		self.debug_messenger =
			unsafe { debug_instance.create_debug_utils_messenger(&debug_messenger_info, None) }
				.unwrap();

		self.debug_instance = Some(debug_instance);
	}

	fn create_surface(&mut self) {
		assert!(
			unsafe {
				glfwCreateWindowSurface(
					std::ptr::without_provenance_mut(
						self.instance.as_ref().unwrap().handle().as_raw() as usize,
					),
					self.window,
					std::ptr::null(),
					std::ptr::from_mut(&mut self.surface) as *mut _,
				) == vk::Result::SUCCESS.as_raw()
			},
			"failed to create window surface"
		);
	}

	fn pick_physical_device(&mut self) {
		let physical_devices =
			unsafe { self.instance.as_ref().unwrap().enumerate_physical_devices() }.unwrap();
		assert!(
			!physical_devices.is_empty(),
			"failed to find GPU with Vulkan support"
		);

		let (mut high_score, mut best_device) = (0, None);

		for device in physical_devices {
			if let Ok(score) = self.score_device(device)
				&& score >= high_score
			{
				high_score = score;
				best_device = Some(device);
			}
		}

		self.physical_device = best_device.expect("no suitable GPU found");
	}

	fn score_device(&self, device: vk::PhysicalDevice) -> Result<u8, ()> {
		let mut score = 0;
		let instance = self.instance.as_ref().unwrap();
		let device_properties = unsafe { instance.get_physical_device_properties(device) };

		// check if device is suitable
		// supports vulkan api 1.3
		if !(device_properties.api_version >= vk::API_VERSION_1_3) {
			return Err(());
		}

		// graphics command queue support
		if {
			let queue_families =
				unsafe { instance.get_physical_device_queue_family_properties(device) };
			!queue_families
				.into_iter()
				.any(|queue_family| queue_family.queue_flags.contains(vk::QueueFlags::GRAPHICS))
		} {
			return Err(());
		}

		// supports all required extensions
		if {
			let required_device_extensions: Vec<*const c_char> = [vk::KHR_SWAPCHAIN_NAME]
				.into_iter()
				.map(|extension_name| extension_name.as_ptr())
				.collect();
			let device_supported_extensions = unsafe {
				instance
					.enumerate_device_extension_properties(device)
					.unwrap()
			};
			let device_supported_extensions: HashSet<_> = device_supported_extensions
				.iter()
				.map(|extension| extension.extension_name_as_c_str().unwrap())
				.collect();

			let mut missing_extension = false;
			for required_extension in required_device_extensions {
				let required_extension = unsafe { CStr::from_ptr(required_extension) };
				if !device_supported_extensions.contains(required_extension) {
					missing_extension = true;
					break;
				}
			}

			missing_extension
		} {
			return Err(());
		}

		// supports all required features
		if {
			let mut vk11 = vk::PhysicalDeviceVulkan11Features::default();
			let mut vk13 = vk::PhysicalDeviceVulkan13Features::default();
			let mut extended_dynamic_state =
				vk::PhysicalDeviceExtendedDynamicStateFeaturesEXT::default();

			let mut device_features = vk::PhysicalDeviceFeatures2::default()
				.push_next(&mut extended_dynamic_state)
				.push_next(&mut vk13)
				.push_next(&mut vk11);

			unsafe { instance.get_physical_device_features2(device, &mut device_features) };

			!(device_features.features.geometry_shader == vk::TRUE
				&& vk11.shader_draw_parameters == vk::TRUE
				&& vk13.dynamic_rendering == vk::TRUE
				&& vk13.synchronization2 == vk::TRUE
				&& extended_dynamic_state.extended_dynamic_state == vk::TRUE)
		} {
			return Err(());
		}

		if device_properties.device_type == vk::PhysicalDeviceType::INTEGRATED_GPU {
			score += 1;
		}

		Ok(score)
	}

	fn create_logical_device(&mut self) {
		let instance = self.instance.as_ref().unwrap();
		let surface_instance = khr::surface::Instance::new(&self.entry, instance);

		let queue_family_properties =
			unsafe { instance.get_physical_device_queue_family_properties(self.physical_device) };
		self.graphics_queue_index = queue_family_properties
			.iter()
			.enumerate()
			.find(|(i, properties)| {
				properties.queue_flags.contains(vk::QueueFlags::GRAPHICS)
					&& unsafe {
						surface_instance.get_physical_device_surface_support(
							self.physical_device,
							*i as u32,
							self.surface,
						)
					}
					.unwrap()
			})
			.unwrap()
			.0
			.try_into()
			.unwrap();

		let queue_priority = &[0.5f32];

		let device_queue_infos = &[vk::DeviceQueueCreateInfo::default()
			.queue_priorities(queue_priority)
			.queue_family_index(self.graphics_queue_index)];

		let mut vk11 = vk::PhysicalDeviceVulkan11Features::default().shader_draw_parameters(true);
		let mut vk13 = vk::PhysicalDeviceVulkan13Features::default()
			.dynamic_rendering(true)
			.synchronization2(true);
		let mut extended_dynamic_state =
			vk::PhysicalDeviceExtendedDynamicStateFeaturesEXT::default()
				.extended_dynamic_state(true);

		let mut feature_chain = vk::PhysicalDeviceFeatures2::default()
			.push_next(&mut extended_dynamic_state)
			.push_next(&mut vk13)
			.push_next(&mut vk11);

		let required_device_extensions: Vec<*const c_char> = [vk::KHR_SWAPCHAIN_NAME]
			.iter()
			.map(|extension_name| extension_name.as_ptr())
			.collect();

		let device_create_info = vk::DeviceCreateInfo::default()
			.push_next(&mut feature_chain)
			.queue_create_infos(device_queue_infos)
			.enabled_extension_names(&required_device_extensions);

		let device = unsafe {
			instance
				.create_device(self.physical_device, &device_create_info, None)
				.unwrap()
		};

		self.queue = Some(unsafe { device.get_device_queue(self.graphics_queue_index, 0) });

		self.surface_instance = Some(surface_instance);
		self.device = Some(device);
	}

	fn create_swapchain(&mut self) {
		let surface_instance = self.surface_instance.as_ref().unwrap();

		let surface_capabilities = unsafe {
			surface_instance
				.get_physical_device_surface_capabilities(self.physical_device, self.surface)
		}
		.unwrap();
		let extent = self.choose_extent(&surface_capabilities);
		let min_image_count = Self::choose_min_image_count(&surface_capabilities);

		let available_formats = unsafe {
			surface_instance.get_physical_device_surface_formats(self.physical_device, self.surface)
		}
		.unwrap();
		let surface_format = Self::choose_surface_format(&available_formats);

		let available_presentmodes = unsafe {
			surface_instance
				.get_physical_device_surface_present_modes(self.physical_device, self.surface)
		}
		.unwrap();
		let present_mode = Self::choose_present_mode(&available_presentmodes);

		let create_info = vk::SwapchainCreateInfoKHR::default()
			.surface(self.surface)
			.min_image_count(min_image_count)
			.image_format(surface_format.format)
			.image_color_space(surface_format.color_space)
			.image_extent(extent)
			.image_array_layers(1)
			.image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
			.image_sharing_mode(vk::SharingMode::EXCLUSIVE)
			.pre_transform(surface_capabilities.current_transform)
			.composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
			.present_mode(present_mode)
			.clipped(true);

		let device_swapchain_functions = khr::swapchain::Device::new(
			self.instance.as_ref().unwrap(),
			self.device.as_ref().unwrap(),
		);

		self.swapchain =
			unsafe { device_swapchain_functions.create_swapchain(&create_info, None) }.unwrap();
		self.swapchain_image =
			unsafe { device_swapchain_functions.get_swapchain_images(self.swapchain) }.unwrap();
		self.device_swapchain_functions = Some(device_swapchain_functions);
		self.extent = extent;
		self.surface_format = surface_format;
	}

	fn choose_surface_format(available_formats: &[vk::SurfaceFormatKHR]) -> vk::SurfaceFormatKHR {
		assert!(!available_formats.is_empty());

		*available_formats
			.iter()
			.find(|format| {
				format.format == vk::Format::B8G8R8A8_SRGB
					&& format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
			})
			.unwrap_or_else(|| &available_formats[0])
	}

	fn choose_present_mode(available_modes: &[vk::PresentModeKHR]) -> vk::PresentModeKHR {
		debug_assert!(
			available_modes
				.iter()
				.any(|&mode| mode == vk::PresentModeKHR::FIFO)
		);

		if available_modes
			.iter()
			.any(|&mode| mode == vk::PresentModeKHR::MAILBOX)
		{
			vk::PresentModeKHR::MAILBOX
		} else {
			vk::PresentModeKHR::FIFO
		}
	}

	fn choose_extent(&self, capabilities: &vk::SurfaceCapabilitiesKHR) -> vk::Extent2D {
		if capabilities.current_extent.width != u32::MAX {
			return capabilities.current_extent;
		}

		let mut width = -1;
		let mut height = -1;
		unsafe {
			glfwGetFramebufferSize(self.window, &mut width, &mut height);
		}
		let width: u32 = width.try_into().unwrap();
		let height: u32 = height.try_into().unwrap();

		vk::Extent2D {
			width: width.clamp(
				capabilities.min_image_extent.width,
				capabilities.max_image_extent.width,
			),
			height: height.clamp(
				capabilities.min_image_extent.height,
				capabilities.max_image_extent.height,
			),
		}
	}

	fn choose_min_image_count(capabilities: &vk::SurfaceCapabilitiesKHR) -> u32 {
		let mut min_image_count = capabilities.min_image_count.max(3);

		if 0 < capabilities.max_image_count
			&& capabilities.max_image_count < capabilities.min_image_count
		{
			min_image_count = capabilities.max_image_count;
		}

		min_image_count
	}

	fn create_imageviews(&mut self) {
		let mut create_info = vk::ImageViewCreateInfo::default()
			.view_type(vk::ImageViewType::TYPE_2D)
			.format(self.surface_format.format)
			.subresource_range(vk::ImageSubresourceRange {
				aspect_mask: vk::ImageAspectFlags::COLOR,
				base_mip_level: 0,
				level_count: 1,
				base_array_layer: 0,
				layer_count: 1,
			});

		self.image_views.reserve_exact(self.swapchain_image.len());

		let device = self.device.as_ref().unwrap();
		for &image in self.swapchain_image.iter() {
			create_info = create_info.image(image);
			self.image_views
				.push(unsafe { device.create_image_view(&create_info, None) }.unwrap());
		}
	}

	fn create_graphics_pipeline(&mut self) {
		let shader_module = self.create_shader_module(concat!(env!("OUT_DIR"), "/shader.spv"));

		let vertex_stage_info = vk::PipelineShaderStageCreateInfo::default()
			.stage(vk::ShaderStageFlags::VERTEX)
			.module(shader_module)
			.name(c"vertex_main");

		let fragment_stage_info = vk::PipelineShaderStageCreateInfo::default()
			.stage(vk::ShaderStageFlags::FRAGMENT)
			.module(shader_module)
			.name(c"fragment_main");

		self.shader_module = shader_module;

		let stages = &[vertex_stage_info, fragment_stage_info];

		let vertex_input = vk::PipelineVertexInputStateCreateInfo::default();

		let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
			.topology(vk::PrimitiveTopology::TRIANGLE_LIST);

		let viewports = &[vk::Viewport {
			x: 0.0,
			y: 0.0,
			width: WIDTH as f32,
			height: HEIGHT as f32,
			min_depth: 0.0,
			max_depth: 1.0,
		}];

		let scissors = &[vk::Rect2D {
			offset: vk::Offset2D { x: 0, y: 0 },
			extent: self.extent,
		}];

		let dynamic_states = &[vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
		let dynamic_state_info =
			vk::PipelineDynamicStateCreateInfo::default().dynamic_states(dynamic_states);

		let viewport_state = vk::PipelineViewportStateCreateInfo::default()
			.viewports(viewports)
			.scissors(scissors);

		let rasterizer = vk::PipelineRasterizationStateCreateInfo::default()
			.depth_clamp_enable(false)
			.rasterizer_discard_enable(false)
			.polygon_mode(vk::PolygonMode::FILL)
			.cull_mode(vk::CullModeFlags::BACK)
			.front_face(vk::FrontFace::CLOCKWISE)
			.depth_bias_enable(false)
			.line_width(1.0);

		let multisampling = vk::PipelineMultisampleStateCreateInfo::default()
			.rasterization_samples(vk::SampleCountFlags::TYPE_1)
			.sample_shading_enable(false);

		let color_blend_attachments = &[vk::PipelineColorBlendAttachmentState::default()
			.blend_enable(false)
			.color_write_mask(
				vk::ColorComponentFlags::R
					| vk::ColorComponentFlags::G
					| vk::ColorComponentFlags::G
					| vk::ColorComponentFlags::A,
			)];

		let color_blend =
			vk::PipelineColorBlendStateCreateInfo::default().attachments(color_blend_attachments);

		let pipeline_layout_info = vk::PipelineLayoutCreateInfo::default();
		self.pipeline_layout = unsafe {
			self.device
				.as_ref()
				.unwrap()
				.create_pipeline_layout(&pipeline_layout_info, None)
		}
		.unwrap();

		let color_attachment_formats = &[self.surface_format.format];

		let mut pipeline_rendering_info = vk::PipelineRenderingCreateInfo::default()
			.color_attachment_formats(color_attachment_formats);

		let create_infos = &[vk::GraphicsPipelineCreateInfo::default()
			.stages(stages)
			.vertex_input_state(&vertex_input)
			.input_assembly_state(&input_assembly)
			.viewport_state(&viewport_state)
			.rasterization_state(&rasterizer)
			.multisample_state(&multisampling)
			.color_blend_state(&color_blend)
			.dynamic_state(&dynamic_state_info)
			.layout(self.pipeline_layout)
			.push_next(&mut pipeline_rendering_info)];

		self.pipeline = unsafe {
			self.device.as_ref().unwrap().create_graphics_pipelines(
				vk::PipelineCache::null(),
				create_infos,
				None,
			)
		}
		.unwrap()[0];
	}

	fn create_shader_module<P: AsRef<Path>>(&self, shader_path: P) -> vk::ShaderModule {
		let device = self.device.as_ref().unwrap();

		let mut file = File::open(shader_path).unwrap();
		let shader_code = ash::util::read_spv(&mut file).unwrap();

		let create_info = vk::ShaderModuleCreateInfo::default().code(&shader_code);
		unsafe { device.create_shader_module(&create_info, None) }.unwrap()
	}

	fn create_command_pool(&mut self) {
		let create_info = vk::CommandPoolCreateInfo::default()
			.flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
			.queue_family_index(self.graphics_queue_index);

		self.command_pool = unsafe {
			self.device
				.as_ref()
				.unwrap()
				.create_command_pool(&create_info, None)
		}
		.unwrap();
	}

	fn create_command_buffer(&mut self) {
		let alloc_info = vk::CommandBufferAllocateInfo::default()
			.command_pool(self.command_pool)
			.level(vk::CommandBufferLevel::PRIMARY)
			.command_buffer_count(1);

		self.command_buffer = unsafe {
			self.device
				.as_ref()
				.unwrap()
				.allocate_command_buffers(&alloc_info)
		}
		.unwrap()[0];
	}

	fn record_command_buffer(&self, image_index: u32) {
		let device = self.device.as_ref().unwrap();

		unsafe {
			device.begin_command_buffer(self.command_buffer, &vk::CommandBufferBeginInfo::default())
		}
		.unwrap();

		self.transition_image_layout(
			vk::ImageLayout::UNDEFINED,
			vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
			vk::AccessFlags2::empty(),
			vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
			vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
			vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
			image_index,
		);

		let color_attachments = &[vk::RenderingAttachmentInfo::default()
			.image_view(self.image_views[image_index as usize])
			.image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
			.load_op(vk::AttachmentLoadOp::CLEAR)
			.store_op(vk::AttachmentStoreOp::STORE)
			.clear_value(vk::ClearValue {
				color: vk::ClearColorValue {
					float32: [0.0, 0.0, 0.0, 1.0],
				},
			})];

		let rendering_info = vk::RenderingInfo::default()
			.render_area(vk::Rect2D {
				offset: vk::Offset2D { x: 0, y: 0 },
				extent: self.extent,
			})
			.layer_count(1)
			.color_attachments(color_attachments);

		unsafe {
			device.cmd_begin_rendering(self.command_buffer, &rendering_info);

			device.cmd_bind_pipeline(
				self.command_buffer,
				vk::PipelineBindPoint::GRAPHICS,
				self.pipeline,
			);

			device.cmd_set_viewport(
				self.command_buffer,
				0,
				&[vk::Viewport {
					x: 0.0,
					y: 0.0,
					width: self.extent.width as f32,
					height: self.extent.height as f32,
					min_depth: 0.0,
					max_depth: 0.0,
				}],
			);
			device.cmd_set_scissor(
				self.command_buffer,
				0,
				&[vk::Rect2D {
					offset: vk::Offset2D { x: 0, y: 0 },
					extent: self.extent,
				}],
			);

			device.cmd_draw(self.command_buffer, 3, 1, 0, 0);

			device.cmd_end_rendering(self.command_buffer);
		}

		self.transition_image_layout(
			vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
			vk::ImageLayout::PRESENT_SRC_KHR,
			vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
			vk::AccessFlags2::empty(),
			vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
			vk::PipelineStageFlags2::BOTTOM_OF_PIPE,
			image_index,
		);

		unsafe {
			device.end_command_buffer(self.command_buffer).unwrap();
		}
	}

	fn transition_image_layout(
		&self,
		old_layout: vk::ImageLayout,
		new_layout: vk::ImageLayout,
		src_access_mask: vk::AccessFlags2,
		dst_access_mask: vk::AccessFlags2,
		src_stage_mask: vk::PipelineStageFlags2,
		dst_stage_mask: vk::PipelineStageFlags2,
		image_index: u32,
	) {
		let barrier = &[vk::ImageMemoryBarrier2::default()
			.old_layout(old_layout)
			.new_layout(new_layout)
			.src_access_mask(src_access_mask)
			.dst_access_mask(dst_access_mask)
			.src_stage_mask(src_stage_mask)
			.dst_stage_mask(dst_stage_mask)
			.image(self.swapchain_image[image_index as usize])
			.subresource_range(vk::ImageSubresourceRange {
				aspect_mask: vk::ImageAspectFlags::COLOR,
				base_mip_level: 0,
				level_count: 1,
				base_array_layer: 0,
				layer_count: 1,
			})];

		let dependency_info = vk::DependencyInfo::default().image_memory_barriers(barrier);

		unsafe {
			self.device
				.as_ref()
				.unwrap()
				.cmd_pipeline_barrier2(self.command_buffer, &dependency_info)
		};
	}

	fn create_sync_objects(&mut self) {
		let device = self.device.as_ref().unwrap();
		let sem_create_info = vk::SemaphoreCreateInfo::default();
		unsafe {
			self.present_complete_sem = device.create_semaphore(&sem_create_info, None).unwrap();
			self.render_complete_sem = device.create_semaphore(&sem_create_info, None).unwrap();
			self.draw_fence = device
				.create_fence(
					&vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
					None,
				)
				.unwrap();
		}
	}

	fn main_loop(&self) {
		unsafe {
			while glfwWindowShouldClose(self.window) == GLFW_FALSE {
				glfwPollEvents();
				self.draw_frame();
			}

			self.device.as_ref().unwrap().device_wait_idle().unwrap();
		}
	}

	fn draw_frame(&self) {
		let device = self.device.as_ref().unwrap();
		let device_swapchain_functions = self.device_swapchain_functions.as_ref().unwrap();

		let fences = &[self.draw_fence];
		unsafe {
			device.wait_for_fences(fences, false, u64::MAX).unwrap();
			device.reset_fences(fences).unwrap();
		}

		let (image_index, _) = unsafe {
			device_swapchain_functions.acquire_next_image(
				self.swapchain,
				u64::MAX,
				self.present_complete_sem,
				vk::Fence::null(),
			)
		}
		.unwrap();
		self.record_command_buffer(image_index);

		let wait_semaphores = &[self.present_complete_sem];
		let wait_stages = &[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
		let signal_semaphores = &[self.render_complete_sem];
		let command_buffers = &[self.command_buffer];
		let submits = &[vk::SubmitInfo::default()
			.wait_semaphores(wait_semaphores)
			.wait_dst_stage_mask(wait_stages)
			.signal_semaphores(signal_semaphores)
			.command_buffers(command_buffers)];

		unsafe {
			device
				.queue_submit(*self.queue.as_ref().unwrap(), submits, self.draw_fence)
				.unwrap();
		}

		let wait_semaphores = &[self.render_complete_sem];
		let swapchains = &[self.swapchain];
		let image_indices = &[image_index];
		let present_info = vk::PresentInfoKHR::default()
			.wait_semaphores(wait_semaphores)
			.swapchains(swapchains)
			.image_indices(image_indices);
		unsafe {
			device_swapchain_functions
				.queue_present(*self.queue.as_ref().unwrap(), &present_info)
				.unwrap();
		}
	}
}

impl Drop for TriangleApplication {
	fn drop(&mut self) {
		unsafe {
			let device = self.device.as_ref().unwrap();
			device.destroy_fence(self.draw_fence, None);
			device.destroy_semaphore(self.present_complete_sem, None);
			device.destroy_semaphore(self.render_complete_sem, None);
			let command_buffer = &[self.command_buffer];
			device.free_command_buffers(self.command_pool, command_buffer);
			device.destroy_command_pool(self.command_pool, None);
			device.destroy_pipeline(self.pipeline, None);
			device.destroy_pipeline_layout(self.pipeline_layout, None);
			device.destroy_shader_module(self.shader_module, None);
			for &imageview in self.image_views.iter() {
				device.destroy_image_view(imageview, None);
			}
			self.device_swapchain_functions
				.as_ref()
				.unwrap()
				.destroy_swapchain(self.swapchain, None);
			self.surface_instance
				.as_ref()
				.unwrap()
				.destroy_surface(self.surface, None);
			self.debug_instance
				.as_ref()
				.unwrap()
				.destroy_debug_utils_messenger(self.debug_messenger, None);
			device.destroy_device(None);
			self.instance.as_ref().unwrap().destroy_instance(None);
		}

		unsafe {
			glfwDestroyWindow(self.window);
			glfwTerminate();
		}
	}
}

unsafe extern "system" fn debug_callback(
	message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
	message_type: vk::DebugUtilsMessageTypeFlagsEXT,
	p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
	_user_data: *mut std::os::raw::c_void,
) -> vk::Bool32 {
	let callback_data = unsafe { *p_callback_data };

	let message = if callback_data.p_message.is_null() {
		Cow::Borrowed("")
	} else {
		unsafe { CStr::from_ptr(callback_data.p_message) }.to_string_lossy()
	};

	let message_id_name = if callback_data.p_message_id_name.is_null() {
		Cow::Borrowed("")
	} else {
		unsafe { CStr::from_ptr(callback_data.p_message_id_name) }.to_string_lossy()
	};

	let message_id = callback_data.message_id_number;

	// consider coloring off severity
	eprintln!(
		"validation layer type: {message_type:?} {message_severity:?} {message_id_name} {message_id} \n\
			message: {message}\n"
	);

	vk::FALSE
}
