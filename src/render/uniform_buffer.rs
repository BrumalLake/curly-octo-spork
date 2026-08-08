extern crate nalgebra_glm as glm;

#[repr(C)]
pub struct UniformBuffer {
	mvp: MVP,
}

#[repr(C, align(16))]
struct MVP {
	model: glm::Mat4,
	view: glm::Mat4,
	proj: glm::Mat4,
}

impl UniformBuffer {
	pub fn new(model: glm::Mat4, view: glm::Mat4, proj: glm::Mat4) -> Self {
		Self {
			mvp: MVP { model, view, proj },
		}
	}
}
