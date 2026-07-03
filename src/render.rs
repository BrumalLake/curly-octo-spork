use std::{collections::HashSet, ffi::CStr, ptr::null_mut, slice};

use ash::{Entry, Instance, vk::{API_VERSION_1_3, ApplicationInfo, InstanceCreateInfo}};
use glfw::{GLFW_CLIENT_API, GLFW_FALSE, GLFW_NO_API, GLFW_RESIZABLE, GLFWwindow, glfwCreateWindow, glfwDestroyWindow, glfwGetRequiredInstanceExtensions, glfwInit, glfwMakeContextCurrent, glfwPollEvents, glfwSwapBuffers, glfwTerminate, glfwWindowHint, glfwWindowShouldClose};

#[derive(Default)]
pub struct TriangleApplication {
	window: *mut GLFWwindow,

	// default is linked
	entry: Entry,
	instance: Option<Instance>
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


			self.window = glfwCreateWindow(WIDTH, HEIGHT, c"Vulkan".as_ptr(), null_mut(), null_mut());

    		glfwMakeContextCurrent(self.window);
		}
	}

	fn init_vulkan(&mut self) {
		self.create_instance();
	}

	fn create_instance(&mut self) {
		let application_info = ApplicationInfo::default()
		.application_name(c"Hello Triangle")
		.application_version(1)
		.engine_name(c"No Engine")
		.engine_version(1)
		.api_version(API_VERSION_1_3);

		let mut glfw_extension_count = 0;
		let glfw_extensions = unsafe {
			match glfwGetRequiredInstanceExtensions(&mut glfw_extension_count) {
				res if res == null_mut() => panic!(),
				res => res
			}
		};

		// verify required extensions are all available
		{
			let raw_extension_properties = unsafe { self.entry.enumerate_instance_extension_properties(None).unwrap() };
			let extension_properties: HashSet<_> = raw_extension_properties
			.iter()
			.map(|e| e.extension_name_as_c_str().unwrap())
			.collect();

			for &mut required_extension in unsafe { slice::from_raw_parts_mut(glfw_extensions, glfw_extension_count.try_into().unwrap()) } {
				let required_extension = unsafe { CStr::from_ptr(required_extension) };
				if !extension_properties.contains(&required_extension) {
					panic!("{required_extension:?} not found in {extension_properties:#?}");
				}
			}
		}

		let instance_create_info = InstanceCreateInfo {
			pp_enabled_extension_names: glfw_extensions,
			enabled_extension_count: glfw_extension_count,
			..Default::default()
		}
		.application_info(&application_info);

		self.instance = Some(unsafe { self.entry.create_instance(&instance_create_info, None).unwrap() });
	}

	fn main_loop(&self) {
		unsafe { while glfwWindowShouldClose(self.window) == GLFW_FALSE {
			glfwSwapBuffers(self.window);
			glfwPollEvents();
		}}
	}

	fn cleanup(self) {
		unsafe { self.instance.unwrap().destroy_instance(None); }
		
		unsafe {
			glfwDestroyWindow(self.window);
			glfwTerminate();
		}
	}
}
