use nalgebra::{self, Vector2, Vector3};

use ash::vk;

#[derive(Copy, Clone)]
pub struct Vertex {
	pos: Vector2<f32>,
	color: Vector3<f32>,
}

impl Vertex {
	pub const BINDING_DESCRIPTION: vk::VertexInputBindingDescription =
		vk::VertexInputBindingDescription {
			binding: 0,
			stride: std::mem::size_of::<Self>() as u32,
			input_rate: vk::VertexInputRate::VERTEX,
		};

	pub const ATTRIBUTE_DESCRIPTIONS: &[vk::VertexInputAttributeDescription] = &[
		vk::VertexInputAttributeDescription {
			location: 0,
			binding: 0,
			format: vk::Format::R32G32_SFLOAT,
			offset: std::mem::offset_of!(Self, pos) as u32,
		},
		vk::VertexInputAttributeDescription {
			location: 1,
			binding: 0,
			format: vk::Format::R32G32B32_SFLOAT,
			offset: std::mem::offset_of!(Self, color) as u32,
		},
	];

	pub const fn new(pos: [f32; 2], color: [f32; 3]) -> Self {
		Self {
			pos: Vector2::new(pos[0], pos[1]),
			color: Vector3::new(color[0], color[1], color[2]),
		}
	}

	pub const fn create_vertices<const N: usize>(values: [([f32; 2], [f32; 3]); N]) -> [Vertex; N] {
		let mut array = [Self::new([0.0; 2], [0.0; 3]); N];

		let mut i = 0;
		while i < N {
			let value = &values[i];
			array[i] = Self::new(value.0, value.1);
			i += 1;
		}

		array
	}
}

impl From<([f32; 2], [f32; 3])> for Vertex {
	fn from(value: ([f32; 2], [f32; 3])) -> Self {
		Self::new(value.0, value.1)
	}
}
