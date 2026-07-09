use std::{
	borrow::Cow,
	collections::HashSet,
	ffi::{CStr, c_char},
	ptr::null_mut,
};

use ash::{
	Device, Entry, Instance,
	ext::debug_utils,
	vk::{
		self, API_VERSION_1_3, ApplicationInfo, DebugUtilsMessageSeverityFlagsEXT,
		DebugUtilsMessageTypeFlagsEXT, DebugUtilsMessengerCreateInfoEXT, DebugUtilsMessengerEXT,
		EXT_DEBUG_UTILS_NAME, InstanceCreateInfo, PhysicalDevice, PhysicalDeviceType,
	},
};
use glfw::{
	GLFW_CLIENT_API, GLFW_FALSE, GLFW_NO_API, GLFW_RESIZABLE, GLFWwindow, glfwCreateWindow,
	glfwDestroyWindow, glfwGetRequiredInstanceExtensions, glfwInit, glfwMakeContextCurrent,
	glfwPollEvents, glfwSwapBuffers, glfwTerminate, glfwWindowHint, glfwWindowShouldClose,
};

const ENABLE_VALIDATION_LAYERS: bool = cfg!(debug_assertions);
const VALIDATION_LAYERS: &[&CStr] = &[c"VK_LAYER_KHRONOS_validation"];

#[derive(Default)]
pub struct TriangleApplication {
	window: *mut GLFWwindow,

	// default is linked
	entry: Entry,
	instance: Option<Instance>,
	debug_instance: Option<debug_utils::Instance>,
	debug_messenger: DebugUtilsMessengerEXT,
	physical_device: PhysicalDevice,
	device: Option<Device>,
	queue: Option<vk::Queue>,
}

impl TriangleApplication {
	pub fn render(mut self) {
		self.init_window();
		self.init_vulkan();
		self.main_loop();
		self.cleanup();
	}

	fn init_window(&mut self) {
		const WIDTH: i32 = 800;
		const HEIGHT: i32 = 600;

		unsafe {
			glfwInit();

			// glfwWindowHint(GLFW_CLIENT_API, GLFW_NO_API);
			glfwWindowHint(GLFW_RESIZABLE, GLFW_FALSE);

			self.window =
				glfwCreateWindow(WIDTH, HEIGHT, c"Vulkan".as_ptr(), null_mut(), null_mut());

			glfwMakeContextCurrent(self.window);
		}
	}

	fn init_vulkan(&mut self) {
		self.create_instance();
		self.setup_debug_messenger();
		self.pick_physical_device();
		self.create_logical_device();
	}

	fn create_instance(&mut self) {
		let application_info = ApplicationInfo::default()
			.application_name(c"Hello Triangle")
			.application_version(0)
			.engine_name(c"No Engine")
			.engine_version(1)
			.api_version(API_VERSION_1_3);

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
			if !layer_names.contains(unsafe { CStr::from_ptr(required_layer) }) {
				panic!("{required_layer:?} not found in {layer_names:#?}");
			}
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
			if !extension_properties.contains(&required_extension) {
				panic!("{required_extension:?} not found in {extension_properties:#?}");
			}
		}

		let instance_create_info = InstanceCreateInfo::default()
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
		let glfw_extensions = unsafe {
			match glfwGetRequiredInstanceExtensions(&mut glfw_extension_count) {
				res if res == null_mut() => panic!(),
				res => res,
			}
		};

		res = unsafe {
			std::slice::from_raw_parts(glfw_extensions, glfw_extension_count.try_into().unwrap())
		}
		.iter()
		.map(|&e| e)
		.collect();

		if ENABLE_VALIDATION_LAYERS {
			res.push(EXT_DEBUG_UTILS_NAME.as_ptr());
		}

		res
	}

	fn setup_debug_messenger(&mut self) {
		let debug_instance =
			debug_utils::Instance::new(&self.entry, self.instance.as_ref().unwrap());

		let debug_messenger_info = DebugUtilsMessengerCreateInfoEXT::default()
			.message_severity(
				DebugUtilsMessageSeverityFlagsEXT::WARNING
					| DebugUtilsMessageSeverityFlagsEXT::ERROR,
			)
			.message_type(
				DebugUtilsMessageTypeFlagsEXT::GENERAL
					| DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
					| DebugUtilsMessageTypeFlagsEXT::VALIDATION,
			)
			.pfn_user_callback(Some(debug_callback));

		self.debug_messenger =
			unsafe { debug_instance.create_debug_utils_messenger(&debug_messenger_info, None) }
				.unwrap();

		self.debug_instance = Some(debug_instance);
	}

	fn pick_physical_device(&mut self) {
		let physical_devices =
			unsafe { self.instance.as_ref().unwrap().enumerate_physical_devices() }.unwrap();
		if physical_devices.is_empty() {
			panic!("failed to find GPU with Vulkan support");
		}

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

	fn score_device(&self, device: PhysicalDevice) -> Result<u8, ()> {
		let mut score = 0;
		let instance = self.instance.as_ref().unwrap();
		let device_properties = unsafe { instance.get_physical_device_properties(device) };

		// check if device is suitable
		// supports vulkan api 1.3
		if !(device_properties.api_version >= API_VERSION_1_3) {
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
				&& extended_dynamic_state.extended_dynamic_state == vk::TRUE)
		} {
			return Err(());
		}

		if device_properties.device_type == PhysicalDeviceType::INTEGRATED_GPU {
			score += 1;
		}

		Ok(score)
	}

	fn create_logical_device(&mut self) {
		let instance = self.instance.as_ref().unwrap();

		let graphics_index: u32;
		let queue_family_properties =
			unsafe { instance.get_physical_device_queue_family_properties(self.physical_device) };
		graphics_index = queue_family_properties
			.iter()
			.enumerate()
			.find(|(_, properties)| properties.queue_flags.contains(vk::QueueFlags::GRAPHICS))
			.unwrap()
			.0
			.try_into()
			.unwrap();

		let queue_priority = &[0.5f32];

		let device_queue_infos = &[vk::DeviceQueueCreateInfo::default()
			.queue_priorities(queue_priority)
			.queue_family_index(graphics_index)];

		let mut vk11 = vk::PhysicalDeviceVulkan11Features::default().shader_draw_parameters(true);
		let mut vk13 = vk::PhysicalDeviceVulkan13Features::default().dynamic_rendering(true);
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

		self.queue = Some(unsafe { device.get_device_queue(graphics_index, 0) });

		self.device = Some(device);
	}

	fn main_loop(&self) {
		unsafe {
			while glfwWindowShouldClose(self.window) == GLFW_FALSE {
				glfwSwapBuffers(self.window);
				glfwPollEvents();
			}
		}
	}

	fn cleanup(self) {
		unsafe {
			self.debug_instance
				.unwrap()
				.destroy_debug_utils_messenger(self.debug_messenger, None);
			self.device.unwrap().destroy_device(None);
			self.instance.unwrap().destroy_instance(None);
		}

		unsafe {
			glfwDestroyWindow(self.window);
			glfwTerminate();
		}
	}
}

unsafe extern "system" fn debug_callback(
	message_severity: DebugUtilsMessageSeverityFlagsEXT,
	message_type: DebugUtilsMessageTypeFlagsEXT,
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
		"validation layer type: {message_type:?} {message_severity:?} {message_id_name} {message_id} \
			message: {message}"
	);

	vk::FALSE
}
