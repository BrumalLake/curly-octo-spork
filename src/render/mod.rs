mod vertex;
use vertex::Vertex;

use std::{
	array,
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
	GLFW_CLIENT_API, GLFW_FALSE, GLFW_NO_API, GLFWwindow, glfwCreateWindow,
	glfwCreateWindowSurface, glfwDestroyWindow, glfwGetFramebufferSize,
	glfwGetRequiredInstanceExtensions, glfwGetWindowUserPointer, glfwInit, glfwPollEvents,
	glfwSetFramebufferSizeCallback, glfwSetWindowUserPointer, glfwTerminate, glfwWaitEvents,
	glfwWindowHint, glfwWindowShouldClose,
};

const ENABLE_VALIDATION_LAYERS: bool = cfg!(debug_assertions);
const VALIDATION_LAYERS: &[&CStr] = &[c"VK_LAYER_KHRONOS_validation"];

const WIDTH: i32 = 800;
const HEIGHT: i32 = 600;

const MAX_FRAMES_IN_FLIGHT: usize = 2;

pub struct TriangleApplication {
	window: *mut GLFWwindow,

	entry: Entry,
	instance: Instance,
	debug_instance: debug_utils::Instance,
	surface_instance: khr::surface::Instance,
	debug_messenger: vk::DebugUtilsMessengerEXT,
	physical_device: vk::PhysicalDevice,
	device: Device,
	graphics_queue: vk::Queue,
	surface: vk::SurfaceKHR,
	device_swapchain_functions: khr::swapchain::Device,
	swapchain: vk::SwapchainKHR,
	graphics_queue_index: u32,
	surface_format: vk::SurfaceFormatKHR,
	extent: vk::Extent2D,
	swapchain_images: Vec<vk::Image>,
	swapchain_image_views: Vec<vk::ImageView>,
	shader_module: vk::ShaderModule,
	pipeline_layout: vk::PipelineLayout,
	pipeline: vk::Pipeline,
	command_pool: vk::CommandPool,
	command_buffers: Vec<vk::CommandBuffer>,
	draw_fences: Vec<vk::Fence>,
	present_complete_sems: Vec<vk::Semaphore>,
	render_complete_sems: Vec<vk::Semaphore>,
	vertex_buffer: vk::Buffer,
	vertex_buffer_memory: vk::DeviceMemory,
	// must be on the heap because stack addresses are not consistent with FFI
	// modified in glfw callback
	framebuffer_resized: Box<bool>,
}

impl TriangleApplication {
	const VERTICES: &[Vertex] = &Vertex::create_vertices([
		([0.0, -0.5], [1.0, 0.0, 0.0]),
		([0.5, 0.5], [0.0, 1.0, 0.0]),
		([-0.5, 0.5], [0.0, 0.0, 1.0]),
	]);

	pub fn new() -> Self {
		Self::default()
	}

	pub fn render() {
		let mut application = Self::new();
		application.main_loop();
	}

	fn init_window() -> *mut GLFWwindow {
		unsafe {
			glfwInit();

			glfwWindowHint(GLFW_CLIENT_API, GLFW_NO_API);

			let window =
				glfwCreateWindow(WIDTH, HEIGHT, c"Vulkan".as_ptr(), null_mut(), null_mut());

			window
		}
	}

	fn init_vulkan(window: *mut GLFWwindow) -> Self {
		let entry = Entry::linked();
		let instance = Self::create_instance(&entry);
		let (debug_messenger, debug_instance) = Self::setup_debug_messenger(&entry, &instance);
		let surface = Self::create_surface(&instance, window);
		let physical_device = Self::pick_physical_device(&instance);
		let (device, graphics_queue, graphics_queue_index, surface_instance) =
			Self::create_logical_device(&entry, &instance, physical_device, surface);
		let (swapchain, swapchain_images, device_swapchain_functions, extent, surface_format) =
			Self::create_swapchain(
				&instance,
				&surface_instance,
				&device,
				physical_device,
				surface,
				window,
			);
		let swapchain_image_views =
			Self::create_imageviews(&device, surface_format, &swapchain_images);
		let (pipeline, pipeline_layout, shader_module) =
			Self::create_graphics_pipeline(&device, surface_format, extent);
		let command_pool = Self::create_command_pool(&device, graphics_queue_index);
		let command_buffers = Self::create_command_buffers(&device, command_pool);
		let (vertex_buffer, vertex_buffer_memory) =
			Self::create_vertex_buffer(&instance, &device, physical_device);
		let (draw_fences, present_complete_sems, render_complete_sems) =
			Self::create_sync_objects(&device, &swapchain_images);

		Self {
			window,
			entry,
			instance,
			debug_instance,
			surface_instance,
			debug_messenger,
			physical_device,
			device,
			graphics_queue,
			surface,
			device_swapchain_functions,
			swapchain,
			graphics_queue_index,
			surface_format,
			extent,
			swapchain_images,
			swapchain_image_views,
			shader_module,
			pipeline_layout,
			pipeline,
			command_pool,
			command_buffers,
			draw_fences,
			present_complete_sems,
			render_complete_sems,
			vertex_buffer,
			vertex_buffer_memory,
			framebuffer_resized: Box::new(false),
		}
	}

	fn create_instance(entry: &Entry) -> Instance {
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

		let layer_properties = unsafe { entry.enumerate_instance_layer_properties().unwrap() };

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
		let extension_properties =
			unsafe { entry.enumerate_instance_extension_properties(None).unwrap() };

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

		unsafe { entry.create_instance(&instance_create_info, None).unwrap() }
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

	fn setup_debug_messenger(
		entry: &Entry,
		instance: &Instance,
	) -> (vk::DebugUtilsMessengerEXT, debug_utils::Instance) {
		let debug_instance = debug_utils::Instance::new(entry, instance);

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

		(
			unsafe { debug_instance.create_debug_utils_messenger(&debug_messenger_info, None) }
				.unwrap(),
			debug_instance,
		)
	}

	fn create_surface(instance: &Instance, window: *mut GLFWwindow) -> vk::SurfaceKHR {
		let mut surface = vk::SurfaceKHR::null();

		assert!(
			unsafe {
				glfwCreateWindowSurface(
					std::ptr::without_provenance_mut(instance.handle().as_raw() as usize),
					window,
					std::ptr::null(),
					std::ptr::from_mut(&mut surface) as *mut _,
				) == vk::Result::SUCCESS.as_raw()
			},
			"failed to create window surface"
		);

		surface
	}

	fn pick_physical_device(instance: &Instance) -> vk::PhysicalDevice {
		let physical_devices = unsafe { instance.enumerate_physical_devices() }.unwrap();
		assert!(
			!physical_devices.is_empty(),
			"failed to find GPU with Vulkan support"
		);

		let (mut high_score, mut best_device) = (0, None);

		for device in physical_devices {
			if let Ok(score) = Self::score_device(instance, device)
				&& score >= high_score
			{
				high_score = score;
				best_device = Some(device);
			}
		}

		best_device.expect("no suitable GPU found")
	}

	fn score_device(instance: &Instance, device: vk::PhysicalDevice) -> Result<u8, ()> {
		let mut score = 0;
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

	fn create_logical_device(
		entry: &Entry,
		instance: &Instance,
		physical_device: vk::PhysicalDevice,
		surface: vk::SurfaceKHR,
	) -> (Device, vk::Queue, u32, khr::surface::Instance) {
		let surface_instance = khr::surface::Instance::new(entry, instance);
		let graphics_queue_index;

		let queue_family_properties =
			unsafe { instance.get_physical_device_queue_family_properties(physical_device) };
		graphics_queue_index = queue_family_properties
			.iter()
			.enumerate()
			.find(|(i, properties)| {
				properties.queue_flags.contains(vk::QueueFlags::GRAPHICS)
					&& unsafe {
						surface_instance.get_physical_device_surface_support(
							physical_device,
							*i as u32,
							surface,
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
			.queue_family_index(graphics_queue_index)];

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
				.create_device(physical_device, &device_create_info, None)
				.unwrap()
		};

		let graphics_queue = unsafe { device.get_device_queue(graphics_queue_index, 0) };

		(
			device,
			graphics_queue,
			graphics_queue_index,
			surface_instance,
		)
	}

	fn create_swapchain(
		instance: &Instance,
		surface_instance: &khr::surface::Instance,
		device: &Device,
		physical_device: vk::PhysicalDevice,
		surface: vk::SurfaceKHR,
		window: *mut GLFWwindow,
	) -> (
		vk::SwapchainKHR,
		Vec<vk::Image>,
		khr::swapchain::Device,
		vk::Extent2D,
		vk::SurfaceFormatKHR,
	) {
		let surface_capabilities = unsafe {
			surface_instance.get_physical_device_surface_capabilities(physical_device, surface)
		}
		.unwrap();
		let extent = Self::choose_extent(window, &surface_capabilities);
		let min_image_count = Self::choose_min_image_count(&surface_capabilities);

		let available_formats = unsafe {
			surface_instance.get_physical_device_surface_formats(physical_device, surface)
		}
		.unwrap();
		let surface_format = Self::choose_surface_format(&available_formats);

		let available_presentmodes = unsafe {
			surface_instance.get_physical_device_surface_present_modes(physical_device, surface)
		}
		.unwrap();
		let present_mode = Self::choose_present_mode(&available_presentmodes);

		let create_info = vk::SwapchainCreateInfoKHR::default()
			.surface(surface)
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

		let device_swapchain_functions = khr::swapchain::Device::new(instance, device);

		let swapchain =
			unsafe { device_swapchain_functions.create_swapchain(&create_info, None) }.unwrap();
		let swapchain_images =
			unsafe { device_swapchain_functions.get_swapchain_images(swapchain) }.unwrap();

		(
			swapchain,
			swapchain_images,
			device_swapchain_functions,
			extent,
			surface_format,
		)
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

	fn choose_extent(
		window: *mut GLFWwindow,
		capabilities: &vk::SurfaceCapabilitiesKHR,
	) -> vk::Extent2D {
		if capabilities.current_extent.width != u32::MAX {
			return capabilities.current_extent;
		}

		let mut width = -1;
		let mut height = -1;
		unsafe {
			glfwGetFramebufferSize(window, &mut width, &mut height);
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

	fn create_imageviews(
		device: &Device,
		surface_format: vk::SurfaceFormatKHR,
		swapchain_images: &[vk::Image],
	) -> Vec<vk::ImageView> {
		let mut create_info = vk::ImageViewCreateInfo::default()
			.view_type(vk::ImageViewType::TYPE_2D)
			.format(surface_format.format)
			.subresource_range(vk::ImageSubresourceRange {
				aspect_mask: vk::ImageAspectFlags::COLOR,
				base_mip_level: 0,
				level_count: 1,
				base_array_layer: 0,
				layer_count: 1,
			});

		let mut image_views = Vec::new();
		image_views.reserve_exact(swapchain_images.len());

		for &image in swapchain_images {
			create_info = create_info.image(image);
			image_views.push(unsafe { device.create_image_view(&create_info, None) }.unwrap());
		}

		image_views
	}

	fn create_graphics_pipeline(
		device: &Device,
		surface_format: vk::SurfaceFormatKHR,
		extent: vk::Extent2D,
	) -> (vk::Pipeline, vk::PipelineLayout, vk::ShaderModule) {
		let shader_module =
			Self::create_shader_module(device, concat!(env!("OUT_DIR"), "/shader.spv"));

		let vertex_stage_info = vk::PipelineShaderStageCreateInfo::default()
			.stage(vk::ShaderStageFlags::VERTEX)
			.module(shader_module)
			.name(c"vertex_main");

		let fragment_stage_info = vk::PipelineShaderStageCreateInfo::default()
			.stage(vk::ShaderStageFlags::FRAGMENT)
			.module(shader_module)
			.name(c"fragment_main");

		let stages = &[vertex_stage_info, fragment_stage_info];

		let binding_descriptions = &[Vertex::BINDING_DESCRIPTION];
		let attribute_descriptions = Vertex::ATTRIBUTE_DESCRIPTIONS;
		let vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
			.vertex_binding_descriptions(binding_descriptions)
			.vertex_attribute_descriptions(attribute_descriptions);

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
			extent: extent,
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
			.color_write_mask(vk::ColorComponentFlags::RGBA)];

		let color_blend =
			vk::PipelineColorBlendStateCreateInfo::default().attachments(color_blend_attachments);

		let pipeline_layout_info = vk::PipelineLayoutCreateInfo::default();
		let pipeline_layout =
			unsafe { device.create_pipeline_layout(&pipeline_layout_info, None) }.unwrap();

		let color_attachment_formats = array::from_ref(&surface_format.format);

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
			.layout(pipeline_layout)
			.push_next(&mut pipeline_rendering_info)];

		let pipeline = unsafe {
			device.create_graphics_pipelines(vk::PipelineCache::null(), create_infos, None)
		}
		.unwrap()[0];

		(pipeline, pipeline_layout, shader_module)
	}

	fn create_shader_module<P: AsRef<Path>>(device: &Device, shader_path: P) -> vk::ShaderModule {
		let mut file = File::open(shader_path).unwrap();
		let shader_code = ash::util::read_spv(&mut file).unwrap();

		let create_info = vk::ShaderModuleCreateInfo::default().code(&shader_code);
		unsafe { device.create_shader_module(&create_info, None) }.unwrap()
	}

	fn create_command_pool(device: &Device, graphics_queue_index: u32) -> vk::CommandPool {
		let create_info = vk::CommandPoolCreateInfo::default()
			.flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
			.queue_family_index(graphics_queue_index);

		unsafe { device.create_command_pool(&create_info, None) }.unwrap()
	}

	fn create_command_buffers(
		device: &Device,
		command_pool: vk::CommandPool,
	) -> Vec<vk::CommandBuffer> {
		let alloc_info = vk::CommandBufferAllocateInfo::default()
			.command_pool(command_pool)
			.level(vk::CommandBufferLevel::PRIMARY)
			.command_buffer_count(MAX_FRAMES_IN_FLIGHT as u32);

		unsafe { device.allocate_command_buffers(&alloc_info) }.unwrap()
	}

	fn record_command_buffer(&self, image_index: u32, frame_index: usize) {
		let command_buffer = self.command_buffers[frame_index];

		unsafe {
			self.device
				.begin_command_buffer(command_buffer, &vk::CommandBufferBeginInfo::default())
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
			frame_index,
		);

		let color_attachments = &[vk::RenderingAttachmentInfo::default()
			.image_view(self.swapchain_image_views[image_index as usize])
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
			self.device
				.cmd_begin_rendering(command_buffer, &rendering_info);

			self.device.cmd_bind_pipeline(
				command_buffer,
				vk::PipelineBindPoint::GRAPHICS,
				self.pipeline,
			);

			self.device.cmd_bind_vertex_buffers(
				command_buffer,
				0,
				array::from_ref(&self.vertex_buffer),
				&[0],
			);

			self.device.cmd_set_viewport(
				command_buffer,
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
			self.device.cmd_set_scissor(
				command_buffer,
				0,
				&[vk::Rect2D {
					offset: vk::Offset2D { x: 0, y: 0 },
					extent: self.extent,
				}],
			);

			self.device
				.cmd_draw(command_buffer, Self::VERTICES.len() as u32, 1, 0, 0);

			self.device.cmd_end_rendering(command_buffer);
		}

		self.transition_image_layout(
			vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
			vk::ImageLayout::PRESENT_SRC_KHR,
			vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
			vk::AccessFlags2::empty(),
			vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
			vk::PipelineStageFlags2::BOTTOM_OF_PIPE,
			image_index,
			frame_index,
		);

		unsafe {
			self.device.end_command_buffer(command_buffer).unwrap();
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
		frame_index: usize,
	) {
		let barrier = &[vk::ImageMemoryBarrier2::default()
			.old_layout(old_layout)
			.new_layout(new_layout)
			.src_access_mask(src_access_mask)
			.dst_access_mask(dst_access_mask)
			.src_stage_mask(src_stage_mask)
			.dst_stage_mask(dst_stage_mask)
			.image(self.swapchain_images[image_index as usize])
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
				.cmd_pipeline_barrier2(self.command_buffers[frame_index], &dependency_info)
		};
	}

	fn create_vertex_buffer(
		instance: &Instance,
		device: &Device,
		physical_device: vk::PhysicalDevice,
	) -> (vk::Buffer, vk::DeviceMemory) {
		let create_info = vk::BufferCreateInfo::default()
			.size(std::mem::size_of_val(Self::VERTICES) as u64)
			.usage(vk::BufferUsageFlags::VERTEX_BUFFER)
			.sharing_mode(vk::SharingMode::EXCLUSIVE);
		let vertex_buffer = unsafe { device.create_buffer(&create_info, None) }.unwrap();

		let mem_requirements = unsafe { device.get_buffer_memory_requirements(vertex_buffer) };

		let alloc_info = vk::MemoryAllocateInfo::default()
			.allocation_size(mem_requirements.size)
			.memory_type_index(Self::find_memory_type(
				instance,
				physical_device,
				mem_requirements.memory_type_bits,
				vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
			));
		let vertex_buffer_memory = unsafe { device.allocate_memory(&alloc_info, None) }.unwrap();

		unsafe {
			device
				.bind_buffer_memory(vertex_buffer, vertex_buffer_memory, 0)
				.unwrap();
			let buf = device
				.map_memory(
					vertex_buffer_memory,
					0,
					create_info.size,
					vk::MemoryMapFlags::empty(),
				)
				.unwrap();
			std::ptr::copy_nonoverlapping(
				Self::VERTICES.as_ptr(),
				buf as *mut Vertex,
				Self::VERTICES.len(),
			);
			device.unmap_memory(vertex_buffer_memory);
		};

		(vertex_buffer, vertex_buffer_memory)
	}

	fn find_memory_type(
		instance: &Instance,
		physical_device: vk::PhysicalDevice,
		type_filter: u32,
		properties: vk::MemoryPropertyFlags,
	) -> u32 {
		let mem_properties =
			unsafe { instance.get_physical_device_memory_properties(physical_device) };

		for i in 0..mem_properties.memory_type_count {
			if type_filter & 1 << i != 0
				&& mem_properties.memory_types[i as usize]
					.property_flags
					.contains(properties)
			{
				return i;
			}
		}

		todo!()
	}

	fn create_sync_objects(
		device: &Device,
		swapchain_images: &[vk::Image],
	) -> (Vec<vk::Fence>, Vec<vk::Semaphore>, Vec<vk::Semaphore>) {
		let mut present_complete_sems = Vec::new();
		present_complete_sems.reserve_exact(MAX_FRAMES_IN_FLIGHT);
		let mut render_complete_sems = Vec::new();
		render_complete_sems.reserve_exact(swapchain_images.len());
		let mut draw_fences = Vec::new();
		draw_fences.reserve_exact(MAX_FRAMES_IN_FLIGHT);

		let sem_create_info = vk::SemaphoreCreateInfo::default();
		for _ in 0..swapchain_images.len() {
			unsafe {
				render_complete_sems.push(device.create_semaphore(&sem_create_info, None).unwrap());
			}
		}
		for _ in 0..MAX_FRAMES_IN_FLIGHT {
			unsafe {
				present_complete_sems
					.push(device.create_semaphore(&sem_create_info, None).unwrap());
				draw_fences.push(
					device
						.create_fence(
							&vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
							None,
						)
						.unwrap(),
				);
			}
		}

		(draw_fences, present_complete_sems, render_complete_sems)
	}

	fn main_loop(&mut self) {
		unsafe {
			let mut frame_index = 0;
			while glfwWindowShouldClose(self.window) == GLFW_FALSE {
				glfwPollEvents();
				self.draw_frame(&mut frame_index);
			}

			self.device.device_wait_idle().unwrap();
		}
	}

	fn draw_frame(&mut self, frame_index: &mut usize) {
		let command_buffer = array::from_ref(&self.command_buffers[*frame_index]);
		let draw_fence = self.draw_fences[*frame_index];
		let present_complete_sem = self.present_complete_sems[*frame_index];
		// per image, so requires the image index to be acquired from swapchain
		let render_complete_sem;

		unsafe {
			self.device
				.wait_for_fences(array::from_ref(&draw_fence), false, u64::MAX)
				.unwrap();
		}

		let (image_index, _) = match unsafe {
			self.device_swapchain_functions.acquire_next_image(
				self.swapchain,
				u64::MAX,
				present_complete_sem,
				vk::Fence::null(),
			)
		} {
			Ok(res) => res,
			Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
				*self.framebuffer_resized = false;
				self.recreate_swapchain();
				return;
			}
			Err(e) => panic!("{e:?}"),
		};

		unsafe {
			self.device
				.reset_fences(array::from_ref(&draw_fence))
				.unwrap();
		}

		self.record_command_buffer(image_index, *frame_index);

		render_complete_sem = self.render_complete_sems[image_index as usize];

		let wait_semaphores = array::from_ref(&present_complete_sem);
		let wait_stages = &[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
		let signal_semaphores = array::from_ref(&render_complete_sem);
		let submits = &[vk::SubmitInfo::default()
			.wait_semaphores(wait_semaphores)
			.wait_dst_stage_mask(wait_stages)
			.signal_semaphores(signal_semaphores)
			.command_buffers(command_buffer)];

		unsafe {
			self.device
				.queue_submit(self.graphics_queue, submits, draw_fence)
				.unwrap();
		}

		let wait_semaphores = array::from_ref(&render_complete_sem);
		let swapchains = array::from_ref(&self.swapchain);
		let image_indices = array::from_ref(&image_index);
		let present_info = vk::PresentInfoKHR::default()
			.wait_semaphores(wait_semaphores)
			.swapchains(swapchains)
			.image_indices(image_indices);
		match unsafe {
			self.device_swapchain_functions
				.queue_present(self.graphics_queue, &present_info)
		} {
			Ok(false) if !*self.framebuffer_resized => (),
			Ok(_) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
				*self.framebuffer_resized = false;
				self.recreate_swapchain();
			}
			Err(e) => panic!("{e:?}"),
		}

		*frame_index = (*frame_index + 1) % MAX_FRAMES_IN_FLIGHT;
	}

	fn recreate_swapchain(&mut self) {
		unsafe {
			self.device.device_wait_idle().unwrap();

			let (mut width, mut height) = (0, 0);
			loop {
				glfwGetFramebufferSize(self.window, &mut width, &mut height);
				if width == 0 || height == 0 {
					glfwWaitEvents();
					continue;
				}

				break;
			}

			for &view in self.swapchain_image_views.iter() {
				self.device.destroy_image_view(view, None);
			}
			self.device_swapchain_functions
				.destroy_swapchain(self.swapchain, None);
		}

		(
			self.swapchain,
			self.swapchain_images,
			self.device_swapchain_functions,
			self.extent,
			self.surface_format,
		) = Self::create_swapchain(
			&self.instance,
			&self.surface_instance,
			&self.device,
			self.physical_device,
			self.surface,
			self.window,
		);

		self.swapchain_image_views =
			Self::create_imageviews(&self.device, self.surface_format, &self.swapchain_images);
	}

	fn set_resize_callback(&mut self) {
		unsafe {
			glfwSetWindowUserPointer(
				self.window,
				&mut *self.framebuffer_resized as *mut bool as *mut _,
			);
			glfwSetFramebufferSizeCallback(self.window, Some(resize_callback));
		}
	}
}

impl Default for TriangleApplication {
	fn default() -> Self {
		let window = Self::init_window();
		let mut application = Self::init_vulkan(window);
		application.set_resize_callback();

		application
	}
}

impl Drop for TriangleApplication {
	fn drop(&mut self) {
		unsafe {
			self.device.destroy_buffer(self.vertex_buffer, None);
			self.device.free_memory(self.vertex_buffer_memory, None);
			for &sem in self.render_complete_sems.iter() {
				self.device.destroy_semaphore(sem, None);
			}
			for &sem in self.present_complete_sems.iter() {
				self.device.destroy_semaphore(sem, None);
			}
			for &fence in self.draw_fences.iter() {
				self.device.destroy_fence(fence, None);
			}
			self.device
				.free_command_buffers(self.command_pool, &self.command_buffers);
			self.device.destroy_command_pool(self.command_pool, None);
			self.device.destroy_pipeline(self.pipeline, None);
			self.device
				.destroy_pipeline_layout(self.pipeline_layout, None);
			self.device.destroy_shader_module(self.shader_module, None);
			for &imageview in self.swapchain_image_views.iter() {
				self.device.destroy_image_view(imageview, None);
			}
			self.device_swapchain_functions
				.destroy_swapchain(self.swapchain, None);
			self.surface_instance.destroy_surface(self.surface, None);
			self.debug_instance
				.destroy_debug_utils_messenger(self.debug_messenger, None);
			self.device.destroy_device(None);
			self.instance.destroy_instance(None);
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

unsafe extern "C" fn resize_callback(
	window: *mut GLFWwindow,
	_width: std::ffi::c_int,
	_height: std::ffi::c_int,
) {
	let framebuffer_resized = unsafe { glfwGetWindowUserPointer(window) as *mut bool };
	// unsafe { (glfwGetWindowUserPointer(window) as *mut TriangleApplication).as_mut() }.unwrap();
	unsafe {
		*framebuffer_resized = true;
	}
}
